use super::catalog;
use super::settings::atomic_write;
use super::{
    CatalogueBundle, CsvDocument, GameSyncSource, SyncApplyResult, SyncChange, SyncProposal,
    SyncSection,
};
use base64::Engine as _;
use retoc::asset_conversion::{self, FZenPackageContext};
use retoc::legacy_asset::FLegacyPackageHeader;
use retoc::logging::Log;
use retoc::version::EngineVersion;
use retoc::{
    Config, EIoChunkType, FIoChunkId, FPackageId, FileWriterTrait, UEPath, build_verse_cell_store,
    iostore,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const ITEM_FOOD_PREFIX: &str = "../../../Voyage/Content/Data/Assets/HumanGrow/Food/";
const ITEM_MEMORY_PREFIX: &str = "../../../Voyage/Content/Data/Assets/HumanGrow/Memories/";
const PROFESSION_PREFIX: &str = "../../../Voyage/Content/Data/Assets/HumanGrow/Traits/Professions/";
const FOOD_STRINGS: &str = "../../../Voyage/Content/LocalizationStringTables/ST_Food.uasset";
const MEMORY_STRINGS: &str = "../../../Voyage/Content/LocalizationStringTables/ST_Memories.uasset";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExtractedItem {
    asset_name: String,
    kind: String,
    display_name: String,
    localization_key: Option<String>,
    stats: BTreeMap<String, i32>,
    unsupported_stats: Vec<String>,
    icon_asset: Option<String>,
    icon_data_url: Option<String>,
    is_development: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExtractedHuman {
    asset_name: String,
    category: String,
    display_name: String,
    requirements: BTreeMap<String, i32>,
    unsupported_stats: Vec<String>,
    tier: Option<u8>,
}

#[derive(Default)]
struct MemoryWriter {
    files: Mutex<HashMap<String, Vec<u8>>>,
}

impl FileWriterTrait for MemoryWriter {
    fn write_file(&self, path: String, _allow_compress: bool, data: Vec<u8>) -> anyhow::Result<()> {
        self.files.lock().unwrap().insert(path, data);
        Ok(())
    }
}

impl MemoryWriter {
    fn take(self) -> HashMap<String, Vec<u8>> {
        self.files.into_inner().unwrap()
    }
}

pub fn scan_and_compare(
    root: &Path,
    requested_game_path: Option<&str>,
) -> Result<SyncProposal, String> {
    let game_path = resolve_game_path(requested_game_path)?;
    let paks_path = resolve_paks_path(&game_path)?;
    let catalogues = catalog::load_all(root)?;
    let mappings = catalog::load_asset_mappings(root)?;

    let (items, humans, package_count, extracted_count, warnings) = extract_catalogue(&paks_path)?;
    Ok(compare_catalogues(
        &game_path,
        &paks_path,
        package_count,
        extracted_count,
        warnings,
        &catalogues,
        &mappings,
        &items,
        &humans,
    ))
}

fn resolve_game_path(requested: Option<&str>) -> Result<PathBuf, String> {
    let requested = requested.map(str::trim).filter(|path| !path.is_empty());
    let path = if let Some(path) = requested {
        PathBuf::from(path)
    } else {
        super::settings::default_game_directory().ok_or_else(|| {
            "The Steam installation was not found automatically. Select the Voyage game folder."
                .to_string()
        })?
    };
    if !path.is_dir() {
        return Err(format!(
            "The selected game folder does not exist: {}",
            path.display()
        ));
    }
    Ok(path)
}

fn resolve_paks_path(game_path: &Path) -> Result<PathBuf, String> {
    let candidates = [
        game_path.join("Voyage/Content/Paks"),
        game_path.join("Content/Paks"),
        game_path.to_path_buf(),
    ];
    candidates
        .into_iter()
        .find(|candidate| {
            candidate.is_dir()
                && std::fs::read_dir(candidate).is_ok_and(|entries| {
                    entries.filter_map(Result::ok).any(|entry| {
                        entry
                            .path()
                            .extension()
                            .and_then(|extension| extension.to_str())
                            .is_some_and(|extension| extension.eq_ignore_ascii_case("utoc"))
                    })
                })
        })
        .ok_or_else(|| {
            format!(
                "No Unreal .utoc containers were found under {}. Select the Voyage game folder, not a save folder.",
                game_path.display()
            )
        })
}

fn extract_catalogue(
    paks_path: &Path,
) -> Result<
    (
        Vec<ExtractedItem>,
        Vec<ExtractedHuman>,
        usize,
        usize,
        Vec<String>,
    ),
    String,
> {
    let store = iostore::open(paks_path, Arc::new(Config::default()))
        .map_err(|error| format!("Could not open installed game containers: {error:#}"))?;
    let package_count = store.packages().count();
    let mut packages_by_path = BTreeMap::<String, FPackageId>::new();
    for package in store.packages() {
        let chunk_id = FIoChunkId::from_package_id(package.id(), 0, EIoChunkType::ExportBundleData);
        let Some(path) = store.chunk_path(chunk_id) else {
            continue;
        };
        packages_by_path.insert(path, package.id());
    }
    let selected = packages_by_path
        .iter()
        .filter(|(path, _)| is_catalogue_package(path))
        .map(|(path, id)| (*id, path.clone()))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(
            "The installed containers do not contain the expected Voyage growth assets."
                .to_string(),
        );
    }

    let log = Log::no_log();
    let cells = build_verse_cell_store(&Vec::new());
    let context = FZenPackageContext::create(&*store, None, &log, Some(cells));
    let writer = MemoryWriter::default();
    let mut failures = Vec::new();
    for (package_id, package_path) in &selected {
        let relative = package_path
            .strip_prefix("../../../")
            .unwrap_or(package_path);
        if let Err(error) =
            asset_conversion::build_legacy(&context, *package_id, UEPath::new(relative), &writer)
        {
            failures.push(format!("Could not read {package_path}: {error:#}"));
        }
    }
    let files = writer.take();
    let extracted_count = selected.len().saturating_sub(failures.len());
    if extracted_count == 0 {
        return Err(failures.join("\n"));
    }

    let food_strings = find_asset_pair(&files, "ST_Food")
        .map(|(_, uexp)| parse_string_table(uexp))
        .unwrap_or_default();
    let memory_strings = find_asset_pair(&files, "ST_Memories")
        .map(|(_, uexp)| parse_string_table(uexp))
        .unwrap_or_default();

    let mut items = Vec::new();
    let mut humans = Vec::new();
    let mut warnings = failures;
    for (_, package_path) in selected {
        let stem = package_path
            .rsplit('/')
            .next()
            .and_then(|name| name.strip_suffix(".uasset"))
            .unwrap_or_default();
        let Some((uasset, uexp)) = find_asset_pair(&files, stem) else {
            continue;
        };
        if package_path.starts_with(ITEM_FOOD_PREFIX) && stem.starts_with("DA_Food_") {
            match parse_item(stem, "food", &package_path, uasset, uexp, &food_strings) {
                Ok(item) => items.push(item),
                Err(error) => warnings.push(format!("{stem}: {error}")),
            }
        } else if package_path.starts_with(ITEM_MEMORY_PREFIX) && stem.starts_with("DA_Memory_") {
            match parse_item(
                stem,
                "memories",
                &package_path,
                uasset,
                uexp,
                &memory_strings,
            ) {
                Ok(item) => items.push(item),
                Err(error) => warnings.push(format!("{stem}: {error}")),
            }
        } else if package_path.starts_with(PROFESSION_PREFIX) && stem.starts_with("DA_Profession_")
        {
            match parse_human(stem, &package_path, uasset, uexp) {
                Ok(human) => humans.push(human),
                Err(error) => warnings.push(format!("{stem}: {error}")),
            }
        }
    }
    extract_item_icons(&context, &packages_by_path, &mut items, &mut warnings);
    items.sort_by(|left, right| left.asset_name.cmp(&right.asset_name));
    assign_human_tiers(&mut humans);
    humans.sort_by(|left, right| left.asset_name.cmp(&right.asset_name));
    Ok((items, humans, package_count, extracted_count, warnings))
}

fn is_catalogue_package(path: &str) -> bool {
    path == FOOD_STRINGS
        || path == MEMORY_STRINGS
        || (path.starts_with(ITEM_FOOD_PREFIX)
            && path
                .rsplit('/')
                .next()
                .is_some_and(|name| name.starts_with("DA_Food_")))
        || (path.starts_with(ITEM_MEMORY_PREFIX)
            && path
                .rsplit('/')
                .next()
                .is_some_and(|name| name.starts_with("DA_Memory_")))
        || (path.starts_with(PROFESSION_PREFIX)
            && path
                .rsplit('/')
                .next()
                .is_some_and(|name| name.starts_with("DA_Profession_")))
}

fn find_asset_pair<'a>(
    files: &'a HashMap<String, Vec<u8>>,
    stem: &str,
) -> Option<(&'a [u8], &'a [u8])> {
    let uasset = files
        .iter()
        .find(|(path, _)| path.ends_with(&format!("/{stem}.uasset")))?;
    let uexp_path = uasset.0.strip_suffix(".uasset")?.to_string() + ".uexp";
    Some((uasset.1.as_slice(), files.get(&uexp_path)?.as_slice()))
}

fn parse_item(
    asset_name: &str,
    kind: &str,
    package_path: &str,
    uasset: &[u8],
    uexp: &[u8],
    strings: &BTreeMap<String, String>,
) -> Result<ExtractedItem, String> {
    let imports = import_names(uasset)?;
    let localization_key = ascii_runs(uexp)
        .into_iter()
        .find(|value| value.ends_with("_Name"));
    let display_name = localization_key
        .as_ref()
        .and_then(|key| strings.get(key))
        .cloned()
        .unwrap_or_else(|| humanize_asset_name(asset_name));
    let icon_asset = imports
        .iter()
        .find(|name| name.starts_with("/Game/UI/Icons/"))
        .cloned();
    let (stats, unsupported_stats) = extract_stats(&imports, uexp, kind);
    Ok(ExtractedItem {
        asset_name: asset_name.to_string(),
        kind: kind.to_string(),
        is_development: is_development_asset(package_path, asset_name, &display_name),
        display_name,
        localization_key,
        stats,
        unsupported_stats,
        icon_asset,
        icon_data_url: None,
    })
}

fn is_development_asset(package_path: &str, asset_name: &str, display_name: &str) -> bool {
    let path = package_path.to_ascii_lowercase();
    display_name.to_ascii_lowercase().contains("[dev]")
        || asset_name.to_ascii_lowercase().contains("megafat")
        || path.split('/').any(|segment| {
            matches!(
                segment,
                "notused" | "not_used" | "unused" | "development" | "developer" | "dev"
            )
        })
}

fn extract_item_icons(
    context: &FZenPackageContext,
    packages_by_path: &BTreeMap<String, FPackageId>,
    items: &mut [ExtractedItem],
    warnings: &mut Vec<String>,
) {
    let icon_assets = items
        .iter()
        .filter_map(|item| item.icon_asset.clone())
        .collect::<BTreeSet<_>>();
    let writer = MemoryWriter::default();
    let mut failed = 0_usize;
    for icon_asset in &icon_assets {
        let package_path = format!(
            "../../../Voyage/Content{}.uasset",
            icon_asset.strip_prefix("/Game").unwrap_or(icon_asset)
        );
        let Some(package_id) = packages_by_path.get(&package_path) else {
            failed += 1;
            continue;
        };
        let relative = package_path
            .strip_prefix("../../../")
            .unwrap_or(&package_path);
        if asset_conversion::build_legacy(context, *package_id, UEPath::new(relative), &writer)
            .is_err()
        {
            failed += 1;
        }
    }
    let files = writer.take();
    let thumbnails = icon_assets
        .iter()
        .filter_map(|asset| {
            let stem = asset.rsplit('/').next()?;
            let (_, uexp) = find_asset_pair(&files, stem)?;
            decode_icon_thumbnail(uexp).map(|data_url| (asset.clone(), data_url))
        })
        .collect::<BTreeMap<_, _>>();
    for item in items {
        item.icon_data_url = item
            .icon_asset
            .as_ref()
            .and_then(|asset| thumbnails.get(asset))
            .cloned();
    }
    let undecoded = icon_assets.len().saturating_sub(thumbnails.len());
    if failed > 0 || undecoded > 0 {
        warnings.push(format!(
            "{} of {} referenced item icons could not be decoded as inline BGRA8 textures.",
            failed.max(undecoded),
            icon_assets.len()
        ));
    }
}

fn decode_icon_thumbnail(bytes: &[u8]) -> Option<String> {
    const FORMAT: &[u8] = b"PF_B8G8R8A8\0";
    let format_offset = bytes
        .windows(FORMAT.len())
        .position(|window| window == FORMAT)?;
    let search_start = format_offset.saturating_sub(64);
    let (width, height) = (search_start..format_offset.saturating_sub(7))
        .rev()
        .find_map(|offset| {
            let width = u32::from_le_bytes(bytes.get(offset..offset + 4)?.try_into().ok()?);
            let height = u32::from_le_bytes(bytes.get(offset + 4..offset + 8)?.try_into().ok()?);
            (width == height && (16..=2048).contains(&width)).then_some((width, height))
        })?;
    let pixel_bytes = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    let data_search_start = format_offset + FORMAT.len();
    let data_search_end = (data_search_start + 128).min(bytes.len());
    let data_offset = (data_search_start..data_search_end).find(|offset| {
        let end = offset.saturating_add(pixel_bytes);
        end + 8 <= bytes.len()
            && bytes
                .get(end..end + 4)
                .is_some_and(|raw| raw == width.to_le_bytes())
            && bytes
                .get(end + 4..end + 8)
                .is_some_and(|raw| raw == height.to_le_bytes())
    })?;
    let pixels = bytes.get(data_offset..data_offset + pixel_bytes)?;
    let target_width = width.min(64);
    let target_height = height.min(64);
    let mut rgba = Vec::with_capacity(target_width as usize * target_height as usize * 4);
    for target_y in 0..target_height {
        let source_y = target_y * height / target_height;
        for target_x in 0..target_width {
            let source_x = target_x * width / target_width;
            let source = ((source_y * width + source_x) * 4) as usize;
            rgba.extend_from_slice(&[
                pixels[source + 2],
                pixels[source + 1],
                pixels[source],
                pixels[source + 3],
            ]);
        }
    }
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, target_width, target_height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&rgba).ok()?;
    }
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(encoded)
    ))
}

fn parse_human(
    asset_name: &str,
    package_path: &str,
    uasset: &[u8],
    uexp: &[u8],
) -> Result<ExtractedHuman, String> {
    let imports = import_names(uasset)?;
    let (mut requirements, unsupported_stats) = extract_stats(&imports, uexp, "humans");
    for (stat, baseline) in [("Weight", 20), ("Height", 30), ("Life Exp", 10)] {
        requirements.entry(stat.to_string()).or_insert(baseline);
    }
    let display_name = best_human_display_name(asset_name, uexp);
    let folder = package_path
        .strip_prefix(PROFESSION_PREFIX)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("Unknown");
    let category = match folder {
        "Artist" => "Arts & Culture",
        "Cultivators" => "Agriculture",
        "Teacher" => "Educator",
        "Healer" => "Healthcare",
        "Leader" => "Leadership",
        "Supplier" => "Logistics",
        "Protector" => "Military",
        "Scientist" => "Science",
        other => other,
    };
    Ok(ExtractedHuman {
        asset_name: asset_name.to_string(),
        category: category.to_string(),
        display_name,
        requirements,
        unsupported_stats,
        tier: None,
    })
}

fn assign_human_tiers(humans: &mut [ExtractedHuman]) {
    let mut categories = BTreeMap::<String, Vec<usize>>::new();
    for (index, human) in humans.iter().enumerate() {
        if human.unsupported_stats.is_empty() {
            categories
                .entry(human.category.clone())
                .or_default()
                .push(index);
        }
    }
    for indices in categories.values_mut() {
        if indices.len() != 4 {
            continue;
        }
        indices.sort_by_key(|index| {
            (
                humans[*index].requirements.values().sum::<i32>(),
                humans[*index].asset_name.clone(),
            )
        });
        let totals = indices
            .iter()
            .map(|index| humans[*index].requirements.values().sum::<i32>())
            .collect::<Vec<_>>();
        if totals.windows(2).any(|pair| pair[0] >= pair[1]) {
            continue;
        }
        for (tier, index) in indices.iter().enumerate() {
            humans[*index].tier = Some((tier + 1) as u8);
        }
    }
}

fn import_names(uasset: &[u8]) -> Result<Vec<String>, String> {
    let versions = [
        EngineVersion::UE5_7,
        EngineVersion::UE5_6,
        EngineVersion::UE5_5,
        EngineVersion::UE5_4,
        EngineVersion::UE5_3,
        EngineVersion::UE5_2,
        EngineVersion::UE5_1,
        EngineVersion::UE5_0,
    ];
    for version in versions {
        let mut cursor = Cursor::new(uasset);
        let Ok(header) =
            FLegacyPackageHeader::deserialize(&mut cursor, Some(version.package_file_version()))
        else {
            continue;
        };
        let names = header
            .imports
            .iter()
            .filter_map(|import| header.name_map.get(import.object_name).ok())
            .map(|name| name.into_owned())
            .collect::<Vec<_>>();
        if !names.is_empty() {
            return Ok(names);
        }
    }
    Err("Could not parse the converted Unreal package header.".to_string())
}

fn extract_stats(
    imports: &[String],
    uexp: &[u8],
    kind: &str,
) -> (BTreeMap<String, i32>, Vec<String>) {
    let allowed = match kind {
        "food" => &["Weight", "Height", "Life Exp", "Strength", "Intellect"][..],
        "memories" => &[
            "Adaptability",
            "Communication",
            "Creativity",
            "Discipline",
            "Empathy",
            "Focus",
            "Leadership",
            "Logic",
            "Patience",
            "Wisdom",
        ][..],
        _ => &[
            "Weight",
            "Height",
            "Life Exp",
            "Strength",
            "Intellect",
            "Adaptability",
            "Communication",
            "Creativity",
            "Discipline",
            "Empathy",
            "Focus",
            "Leadership",
            "Logic",
            "Patience",
            "Wisdom",
        ][..],
    };
    let mut stats = BTreeMap::new();
    let mut unsupported = Vec::new();
    for (index, import) in imports.iter().enumerate() {
        let Some(stat) = stat_from_property(import) else {
            continue;
        };
        let reference = -((index as i32) + 1);
        let Some(value) = find_property_float(uexp, reference) else {
            continue;
        };
        if !allowed.contains(&stat.as_str()) {
            unsupported.push(stat);
        } else {
            stats.insert(stat, value);
        }
    }
    unsupported.sort();
    unsupported.dedup();
    (stats, unsupported)
}

fn stat_from_property(name: &str) -> Option<String> {
    let tail = name.rsplit('/').next()?;
    let tail = tail
        .strip_prefix("DA_Physical_Human_Property_")
        .or_else(|| tail.strip_prefix("DA_Human_Property_"))?;
    let stat = match tail {
        "Life_Expectancy" => "Life Exp".to_string(),
        "Weight" | "Height" | "Strength" | "Intellect" | "Adaptability" | "Creativity"
        | "Communication" | "Discipline" | "Empathy" | "Focus" | "Leadership" | "Logic"
        | "Patience" | "Wisdom" => tail.to_string(),
        other => humanize_words(other),
    };
    Some(stat)
}

fn find_property_float(bytes: &[u8], reference: i32) -> Option<i32> {
    let reference = reference.to_le_bytes();
    if bytes.len() < 10 {
        return None;
    }
    for offset in 0..=bytes.len() - 10 {
        if bytes[offset..offset + 4] != reference {
            continue;
        }
        let candidates = if bytes.get(offset + 4..offset + 7) == Some(&[0x80, 0x05, 0x02]) {
            vec![offset + 7]
        } else if bytes.get(offset + 4..offset + 6) == Some(&[0x00, 0x05]) {
            vec![offset + 6]
        } else {
            Vec::new()
        };
        for value_offset in candidates {
            let raw =
                f32::from_le_bytes(bytes.get(value_offset..value_offset + 4)?.try_into().ok()?);
            if raw.is_finite()
                && raw.round() >= 1.0
                && raw <= 100_000.0
                && (raw - raw.round()).abs() < 0.001
            {
                return Some(raw.round() as i32);
            }
        }
    }
    None
}

fn parse_string_table(bytes: &[u8]) -> BTreeMap<String, String> {
    for offset in 0..bytes.len().min(96) {
        let Some((table_name, mut position)) = read_fstring(bytes, offset) else {
            continue;
        };
        if !table_name.starts_with("ST_") || position + 4 > bytes.len() {
            continue;
        }
        let count = i32::from_le_bytes(bytes[position..position + 4].try_into().unwrap());
        if !(1..=10_000).contains(&count) {
            continue;
        }
        position += 4;
        let mut entries = BTreeMap::new();
        for _ in 0..count {
            let Some((key, next)) = read_fstring(bytes, position) else {
                return BTreeMap::new();
            };
            let Some((value, next_value)) = read_fstring(bytes, next) else {
                return BTreeMap::new();
            };
            entries.insert(key, value);
            position = next_value;
        }
        return entries;
    }
    BTreeMap::new()
}

fn read_fstring(bytes: &[u8], offset: usize) -> Option<(String, usize)> {
    let raw_length = i32::from_le_bytes(bytes.get(offset..offset + 4)?.try_into().ok()?);
    if raw_length == 0 {
        return Some((String::new(), offset + 4));
    }
    if raw_length > 0 {
        let length = raw_length as usize;
        if length > 16_384 {
            return None;
        }
        let raw = bytes.get(offset + 4..offset + 4 + length)?;
        if raw.last() != Some(&0) {
            return None;
        }
        let value = std::str::from_utf8(&raw[..raw.len() - 1]).ok()?.to_string();
        Some((value, offset + 4 + length))
    } else {
        let length = raw_length.unsigned_abs() as usize;
        if length > 16_384 {
            return None;
        }
        let raw = bytes.get(offset + 4..offset + 4 + length * 2)?;
        let units = raw
            .chunks_exact(2)
            .take(length.saturating_sub(1))
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        Some((String::from_utf16(&units).ok()?, offset + 4 + length * 2))
    }
}

fn ascii_runs(bytes: &[u8]) -> Vec<String> {
    let mut values = Vec::new();
    let mut start = None;
    for (index, byte) in bytes.iter().copied().chain(std::iter::once(0)).enumerate() {
        if (0x20..=0x7e).contains(&byte) {
            start.get_or_insert(index);
        } else if let Some(run_start) = start.take() {
            if index.saturating_sub(run_start) >= 4 {
                values.push(String::from_utf8_lossy(&bytes[run_start..index]).into_owned());
            }
        }
    }
    values
}

fn best_human_display_name(asset_name: &str, uexp: &[u8]) -> String {
    let asset_tokens = normalized_tokens(asset_name.trim_start_matches("DA_Profession_"));
    ascii_runs(uexp)
        .into_iter()
        .filter(|candidate| {
            candidate.len() <= 80
                && !candidate.starts_with('/')
                && !candidate.starts_with("DA_")
                && !candidate.contains('_')
                && !is_hex_identifier(candidate)
        })
        .max_by_key(|candidate| {
            let tokens = normalized_tokens(candidate);
            tokens.intersection(&asset_tokens).count() * 100 + tokens.len()
        })
        .filter(|candidate| !candidate.is_empty())
        .unwrap_or_else(|| humanize_asset_name(asset_name))
}

fn normalized_tokens(value: &str) -> BTreeSet<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn is_hex_identifier(value: &str) -> bool {
    value.len() >= 24 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn humanize_asset_name(asset_name: &str) -> String {
    let value = asset_name
        .trim_start_matches("DA_Food_")
        .trim_start_matches("DA_Memory_")
        .trim_start_matches("DA_Profession_");
    humanize_words(value)
}

fn humanize_words(value: &str) -> String {
    value.replace(['_', '-'], " ")
}

fn item_name_key(item: &ExtractedItem) -> (String, String) {
    (item.kind.clone(), item.display_name.to_ascii_lowercase())
}

fn plan_item_names(items: &[ExtractedItem]) -> (HashSet<String>, HashSet<String>) {
    let mut groups = BTreeMap::<(String, String), Vec<&ExtractedItem>>::new();
    for item in items {
        groups.entry(item_name_key(item)).or_default().push(item);
    }
    let mut owners = HashSet::new();
    let mut conflicts = HashSet::new();
    for group in groups.values_mut() {
        group.sort_by(|left, right| left.asset_name.cmp(&right.asset_name));
        let candidates = group
            .iter()
            .copied()
            .filter(|item| !item.is_development)
            .collect::<Vec<_>>();
        let candidates = if candidates.is_empty() {
            group.clone()
        } else {
            candidates
        };
        let owner = candidates[0];
        let incompatible_production_assets = candidates.iter().any(|item| {
            item.stats != owner.stats || item.unsupported_stats != owner.unsupported_stats
        });
        if incompatible_production_assets {
            conflicts.extend(group.iter().map(|item| item.asset_name.clone()));
            continue;
        }
        owners.insert(owner.asset_name.clone());
        for item in group
            .iter()
            .copied()
            .filter(|item| item.asset_name != owner.asset_name)
        {
            if item.stats != owner.stats || item.unsupported_stats != owner.unsupported_stats {
                conflicts.insert(item.asset_name.clone());
            }
        }
    }
    (owners, conflicts)
}

fn item_name_conflict(item: &ExtractedItem) -> SyncChange {
    SyncChange {
        id: change_id(&item.kind, "conflict", &item.asset_name),
        section: item.kind.clone(),
        action: "conflict".to_string(),
        asset_name: Some(item.asset_name.clone()),
        display_name: item.display_name.clone(),
        summary: "Another installed asset uses the same display name with different values."
            .to_string(),
        current: None,
        proposed: None,
        selected_by_default: false,
        can_apply: false,
        reason: Some(
            "Keep this technical asset separate or assign it manually; it cannot share one catalogue row safely."
                .to_string(),
        ),
        icon_asset: item.icon_asset.clone(),
        icon_data_url: item.icon_data_url.clone(),
    }
}

#[allow(clippy::too_many_arguments)]
fn compare_catalogues(
    game_path: &Path,
    paks_path: &Path,
    package_count: usize,
    extracted_count: usize,
    mut warnings: Vec<String>,
    catalogues: &CatalogueBundle,
    mappings: &BTreeMap<String, String>,
    items: &[ExtractedItem],
    humans: &[ExtractedHuman],
) -> SyncProposal {
    let mut food_changes = Vec::new();
    let mut memory_changes = Vec::new();
    let mut human_changes = Vec::new();
    let mut mapping_changes = Vec::new();
    let mut matched_food = HashSet::new();
    let mut matched_memories = HashSet::new();
    let (catalogue_owners, conflicting_assets) = plan_item_names(items);
    let mut planned_catalogue_targets = HashSet::<(String, String)>::new();

    for item in items {
        let item_key = item_name_key(item);
        let owns_catalogue_change = catalogue_owners.contains(&item.asset_name);
        let has_name_conflict = conflicting_assets.contains(&item.asset_name);
        let document = if item.kind == "food" {
            &catalogues.food
        } else {
            &catalogues.memories
        };
        let mapped_name = mappings.get(&item.asset_name);
        let direct_local = mapped_name
            .and_then(|target| find_row(document, target))
            .or_else(|| find_row(document, &item.display_name));
        let stat_local = direct_local
            .is_none()
            .then(|| find_unique_stat_match(document, &item.stats))
            .flatten();
        let inferred_by_stats = stat_local.is_some();
        let local = direct_local.or(stat_local);
        let resolved_target = local
            .map(|row| row[0].clone())
            .unwrap_or_else(|| item.display_name.clone());
        let section_changes = if item.kind == "food" {
            &mut food_changes
        } else {
            &mut memory_changes
        };

        if let Some(row) = local {
            if item.kind == "food" {
                matched_food.insert(row[0].to_ascii_lowercase());
            } else {
                matched_memories.insert(row[0].to_ascii_lowercase());
            }
            let proposed = merge_stats(document, row, &item.stats);
            if has_name_conflict {
                section_changes.push(item_name_conflict(item));
            } else if owns_catalogue_change && proposed != *row {
                section_changes.push(SyncChange {
                    id: change_id(&item.kind, "changed", &item.asset_name),
                    section: item.kind.clone(),
                    action: "changed".to_string(),
                    asset_name: Some(item.asset_name.clone()),
                    display_name: row[0].clone(),
                    summary: describe_row_diff(document, row, &proposed),
                    current: Some(row.clone()),
                    proposed: Some(proposed),
                    selected_by_default: item.unsupported_stats.is_empty(),
                    can_apply: item.unsupported_stats.is_empty(),
                    reason: unsupported_reason(&item.unsupported_stats),
                    icon_asset: item.icon_asset.clone(),
                    icon_data_url: item.icon_data_url.clone(),
                });
            }
            planned_catalogue_targets.insert((item.kind.clone(), row[0].to_ascii_lowercase()));
        } else if has_name_conflict {
            section_changes.push(item_name_conflict(item));
        } else if owns_catalogue_change {
            let proposed = new_item_row(document, &item.display_name, &item.stats);
            let supported = item.unsupported_stats.is_empty() && !item.stats.is_empty();
            section_changes.push(SyncChange {
                id: change_id(&item.kind, "added", &item.asset_name),
                section: item.kind.clone(),
                action: if supported { "added" } else { "unsupported" }.to_string(),
                asset_name: Some(item.asset_name.clone()),
                display_name: item.display_name.clone(),
                summary: if supported {
                    format!(
                        "New installed {} with {} extracted values.",
                        item.kind.trim_end_matches('s'),
                        item.stats.len()
                    )
                } else {
                    "The installed asset does not fit the current calculator schema.".to_string()
                },
                current: None,
                proposed: supported.then_some(proposed),
                selected_by_default: supported && !item.is_development,
                can_apply: supported,
                reason: unsupported_reason(&item.unsupported_stats).or_else(|| {
                    item.is_development.then(|| {
                        "Development-only asset; review manually before importing.".to_string()
                    })
                }),
                icon_asset: item.icon_asset.clone(),
                icon_data_url: item.icon_data_url.clone(),
            });
            if supported {
                planned_catalogue_targets.insert(item_key.clone());
            }
        }

        match mapped_name {
            None => {
                let target_exists = !has_name_conflict
                    && (local.is_some() || planned_catalogue_targets.contains(&item_key));
                mapping_changes.push(SyncChange {
                    id: change_id("mappings", "added", &item.asset_name),
                    section: "mappings".to_string(),
                    action: if target_exists {
                        if inferred_by_stats { "suggested" } else { "added" }
                    } else {
                        "blocked"
                    }
                    .to_string(),
                    asset_name: Some(item.asset_name.clone()),
                    display_name: item.display_name.clone(),
                    summary: if inferred_by_stats {
                        format!(
                            "Suggested mapping to {}: its complete stat vector uniquely matches the installed asset named {}.",
                            resolved_target, item.display_name
                        )
                    } else {
                        format!("Map {} to {}.", item.asset_name, resolved_target)
                    },
                    current: None,
                    proposed: target_exists
                        .then(|| vec![item.asset_name.clone(), resolved_target.clone()]),
                    selected_by_default: target_exists
                        && !item.is_development
                        && !inferred_by_stats,
                    can_apply: target_exists,
                    reason: (!target_exists).then(|| "Import or assign a compatible catalogue item first.".to_string()),
                    icon_asset: item.icon_asset.clone(),
                    icon_data_url: item.icon_data_url.clone(),
                });
            }
            Some(target) if local.is_none() => mapping_changes.push(SyncChange {
                id: change_id("mappings", "conflict", &item.asset_name),
                section: "mappings".to_string(),
                action: "conflict".to_string(),
                asset_name: Some(item.asset_name.clone()),
                display_name: item.display_name.clone(),
                summary: format!("The verified mapping points to missing catalogue item {target}."),
                current: Some(vec![item.asset_name.clone(), target.clone()]),
                proposed: None,
                selected_by_default: false,
                can_apply: false,
                reason: Some("Resolve the local catalogue target manually; the game name is not used to overwrite verified mappings.".to_string()),
                icon_asset: item.icon_asset.clone(),
                icon_data_url: item.icon_data_url.clone(),
            }),
            _ => {}
        }
    }

    add_missing_rows(&catalogues.food, &matched_food, &mut food_changes);
    add_missing_rows(&catalogues.memories, &matched_memories, &mut memory_changes);

    let mut matched_humans = HashSet::new();
    for human in humans {
        let normalized_game_name = normalize_profession_name(&human.display_name);
        let local = catalogues.humans.rows.iter().find(|row| {
            row.first()
                .is_some_and(|category| category.eq_ignore_ascii_case(&human.category))
                && row
                    .get(1)
                    .is_some_and(|name| normalize_profession_name(name) == normalized_game_name)
        });
        if let Some(row) = local {
            matched_humans.insert(row[1].to_ascii_lowercase());
            let proposed = merge_stats(&catalogues.humans, row, &human.requirements);
            if proposed != *row {
                human_changes.push(SyncChange {
                    id: change_id("humans", "changed", &human.asset_name),
                    section: "humans".to_string(),
                    action: "changed".to_string(),
                    asset_name: Some(human.asset_name.clone()),
                    display_name: row[1].clone(),
                    summary: describe_row_diff(&catalogues.humans, row, &proposed),
                    current: Some(row.clone()),
                    proposed: Some(proposed),
                    selected_by_default: human.unsupported_stats.is_empty(),
                    can_apply: human.unsupported_stats.is_empty(),
                    reason: unsupported_reason(&human.unsupported_stats),
                    icon_asset: None,
                    icon_data_url: None,
                });
            }
        } else if human.unsupported_stats.is_empty() && human.tier.is_some() {
            let proposed = new_human_row(&catalogues.humans, human);
            human_changes.push(SyncChange {
                id: change_id("humans", "added", &human.asset_name),
                section: "humans".to_string(),
                action: "added".to_string(),
                asset_name: Some(human.asset_name.clone()),
                display_name: proposed[1].clone(),
                summary: format!(
                    "New installed profession with {} complete requirements.",
                    human.requirements.len()
                ),
                current: None,
                proposed: Some(proposed),
                selected_by_default: true,
                can_apply: true,
                reason: Some(
                    "The shared physical baseline and tier were verified against every installed four-profession category."
                        .to_string(),
                ),
                icon_asset: None,
                icon_data_url: None,
            });
        } else {
            human_changes.push(SyncChange {
                id: change_id("humans", "unsupported", &human.asset_name),
                section: "humans".to_string(),
                action: "unsupported".to_string(),
                asset_name: Some(human.asset_name.clone()),
                display_name: human.display_name.clone(),
                summary: "Installed profession not present in the local catalogue.".to_string(),
                current: None,
                proposed: None,
                selected_by_default: false,
                can_apply: false,
                reason: Some(if human.unsupported_stats.is_empty() {
                    "The profession tier could not be inferred from a complete four-profession category.".to_string()
                } else {
                    format!("Unsupported profession properties: {}.", human.unsupported_stats.join(", "))
                }),
                icon_asset: None,
                icon_data_url: None,
            });
        }
    }
    for row in &catalogues.humans.rows {
        if !matched_humans.contains(&row[1].to_ascii_lowercase()) {
            human_changes.push(missing_change("humans", &row[1], row));
        }
    }

    if items.is_empty() {
        warnings.push("No growth item assets were extracted.".to_string());
    }
    if humans.is_empty() {
        warnings.push("No profession assets were extracted.".to_string());
    }
    for changes in [
        &mut food_changes,
        &mut memory_changes,
        &mut human_changes,
        &mut mapping_changes,
    ] {
        changes.sort_by(|left, right| {
            action_rank(&left.action)
                .cmp(&action_rank(&right.action))
                .then_with(|| left.display_name.cmp(&right.display_name))
        });
    }
    SyncProposal {
        source: GameSyncSource {
            game_path: game_path.display().to_string(),
            paks_path: paks_path.display().to_string(),
            package_count,
            extracted_count,
            warnings,
        },
        sections: vec![
            SyncSection {
                kind: "food".to_string(),
                changes: food_changes,
            },
            SyncSection {
                kind: "memories".to_string(),
                changes: memory_changes,
            },
            SyncSection {
                kind: "humans".to_string(),
                changes: human_changes,
            },
            SyncSection {
                kind: "mappings".to_string(),
                changes: mapping_changes,
            },
        ],
    }
}

fn unsupported_reason(stats: &[String]) -> Option<String> {
    (!stats.is_empty()).then(|| {
        format!(
            "Unsupported properties for this section: {}.",
            stats.join(", ")
        )
    })
}

fn find_row<'a>(document: &'a CsvDocument, name: &str) -> Option<&'a Vec<String>> {
    let name_column = if document.kind == "humans" { 1 } else { 0 };
    document
        .rows
        .iter()
        .find(|row| row[name_column].eq_ignore_ascii_case(name))
}

fn find_unique_stat_match<'a>(
    document: &'a CsvDocument,
    stats: &BTreeMap<String, i32>,
) -> Option<&'a Vec<String>> {
    if stats.is_empty() {
        return None;
    }
    let matches = document
        .rows
        .iter()
        .filter(|row| row_stats(document, row) == *stats)
        .collect::<Vec<_>>();
    if matches.len() == 1 {
        Some(matches[0])
    } else {
        None
    }
}

fn row_stats(document: &CsvDocument, row: &[String]) -> BTreeMap<String, i32> {
    document
        .headers
        .iter()
        .enumerate()
        .filter(|(_, header)| {
            !matches!(
                header.as_str(),
                "Food" | "Memory" | "TotalAvailability" | "WorldCount"
            )
        })
        .filter_map(|(index, header)| {
            row.get(index)?
                .trim()
                .parse::<i32>()
                .ok()
                .filter(|value| *value > 0)
                .map(|value| (header.clone(), value))
        })
        .collect()
}

fn merge_stats(
    document: &CsvDocument,
    current: &[String],
    stats: &BTreeMap<String, i32>,
) -> Vec<String> {
    let mut proposed = current.to_vec();
    for (stat, value) in stats {
        if let Some(column) = document.headers.iter().position(|header| header == stat) {
            proposed[column] = value.to_string();
        }
    }
    proposed
}

fn new_item_row(document: &CsvDocument, name: &str, stats: &BTreeMap<String, i32>) -> Vec<String> {
    let mut row = vec![String::new(); document.headers.len()];
    row[0] = name.to_string();
    merge_stats(document, &row, stats)
}

fn new_human_row(document: &CsvDocument, human: &ExtractedHuman) -> Vec<String> {
    let mut row = vec![String::new(); document.headers.len()];
    row[0] = human.category.clone();
    row[1] = format!(
        "{} T{}",
        human.display_name,
        human.tier.expect("new human rows require a verified tier")
    );
    merge_stats(document, &row, &human.requirements)
}

fn describe_row_diff(document: &CsvDocument, current: &[String], proposed: &[String]) -> String {
    document
        .headers
        .iter()
        .zip(current.iter().zip(proposed))
        .filter(|(_, (before, after))| before != after)
        .map(|(header, (before, after))| {
            format!("{header}: {} → {}", empty_label(before), empty_label(after))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn empty_label(value: &str) -> &str {
    if value.is_empty() { "empty" } else { value }
}

fn add_missing_rows(
    document: &CsvDocument,
    matched: &HashSet<String>,
    changes: &mut Vec<SyncChange>,
) {
    for row in &document.rows {
        if !matched.contains(&row[0].to_ascii_lowercase()) {
            changes.push(missing_change(&document.kind, &row[0], row));
        }
    }
}

fn missing_change(section: &str, name: &str, row: &[String]) -> SyncChange {
    SyncChange {
        id: change_id(section, "missing", name),
        section: section.to_string(),
        action: "missing".to_string(),
        asset_name: None,
        display_name: name.to_string(),
        summary: "Local entry was not found in the installed game scan. It will not be deleted."
            .to_string(),
        current: Some(row.to_vec()),
        proposed: None,
        selected_by_default: false,
        can_apply: false,
        reason: Some(
            "Missing entries are diagnostics only and are never auto-deleted.".to_string(),
        ),
        icon_asset: None,
        icon_data_url: None,
    }
}

fn change_id(section: &str, action: &str, key: &str) -> String {
    format!("{section}:{action}:{}", key.to_ascii_lowercase())
}

fn action_rank(action: &str) -> usize {
    match action {
        "added" => 0,
        "changed" => 1,
        "suggested" => 2,
        "conflict" | "blocked" | "unsupported" => 3,
        "missing" => 4,
        _ => 5,
    }
}

fn normalize_profession_name(value: &str) -> String {
    let value = value.trim();
    let value = value
        .strip_suffix(" T1")
        .or_else(|| value.strip_suffix(" T2"))
        .or_else(|| value.strip_suffix(" T3"))
        .or_else(|| value.strip_suffix(" T4"))
        .unwrap_or(value);
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

pub fn apply_proposal(
    root: &Path,
    proposal: &SyncProposal,
    selected_ids: &[String],
) -> Result<SyncApplyResult, String> {
    let selected = selected_ids.iter().cloned().collect::<HashSet<_>>();
    let mut catalogues = catalog::load_all(root)?;
    let mut mappings = catalog::load_asset_mappings(root)?;
    let mut applied_count = 0;

    for change in proposal
        .sections
        .iter()
        .flat_map(|section| section.changes.iter())
        .filter(|change| selected.contains(&change.id))
    {
        if !change.can_apply {
            return Err(format!(
                "Change {} is diagnostic-only and cannot be applied.",
                change.id
            ));
        }
        if change.section == "mappings" {
            apply_mapping_change(&mut mappings, change)?;
        } else {
            let document = match change.section.as_str() {
                "food" => &mut catalogues.food,
                "memories" => &mut catalogues.memories,
                "humans" => &mut catalogues.humans,
                other => return Err(format!("Unsupported sync section: {other}")),
            };
            apply_document_change(document, change)?;
        }
        applied_count += 1;
    }
    if applied_count == 0 {
        return Err("Select at least one applicable change.".to_string());
    }

    catalog::validate_document(&catalogues.food)?;
    catalog::validate_document(&catalogues.memories)?;
    catalog::validate_document(&catalogues.humans)?;
    let food_bytes = catalog::serialize_document(&catalogues.food)?;
    let memory_bytes = catalog::serialize_document(&catalogues.memories)?;
    let human_bytes = catalog::serialize_document(&catalogues.humans)?;
    let mapping_bytes = catalog::serialize_asset_mappings(&mappings, &catalogues)?;

    atomic_write(&catalog::catalogue_path(root, "food")?, &food_bytes, root)?;
    atomic_write(
        &catalog::catalogue_path(root, "memories")?,
        &memory_bytes,
        root,
    )?;
    atomic_write(
        &catalog::catalogue_path(root, "humans")?,
        &human_bytes,
        root,
    )?;
    atomic_write(&root.join("data/asset-mappings.json"), &mapping_bytes, root)?;

    Ok(SyncApplyResult {
        applied_count,
        catalogues: catalog::load_all(root)?,
        asset_mappings: catalog::load_asset_mappings(root)?,
    })
}

fn apply_document_change(document: &mut CsvDocument, change: &SyncChange) -> Result<(), String> {
    let proposed = change
        .proposed
        .as_ref()
        .ok_or_else(|| format!("Change {} has no proposed row.", change.id))?;
    match &change.current {
        Some(current) => {
            let Some(index) = document.rows.iter().position(|row| row == current) else {
                return Err(format!(
                    "{} changed after the scan. Scan again before applying.",
                    change.display_name
                ));
            };
            document.rows[index] = proposed.clone();
        }
        None => {
            let name_column = if document.kind == "humans" { 1 } else { 0 };
            if document
                .rows
                .iter()
                .any(|row| row[name_column].eq_ignore_ascii_case(&proposed[name_column]))
            {
                return Err(format!(
                    "{} already exists. Scan again before applying.",
                    change.display_name
                ));
            }
            document.rows.push(proposed.clone());
        }
    }
    Ok(())
}

fn apply_mapping_change(
    mappings: &mut BTreeMap<String, String>,
    change: &SyncChange,
) -> Result<(), String> {
    let proposed = change
        .proposed
        .as_ref()
        .filter(|row| row.len() == 2)
        .ok_or_else(|| format!("Change {} has no proposed mapping.", change.id))?;
    match &change.current {
        Some(current) if current.len() == 2 => {
            if mappings.get(&current[0]) != Some(&current[1]) {
                return Err(format!(
                    "Mapping {} changed after the scan. Scan again before applying.",
                    current[0]
                ));
            }
        }
        None if mappings.contains_key(&proposed[0]) => {
            return Err(format!(
                "Mapping {} already exists. Scan again before applying.",
                proposed[0]
            ));
        }
        _ => {}
    }
    mappings.insert(proposed[0].clone(), proposed[1].clone());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(kind: &str, headers: &[&str], rows: &[&[&str]]) -> CsvDocument {
        CsvDocument {
            kind: kind.to_string(),
            file_name: format!("{kind}.csv"),
            headers: headers.iter().map(|value| value.to_string()).collect(),
            rows: rows
                .iter()
                .map(|row| row.iter().map(|value| value.to_string()).collect())
                .collect(),
        }
    }

    fn empty_catalogues() -> CatalogueBundle {
        CatalogueBundle {
            food: document(
                "food",
                &[
                    "Food",
                    "Height",
                    "Intellect",
                    "Life Exp",
                    "Strength",
                    "Weight",
                    "TotalAvailability",
                ],
                &[],
            ),
            memories: document(
                "memories",
                &[
                    "Memory",
                    "Adaptability",
                    "Communication",
                    "Creativity",
                    "Discipline",
                    "Empathy",
                    "Focus",
                    "Leadership",
                    "Logic",
                    "Patience",
                    "Wisdom",
                    "WorldCount",
                ],
                &[],
            ),
            humans: document(
                "humans",
                &[
                    "Category",
                    "Profession",
                    "Weight",
                    "Height",
                    "Life Exp",
                    "Strength",
                    "Intellect",
                    "Adaptability",
                    "Creativity",
                    "Communication",
                    "Discipline",
                    "Empathy",
                    "Focus",
                    "Leadership",
                    "Logic",
                    "Patience",
                    "Wisdom",
                ],
                &[],
            ),
        }
    }

    #[test]
    fn reads_unreal_string_table_pairs() {
        let mut bytes = vec![0, 1, 0, 0, 0, 0];
        write_fstring(&mut bytes, "ST_Test");
        bytes.extend_from_slice(&2_i32.to_le_bytes());
        write_fstring(&mut bytes, "Food_A_Name");
        write_fstring(&mut bytes, "Alpha");
        write_fstring(&mut bytes, "Food_B_Name");
        write_fstring(&mut bytes, "Beta");
        assert_eq!(
            parse_string_table(&bytes),
            BTreeMap::from([
                ("Food_A_Name".to_string(), "Alpha".to_string()),
                ("Food_B_Name".to_string(), "Beta".to_string()),
            ])
        );
    }

    #[test]
    fn reads_known_unversioned_property_float_encodings() {
        let mut item = (-9_i32).to_le_bytes().to_vec();
        item.extend_from_slice(&[0x00, 0x05]);
        item.extend_from_slice(&10_f32.to_le_bytes());
        assert_eq!(find_property_float(&item, -9), Some(10));

        let mut profession = (-11_i32).to_le_bytes().to_vec();
        profession.extend_from_slice(&[0x80, 0x05, 0x02]);
        profession.extend_from_slice(&30_f32.to_le_bytes());
        assert_eq!(find_property_float(&profession, -11), Some(30));
    }

    #[test]
    fn creates_png_thumbnail_from_inline_bgra8_texture() {
        let width = 16_u32;
        let mut bytes = vec![0; 8];
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(b"PF_B8G8R8A8\0");
        bytes.extend_from_slice(&[0; 7]);
        for _ in 0..width * width {
            bytes.extend_from_slice(&[0x10, 0x20, 0x30, 0xff]);
        }
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&width.to_le_bytes());
        let data_url = decode_icon_thumbnail(&bytes).unwrap();
        assert!(data_url.starts_with("data:image/png;base64,iVBOR"));
    }

    #[test]
    fn verified_mapping_wins_over_installed_display_name() {
        let food = document(
            "food",
            &[
                "Food",
                "Height",
                "Intellect",
                "Life Exp",
                "Strength",
                "Weight",
                "TotalAvailability",
            ],
            &[["Local Pear", "1", "1", "1", "1", "1", "20"].as_slice()],
        );
        let memories = document(
            "memories",
            &[
                "Memory",
                "Adaptability",
                "Communication",
                "Creativity",
                "Discipline",
                "Empathy",
                "Focus",
                "Leadership",
                "Logic",
                "Patience",
                "Wisdom",
                "WorldCount",
            ],
            &[[
                "Travel Journal",
                "5",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "20",
            ]
            .as_slice()],
        );
        let humans = document(
            "humans",
            &[
                "Category",
                "Profession",
                "Weight",
                "Height",
                "Life Exp",
                "Strength",
                "Intellect",
                "Adaptability",
                "Creativity",
                "Communication",
                "Discipline",
                "Empathy",
                "Focus",
                "Leadership",
                "Logic",
                "Patience",
                "Wisdom",
            ],
            &[[
                "Engineer",
                "Engineer T1",
                "1",
                "1",
                "1",
                "1",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
            ]
            .as_slice()],
        );
        let bundle = CatalogueBundle {
            food,
            memories,
            humans,
        };
        let item = ExtractedItem {
            asset_name: "DA_Memory_Drawings_Notes".to_string(),
            kind: "memories".to_string(),
            display_name: "A different localized name".to_string(),
            localization_key: Some("Memory_Drawing_Notes_Name".to_string()),
            stats: BTreeMap::from([("Adaptability".to_string(), 10)]),
            unsupported_stats: Vec::new(),
            icon_asset: None,
            icon_data_url: None,
            is_development: false,
        };
        let proposal = compare_catalogues(
            Path::new("game"),
            Path::new("paks"),
            1,
            1,
            Vec::new(),
            &bundle,
            &BTreeMap::from([(
                "DA_Memory_Drawings_Notes".to_string(),
                "Travel Journal".to_string(),
            )]),
            &[item],
            &[],
        );
        let change = proposal.sections[1]
            .changes
            .iter()
            .find(|change| change.action == "changed")
            .unwrap();
        assert_eq!(change.display_name, "Travel Journal");
        assert_eq!(change.proposed.as_ref().unwrap()[1], "10");
        assert!(proposal.sections[3].changes.is_empty());
    }

    #[test]
    fn missing_entries_are_never_applicable() {
        let change = missing_change("food", "Old item", &["Old item".to_string()]);
        assert_eq!(change.action, "missing");
        assert!(!change.can_apply);
        assert!(!change.selected_by_default);
        assert!(change.proposed.is_none());
    }

    #[test]
    fn conflicting_development_alias_does_not_duplicate_a_catalogue_row() {
        let items = [
            ExtractedItem {
                asset_name: "DA_Memory_Books_Encyclopedia".to_string(),
                kind: "memories".to_string(),
                display_name: "Encyclopedia".to_string(),
                localization_key: None,
                stats: BTreeMap::from([("Logic".to_string(), 5)]),
                unsupported_stats: Vec::new(),
                icon_asset: None,
                icon_data_url: None,
                is_development: false,
            },
            ExtractedItem {
                asset_name: "DA_Memory_Encyclopedia2".to_string(),
                kind: "memories".to_string(),
                display_name: "Encyclopedia".to_string(),
                localization_key: None,
                stats: BTreeMap::from([("Logic".to_string(), 40)]),
                unsupported_stats: Vec::new(),
                icon_asset: None,
                icon_data_url: None,
                is_development: true,
            },
        ];
        let proposal = compare_catalogues(
            Path::new("game"),
            Path::new("paks"),
            1,
            2,
            Vec::new(),
            &empty_catalogues(),
            &BTreeMap::new(),
            &items,
            &[],
        );
        let changes = &proposal.sections[1].changes;
        assert_eq!(
            changes
                .iter()
                .filter(|change| change.action == "added")
                .count(),
            1
        );
        assert_eq!(
            changes
                .iter()
                .filter(|change| change.action == "conflict")
                .count(),
            1
        );
        assert_eq!(changes.iter().filter(|change| change.can_apply).count(), 1);
    }

    #[test]
    fn complete_profession_categories_can_bootstrap_an_empty_catalogue() {
        let mut humans = (1..=4)
            .map(|level| ExtractedHuman {
                asset_name: format!("DA_Profession_Test_{level}"),
                category: "Test".to_string(),
                display_name: format!("Test Profession {level}"),
                requirements: BTreeMap::from([
                    ("Weight".to_string(), 20),
                    ("Height".to_string(), 30),
                    ("Life Exp".to_string(), 10),
                    ("Logic".to_string(), level * 10),
                ]),
                unsupported_stats: Vec::new(),
                tier: None,
            })
            .collect::<Vec<_>>();
        assign_human_tiers(&mut humans);
        let proposal = compare_catalogues(
            Path::new("game"),
            Path::new("paks"),
            1,
            4,
            Vec::new(),
            &empty_catalogues(),
            &BTreeMap::new(),
            &[],
            &humans,
        );
        let changes = &proposal.sections[2].changes;
        assert_eq!(changes.len(), 4);
        assert!(
            changes
                .iter()
                .all(|change| change.action == "added" && change.can_apply)
        );
        assert_eq!(
            changes[0].proposed.as_ref().unwrap()[1],
            "Test Profession 1 T1"
        );
        assert_eq!(
            changes[3].proposed.as_ref().unwrap()[1],
            "Test Profession 4 T4"
        );
    }

    #[test]
    #[ignore = "requires a locally installed copy of The Last Caretaker"]
    fn installed_game_scan_is_consistent_when_available() {
        let game = super::super::settings::default_game_directory()
            .expect("installed Steam game was not discovered");
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let paks = resolve_paks_path(&game).unwrap();
        let (items, humans, _, _, _) = extract_catalogue(&paks).unwrap();
        let proposal = scan_and_compare(root, game.to_str()).unwrap();
        for section in &proposal.sections {
            let counts = section
                .changes
                .iter()
                .fold(BTreeMap::new(), |mut counts, change| {
                    *counts.entry(change.action.as_str()).or_insert(0_usize) += 1;
                    counts
                });
            println!("{}: {:?}", section.kind, counts);
            for change in section
                .changes
                .iter()
                .filter(|change| change.action == "changed")
                .take(3)
            {
                println!("  {}: {}", change.display_name, change.summary);
            }
        }
        assert!(proposal.source.extracted_count >= 80);
        assert_eq!(proposal.sections.len(), 4);
        let decoded_icons = proposal
            .sections
            .iter()
            .flat_map(|section| &section.changes)
            .filter(|change| change.icon_data_url.is_some())
            .count();
        println!("decoded icons in proposal: {decoded_icons}");
        assert!(decoded_icons >= 10);
        let empty_proposal = compare_catalogues(
            &game,
            &paks,
            proposal.source.package_count,
            proposal.source.extracted_count,
            Vec::new(),
            &empty_catalogues(),
            &BTreeMap::new(),
            &items,
            &humans,
        );
        assert_eq!(
            empty_proposal.sections[2]
                .changes
                .iter()
                .filter(|change| change.action == "added" && change.can_apply)
                .count(),
            40
        );
        for section in &empty_proposal.sections {
            let applicable_names = section
                .changes
                .iter()
                .filter(|change| change.can_apply && change.section != "mappings")
                .filter_map(|change| change.proposed.as_ref())
                .map(|row| {
                    let column = usize::from(section.kind == "humans");
                    row[column].to_ascii_lowercase()
                })
                .collect::<Vec<_>>();
            assert_eq!(
                applicable_names.iter().collect::<HashSet<_>>().len(),
                applicable_names.len(),
                "{} proposed duplicate catalogue names",
                section.kind
            );
        }
        let test_root = root.join("src-tauri/target/game-sync-empty-catalogues-test");
        if test_root.exists() {
            std::fs::remove_dir_all(&test_root).unwrap();
        }
        std::fs::create_dir_all(test_root.join("data")).unwrap();
        std::fs::create_dir_all(test_root.join("backups")).unwrap();
        let blank_catalogues = empty_catalogues();
        for document in [
            &blank_catalogues.food,
            &blank_catalogues.memories,
            &blank_catalogues.humans,
        ] {
            std::fs::write(
                catalog::catalogue_path(&test_root, &document.kind).unwrap(),
                catalog::serialize_document(document).unwrap(),
            )
            .unwrap();
        }
        std::fs::write(test_root.join("data/asset-mappings.json"), b"{}").unwrap();
        let selected_ids = empty_proposal
            .sections
            .iter()
            .flat_map(|section| &section.changes)
            .filter(|change| change.selected_by_default)
            .map(|change| change.id.clone())
            .collect::<Vec<_>>();
        let applied = apply_proposal(&test_root, &empty_proposal, &selected_ids).unwrap();
        assert_eq!(applied.applied_count, selected_ids.len());
        assert_eq!(applied.catalogues.humans.rows.len(), 40);
        let expected_humans = catalog::load_all(root)
            .unwrap()
            .humans
            .rows
            .into_iter()
            .collect::<HashSet<_>>();
        let imported_humans = applied
            .catalogues
            .humans
            .rows
            .into_iter()
            .collect::<HashSet<_>>();
        assert_eq!(imported_humans, expected_humans);
        std::fs::remove_dir_all(test_root).unwrap();
        assert!(
            proposal
                .sections
                .iter()
                .flat_map(|section| &section.changes)
                .all(|change| !change.summary.contains("→ 0"))
        );
    }

    fn write_fstring(bytes: &mut Vec<u8>, value: &str) {
        bytes.extend_from_slice(&((value.len() + 1) as i32).to_le_bytes());
        bytes.extend_from_slice(value.as_bytes());
        bytes.push(0);
    }
}

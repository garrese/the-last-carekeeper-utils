use super::{ImportedItem, InventoryReport, InventorySource, Settings};
use flate2::read::ZlibDecoder;
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const WRAPPER_MAGIC: [u8; 8] = [0xc1, 0x83, 0x2a, 0x9e, 0x22, 0x22, 0x22, 0x22];
const HEADER_SIZE: usize = 49;
const MAX_COMPRESSED_BYTES: u64 = 1_073_741_824;
const MAX_RAW_BYTES: usize = 536_870_912;
const CHARACTER_CLASS: &str =
    "/Game/Blueprints/BP_FirstPersonCharacter_New.BP_FirstPersonCharacter_New_C";
const PLAYER_BOX_CLASS: &str =
    "/Game/Blueprints/Interactives/Containers/BP_Inventory_PlayerBox.BP_Inventory_PlayerBox_C";
const PLAYER_BOX_SMALL_CLASS: &str = "/Game/Blueprints/Interactives/Containers/BP_Inventory_PlayerBox_Small.BP_Inventory_PlayerBox_Small_C";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Fingerprint {
    length: u64,
    modified: Option<SystemTime>,
}

#[derive(Debug, Clone)]
struct TypeName {
    name: String,
}

#[derive(Debug, Clone)]
struct Property {
    name: String,
    type_name: String,
    data_start: usize,
    data_end: usize,
}

#[derive(Debug, Clone)]
struct ActorRecord {
    offset: usize,
    end: usize,
    class_path: String,
    actor_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RawItem {
    asset_name: String,
    quantity: u32,
}

#[derive(Debug, Clone)]
struct ParsedSource {
    id: String,
    label: String,
    kind: String,
    items: Vec<RawItem>,
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
    end: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], offset: usize, end: usize) -> Result<Self, String> {
        if offset > end || end > bytes.len() {
            return Err("Reader range is outside the save data.".to_string());
        }
        Ok(Self { bytes, offset, end })
    }

    fn ensure(&self, count: usize) -> Result<(), String> {
        if self
            .offset
            .checked_add(count)
            .is_none_or(|end| end > self.end)
        {
            return Err(format!("Read outside save data at 0x{:x}.", self.offset));
        }
        Ok(())
    }

    fn u8(&mut self) -> Result<u8, String> {
        self.ensure(1)?;
        let value = self.bytes[self.offset];
        self.offset += 1;
        Ok(value)
    }

    fn i32(&mut self) -> Result<i32, String> {
        self.ensure(4)?;
        let value =
            i32::from_le_bytes(self.bytes[self.offset..self.offset + 4].try_into().unwrap());
        self.offset += 4;
        Ok(value)
    }

    fn u32(&mut self) -> Result<u32, String> {
        self.ensure(4)?;
        let value =
            u32::from_le_bytes(self.bytes[self.offset..self.offset + 4].try_into().unwrap());
        self.offset += 4;
        Ok(value)
    }

    fn skip(&mut self, count: usize) -> Result<(), String> {
        self.ensure(count)?;
        self.offset += count;
        Ok(())
    }

    fn string(&mut self) -> Result<String, String> {
        let length = self.i32()?;
        if length == 0 {
            return Ok(String::new());
        }
        if length > 0 {
            let byte_count =
                usize::try_from(length).map_err(|_| "Invalid string length.".to_string())?;
            if byte_count > 1_048_576 {
                return Err(format!(
                    "Unreasonable string length at 0x{:x}.",
                    self.offset - 4
                ));
            }
            self.ensure(byte_count)?;
            let payload = &self.bytes[self.offset..self.offset + byte_count.saturating_sub(1)];
            self.offset += byte_count;
            return String::from_utf8(payload.to_vec())
                .map_err(|_| "Invalid UTF-8 string in save.".to_string());
        }
        let characters = usize::try_from(-i64::from(length))
            .map_err(|_| "Invalid UTF-16 string length.".to_string())?;
        if characters > 524_288 {
            return Err(format!(
                "Unreasonable UTF-16 string length at 0x{:x}.",
                self.offset - 4
            ));
        }
        let byte_count = characters
            .checked_mul(2)
            .ok_or_else(|| "UTF-16 string length overflow.".to_string())?;
        self.ensure(byte_count)?;
        let payload = &self.bytes[self.offset..self.offset + byte_count.saturating_sub(2)];
        self.offset += byte_count;
        let (pairs, remainder) = payload.as_chunks::<2>();
        debug_assert!(remainder.is_empty());
        let units = pairs
            .iter()
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        String::from_utf16(&units).map_err(|_| "Invalid UTF-16 string in save.".to_string())
    }
}

fn read_type_name(reader: &mut Reader<'_>, depth: usize) -> Result<TypeName, String> {
    if depth > 12 {
        return Err("Property type is nested too deeply.".to_string());
    }
    let name = reader.string()?;
    let parameter_count = reader.i32()?;
    if !(0..=12).contains(&parameter_count) {
        return Err(format!("Invalid type parameter count for {name}."));
    }
    for _ in 0..parameter_count {
        read_type_name(reader, depth + 1)?;
    }
    Ok(TypeName { name })
}

fn read_property_at(bytes: &[u8], offset: usize, limit: usize) -> Result<Property, String> {
    let mut reader = Reader::new(bytes, offset, limit)?;
    let name = reader.string()?;
    let property_type = read_type_name(&mut reader, 0)?;
    let size = usize::try_from(reader.u32()?).map_err(|_| "Property size overflow.".to_string())?;
    if property_type.name == "BoolProperty" {
        reader.u8()?;
    }
    let has_guid = reader.u8()?;
    if has_guid != 0 {
        reader.skip(16)?;
    }
    let data_start = reader.offset;
    let data_end = data_start
        .checked_add(size)
        .ok_or_else(|| "Property end overflow.".to_string())?;
    if data_end > limit {
        return Err(format!("Property {name} exceeds its actor record."));
    }
    Ok(Property {
        name,
        type_name: property_type.name,
        data_start,
        data_end,
    })
}

fn property_string(bytes: &[u8], property: &Property) -> Result<String, String> {
    let mut reader = Reader::new(bytes, property.data_start, property.data_end)?;
    match property.type_name.as_str() {
        "StrProperty" | "NameProperty" | "ObjectProperty" | "ClassProperty" => reader.string(),
        "TextProperty" => {
            reader.u32()?;
            let history_type = reader.u8()?;
            if history_type != 0xff {
                return Err("Unsupported localized text representation.".to_string());
            }
            reader.i32()?;
            reader.string()
        }
        _ => Err(format!(
            "Property {} is not a supported string type.",
            property.name
        )),
    }
}

fn property_int(bytes: &[u8], property: &Property) -> Result<i32, String> {
    if property.type_name != "IntProperty" || property.data_end - property.data_start != 4 {
        return Err(format!("Property {} is not an IntProperty.", property.name));
    }
    Ok(i32::from_le_bytes(
        bytes[property.data_start..property.data_end]
            .try_into()
            .unwrap(),
    ))
}

fn fstring_pattern(value: &str) -> Vec<u8> {
    let mut pattern = Vec::with_capacity(value.len() + 5);
    pattern.extend_from_slice(&i32::try_from(value.len() + 1).unwrap().to_le_bytes());
    pattern.extend_from_slice(value.as_bytes());
    pattern.push(0);
    pattern
}

fn find_all(bytes: &[u8], pattern: &[u8], start: usize, end: usize) -> Vec<usize> {
    if pattern.is_empty() || start >= end || end > bytes.len() || pattern.len() > end - start {
        return Vec::new();
    }
    bytes[start..end]
        .windows(pattern.len())
        .enumerate()
        .filter_map(|(index, window)| (window == pattern).then_some(start + index))
        .collect()
}

fn find_actor_records(raw: &[u8]) -> Vec<ActorRecord> {
    let class_marker = fstring_pattern("ActorClass");
    let name_marker = fstring_pattern("ActorName");
    let mut records = Vec::new();
    for offset in find_all(raw, &class_marker, 0, raw.len()) {
        let Ok(class_property) = read_property_at(raw, offset, raw.len()) else {
            continue;
        };
        let Ok(class_path) = property_string(raw, &class_property) else {
            continue;
        };
        if !class_path.starts_with("/Game/Blueprints/") {
            continue;
        }
        let search_end = raw.len().min(class_property.data_end.saturating_add(1024));
        let actor_name = find_all(raw, &name_marker, class_property.data_end, search_end)
            .into_iter()
            .find_map(|name_offset| {
                let property = read_property_at(raw, name_offset, search_end).ok()?;
                property_string(raw, &property).ok()
            })
            .unwrap_or_else(|| format!("actor-{}", records.len() + 1));
        records.push(ActorRecord {
            offset,
            end: raw.len(),
            class_path,
            actor_name,
        });
    }
    records.sort_by_key(|record| record.offset);
    for index in 0..records.len() {
        records[index].end = records.get(index + 1).map_or(raw.len(), |next| next.offset);
    }
    records
}

fn scan_items(raw: &[u8], start: usize, end: usize) -> Vec<RawItem> {
    let asset_marker = fstring_pattern("AssetName");
    let count_marker = fstring_pattern("ItemCount");
    let asset_offsets = find_all(raw, &asset_marker, start, end);
    let mut items = Vec::new();
    for (index, offset) in asset_offsets.iter().copied().enumerate() {
        let Ok(asset_property) = read_property_at(raw, offset, end) else {
            continue;
        };
        let Ok(asset_name) = property_string(raw, &asset_property) else {
            continue;
        };
        if !asset_name.starts_with("DA_Food_") && !asset_name.starts_with("DA_Memory_") {
            continue;
        }
        let next_asset = asset_offsets.get(index + 1).copied().unwrap_or(end);
        let count_end = end
            .min(next_asset)
            .min(asset_property.data_end.saturating_add(4096));
        let quantity = find_all(raw, &count_marker, asset_property.data_end, count_end)
            .into_iter()
            .find_map(|count_offset| {
                let property = read_property_at(raw, count_offset, count_end).ok()?;
                let value = property_int(raw, &property).ok()?;
                u32::try_from(value).ok()
            })
            .unwrap_or(1);
        items.push(RawItem {
            asset_name,
            quantity,
        });
    }

    items
}

fn deduplicate_exact_full_mirror(items: &mut Vec<RawItem>) -> bool {
    let mirrored = items.len() >= 2
        && items.len().is_multiple_of(2)
        && items[..items.len() / 2] == items[items.len() / 2..];
    if mirrored {
        items.truncate(items.len() / 2);
    }
    mirrored
}

fn find_backpack_items_range(raw: &[u8], start: usize, end: usize) -> Option<(usize, usize)> {
    let inventory_table =
        fstring_pattern("/Game/LocalizationStringTables/ST_Inventory.ST_Inventory");
    let backpack = fstring_pattern("Backpack");
    let byte_data = fstring_pattern("ByteData");
    let items = fstring_pattern("Items");

    find_all(raw, &backpack, start, end)
        .into_iter()
        .find_map(|offset| {
            let context_start = offset.saturating_sub(1024).max(start);
            if find_all(raw, &inventory_table, context_start, offset).is_empty() {
                return None;
            }

            // The localized Backpack descriptor is followed by one ByteData
            // payload for its descriptor and a second one for its live state.
            // The canonical Items map is nested in that second payload. A third
            // ByteData payload in the same character actor contains unrelated
            // historical item records, so scanning to the actor end is unsafe.
            let descriptor_context_end = offset.saturating_add(4096).min(end);
            let backpack_payload = find_all(raw, &byte_data, offset, descriptor_context_end)
                .into_iter()
                .filter_map(|property_offset| {
                    let property = read_property_at(raw, property_offset, end).ok()?;
                    (property.name == "ByteData" && property.type_name == "ArrayProperty")
                        .then_some(property)
                })
                .nth(1)?;

            find_all(
                raw,
                &items,
                backpack_payload.data_start,
                backpack_payload.data_end,
            )
            .into_iter()
            .find_map(|property_offset| {
                let property =
                    read_property_at(raw, property_offset, backpack_payload.data_end).ok()?;
                (property.name == "Items" && property.type_name == "MapProperty")
                    .then_some((property.data_start, property.data_end))
            })
        })
}

fn find_chest_label(raw: &[u8], start: usize, end: usize) -> Option<String> {
    let marker = fstring_pattern("Name");
    find_all(raw, &marker, start, end)
        .into_iter()
        .find_map(|offset| {
            let property = read_property_at(raw, offset, end).ok()?;
            if property.name != "Name" || property.type_name != "TextProperty" {
                return None;
            }
            let value = property_string(raw, &property).ok()?.trim().to_string();
            (!value.is_empty() && value.len() <= 256).then_some(value)
        })
}

fn parse_sources(raw: &[u8]) -> Result<(Vec<ParsedSource>, Vec<String>), String> {
    if raw.len() < 8 || &raw[4..8] != b"GVAS" {
        return Err("The decompressed data does not contain the expected GVAS header.".to_string());
    }
    let actors = find_actor_records(raw);
    let mut sources = Vec::new();
    let mut warnings = Vec::new();
    for actor in actors.iter().filter(|actor| {
        actor.class_path == CHARACTER_CLASS
            || actor.class_path == PLAYER_BOX_CLASS
            || actor.class_path == PLAYER_BOX_SMALL_CLASS
    }) {
        if actor.class_path == CHARACTER_CLASS {
            let items = if let Some((backpack_start, backpack_end)) =
                find_backpack_items_range(raw, actor.offset, actor.end)
            {
                scan_items(raw, backpack_start, backpack_end)
            } else {
                warnings.push(
                    "The canonical Backpack section was not found; the character actor was scanned as a compatibility fallback."
                        .to_string(),
                );
                let mut fallback = scan_items(raw, actor.offset, actor.end);
                deduplicate_exact_full_mirror(&mut fallback);
                fallback
            };
            sources.push(ParsedSource {
                id: format!("backpack:{}", actor.actor_name),
                label: "Player backpack".to_string(),
                kind: "backpack".to_string(),
                items,
            });
            continue;
        }
        let mut items = scan_items(raw, actor.offset, actor.end);
        let mirrored = deduplicate_exact_full_mirror(&mut items);
        let custom_name = find_chest_label(raw, actor.offset, actor.end);
        if !items.is_empty() && !mirrored {
            warnings.push(format!(
                "Chest {} contains a non-mirrored item sequence; quantities were preserved without deduplication.",
                custom_name.as_deref().unwrap_or(&actor.actor_name)
            ));
        }
        sources.push(ParsedSource {
            id: format!("player-box:{}", actor.actor_name),
            label: custom_name.unwrap_or_else(|| actor.actor_name.clone()),
            kind: "player-box".to_string(),
            items,
        });
    }
    if !sources.iter().any(|source| source.kind == "backpack") {
        return Err(
            "The player backpack could not be identified in this save version.".to_string(),
        );
    }
    Ok((sources, warnings))
}

fn read_u24(bytes: &[u8], offset: usize) -> Result<usize, String> {
    if offset + 3 > bytes.len() {
        return Err("Truncated block size in save header.".to_string());
    }
    Ok(usize::from(bytes[offset])
        | (usize::from(bytes[offset + 1]) << 8)
        | (usize::from(bytes[offset + 2]) << 16))
}

fn decompress_save(compressed: &[u8]) -> Result<(Vec<u8>, usize), String> {
    let mut raw = Vec::new();
    let mut offset = 0usize;
    let mut block_count = 0usize;
    while offset < compressed.len() {
        if offset
            .checked_add(HEADER_SIZE)
            .is_none_or(|end| end > compressed.len())
        {
            return Err(format!("Truncated save block header at 0x{offset:x}."));
        }
        if compressed[offset..offset + 8] != WRAPPER_MAGIC {
            return Err(format!("Unrecognized save block header at 0x{offset:x}."));
        }
        let compressed_size = read_u24(compressed, offset + 17)?;
        let uncompressed_size = read_u24(compressed, offset + 25)?;
        let payload_start = offset + HEADER_SIZE;
        let payload_end = payload_start
            .checked_add(compressed_size)
            .ok_or_else(|| "Compressed block size overflow.".to_string())?;
        if payload_end > compressed.len() {
            return Err(format!(
                "Compressed block at 0x{offset:x} exceeds the save file."
            ));
        }
        if raw.len().saturating_add(uncompressed_size) > MAX_RAW_BYTES {
            return Err("The decompressed save exceeds the 512 MiB safety limit.".to_string());
        }
        let mut decoder = ZlibDecoder::new(&compressed[payload_start..payload_end]);
        let before = raw.len();
        decoder
            .read_to_end(&mut raw)
            .map_err(|error| format!("Could not decompress block at 0x{offset:x}: {error}"))?;
        if raw.len() - before != uncompressed_size {
            return Err(format!(
                "Block at 0x{offset:x} has an unexpected decompressed size."
            ));
        }
        offset = payload_end;
        block_count += 1;
    }
    Ok((raw, block_count))
}

fn metadata_fingerprint(path: &Path) -> Result<Fingerprint, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Could not inspect {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err("The selected save path is not a file.".to_string());
    }
    if metadata.len() == 0 {
        return Err("The selected save file is empty.".to_string());
    }
    if metadata.len() > MAX_COMPRESSED_BYTES {
        return Err("The selected save exceeds the 1 GiB safety limit.".to_string());
    }
    Ok(Fingerprint {
        length: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

fn read_stable_snapshot(path: &Path) -> Result<(Vec<u8>, Fingerprint), String> {
    let mut last_reason = "the file changed while it was being read".to_string();
    for _ in 0..4 {
        let before = metadata_fingerprint(path)?;
        thread::sleep(Duration::from_millis(150));
        match fs::read(path) {
            Ok(bytes) => {
                let after = metadata_fingerprint(path)?;
                if before == after && bytes.len() as u64 == after.length {
                    return Ok((bytes, after));
                }
                last_reason = "the game was still writing the save".to_string();
            }
            Err(error) => {
                last_reason = format!("the save was temporarily unavailable: {error}");
            }
        }
        thread::sleep(Duration::from_millis(350));
    }
    Err(format!(
        "Could not obtain a stable read-only snapshot because {last_reason}. Wait for the game save to finish and try again."
    ))
}

pub fn import_inventory(root: &Path, settings: &Settings) -> Result<InventoryReport, String> {
    let save_path = settings
        .save_path
        .as_deref()
        .ok_or_else(|| "Select a .sav file first.".to_string())?;
    let path = Path::new(save_path);
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("sav"))
    {
        return Err("The selected file must have a .sav extension.".to_string());
    }
    let (compressed, fingerprint) = read_stable_snapshot(path)?;
    let (raw, block_count) = decompress_save(&compressed)?;
    let (parsed_sources, mut warnings) = parse_sources(&raw)?;
    let mappings = super::catalog::load_asset_mappings(root)?;

    let discovered_chests = parsed_sources
        .iter()
        .filter(|source| source.kind == "player-box")
        .map(|source| source.label.clone())
        .collect::<Vec<_>>();
    let missing_chests = settings
        .chest_names
        .iter()
        .filter(|configured| {
            !discovered_chests
                .iter()
                .any(|found| found.eq_ignore_ascii_case(configured.trim()))
        })
        .cloned()
        .collect::<Vec<_>>();
    if !missing_chests.is_empty() {
        warnings.push(format!(
            "Configured chest names not found: {}",
            missing_chests.join(", ")
        ));
    }

    let selected_sources = parsed_sources.into_iter().filter(|source| {
        source.kind == "backpack"
            || settings
                .chest_names
                .iter()
                .any(|configured| source.label.eq_ignore_ascii_case(configured.trim()))
    });
    let mut sources = Vec::new();
    let mut inventory = BTreeMap::new();
    let mut unresolved_assets = Vec::new();
    for source in selected_sources {
        let mut aggregate = BTreeMap::<String, u32>::new();
        for item in source.items {
            *aggregate.entry(item.asset_name).or_default() += item.quantity;
        }
        let items = aggregate
            .into_iter()
            .map(|(asset_name, quantity)| {
                let mapped_name = mappings.get(&asset_name).cloned();
                if let Some(name) = &mapped_name {
                    *inventory.entry(name.clone()).or_default() += quantity;
                } else if !unresolved_assets.contains(&asset_name) {
                    unresolved_assets.push(asset_name.clone());
                }
                ImportedItem {
                    asset_name,
                    mapped_name,
                    quantity,
                }
            })
            .collect();
        sources.push(InventorySource {
            id: source.id,
            label: source.label,
            kind: source.kind,
            items,
        });
    }
    unresolved_assets.sort();
    let modified_unix_ms = fingerprint
        .modified
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_millis());
    Ok(InventoryReport {
        save_path: save_path.to_string(),
        file_size: fingerprint.length,
        modified_unix_ms,
        raw_bytes: raw.len(),
        block_count,
        sources,
        discovered_chests,
        missing_chests,
        inventory,
        unresolved_assets,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_mirror_is_deduplicated() {
        let left = vec![
            RawItem {
                asset_name: "A".into(),
                quantity: 2,
            },
            RawItem {
                asset_name: "B".into(),
                quantity: 1,
            },
        ];
        let mut all = left.clone();
        all.extend(left.clone());
        assert!(deduplicate_exact_full_mirror(&mut all));
        assert_eq!(all, left);
    }

    #[test]
    fn non_mirrored_items_are_preserved() {
        let mut items = vec![
            RawItem {
                asset_name: "A".into(),
                quantity: 2,
            },
            RawItem {
                asset_name: "A".into(),
                quantity: 1,
            },
        ];
        let original = items.clone();
        assert!(!deduplicate_exact_full_mirror(&mut items));
        assert_eq!(items, original);
    }

    fn push_simple_type(bytes: &mut Vec<u8>, name: &str) {
        bytes.extend_from_slice(&fstring_pattern(name));
        bytes.extend_from_slice(&0i32.to_le_bytes());
    }

    fn push_simple_property(bytes: &mut Vec<u8>, name: &str, property_type: &str, data: &[u8]) {
        bytes.extend_from_slice(&fstring_pattern(name));
        push_simple_type(bytes, property_type);
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.push(0);
        bytes.extend_from_slice(data);
    }

    fn push_byte_data(bytes: &mut Vec<u8>, data: &[u8]) -> (usize, usize) {
        bytes.extend_from_slice(&fstring_pattern("ByteData"));
        bytes.extend_from_slice(&fstring_pattern("ArrayProperty"));
        bytes.extend_from_slice(&1i32.to_le_bytes());
        bytes.extend_from_slice(&fstring_pattern("ByteProperty"));
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.push(0);
        let data_start = bytes.len();
        bytes.extend_from_slice(data);
        (data_start, bytes.len())
    }

    fn serialized_item(asset_name: &str, quantity: i32) -> Vec<u8> {
        let mut bytes = Vec::new();
        push_simple_property(
            &mut bytes,
            "AssetName",
            "NameProperty",
            &fstring_pattern(asset_name),
        );
        push_simple_property(
            &mut bytes,
            "ItemCount",
            "IntProperty",
            &quantity.to_le_bytes(),
        );
        bytes
    }

    fn push_items_map(bytes: &mut Vec<u8>, data: &[u8]) -> (usize, usize) {
        bytes.extend_from_slice(&fstring_pattern("Items"));
        bytes.extend_from_slice(&fstring_pattern("MapProperty"));
        bytes.extend_from_slice(&2i32.to_le_bytes());
        bytes.extend_from_slice(&fstring_pattern("IntProperty"));
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&fstring_pattern("StructProperty"));
        bytes.extend_from_slice(&1i32.to_le_bytes());
        bytes.extend_from_slice(&fstring_pattern("VoyageItemSerialize"));
        bytes.extend_from_slice(&1i32.to_le_bytes());
        bytes.extend_from_slice(&fstring_pattern("/Script/Voyage"));
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.push(0);
        let data_start = bytes.len();
        bytes.extend_from_slice(data);
        (data_start, bytes.len())
    }

    #[test]
    fn canonical_backpack_range_requires_inventory_context() {
        let mut raw = fstring_pattern("Backpack");
        raw.extend_from_slice(&[0; 32]);
        let inventory_offset = raw.len();
        raw.extend_from_slice(&fstring_pattern(
            "/Game/LocalizationStringTables/ST_Inventory.ST_Inventory",
        ));
        let backpack_offset = raw.len();
        raw.extend_from_slice(&fstring_pattern("Backpack"));
        push_byte_data(&mut raw, &[]);
        let mut backpack_payload = Vec::new();
        let inner_range = push_items_map(&mut backpack_payload, &[]);
        let payload_range = push_byte_data(&mut raw, &backpack_payload);
        let expected = (
            payload_range.0 + inner_range.0,
            payload_range.0 + inner_range.1,
        );

        assert!(inventory_offset < backpack_offset);
        assert_eq!(
            find_backpack_items_range(&raw, 0, raw.len()),
            Some(expected)
        );
    }

    #[test]
    fn canonical_backpack_range_excludes_later_character_item_maps() {
        let mut raw = fstring_pattern("/Game/LocalizationStringTables/ST_Inventory.ST_Inventory");
        raw.extend_from_slice(&fstring_pattern("Backpack"));
        push_byte_data(&mut raw, &[]);
        let backpack_item = serialized_item("DA_Memory_Drawings_Cards", 2);
        let mut backpack_payload = Vec::new();
        let inner_range = push_items_map(&mut backpack_payload, &backpack_item);
        push_simple_property(
            &mut backpack_payload,
            "Resources",
            "IntProperty",
            &0i32.to_le_bytes(),
        );
        let payload_range = push_byte_data(&mut raw, &backpack_payload);
        let expected = (
            payload_range.0 + inner_range.0,
            payload_range.0 + inner_range.1,
        );
        let historical_food = serialized_item("DA_Food_High-FatEnergy", 1);
        let mut historical_payload = Vec::new();
        push_items_map(&mut historical_payload, &historical_food);
        push_byte_data(&mut raw, &historical_payload);

        let range = find_backpack_items_range(&raw, 0, raw.len()).unwrap();
        assert_eq!(range, expected);
        assert_eq!(
            scan_items(&raw, range.0, range.1),
            vec![RawItem {
                asset_name: "DA_Memory_Drawings_Cards".into(),
                quantity: 2,
            }]
        );
    }

    #[test]
    fn verified_sample_matches_inventory_evidence_when_available() {
        let path = std::env::var_os("TLC_TEST_SAVE")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("USERPROFILE").map(|home| {
                    std::path::PathBuf::from(home).join("Downloads/76561198072091332_0.1.sav")
                })
            });
        let Some(path) = path.filter(|path| path.is_file()) else {
            return;
        };
        let compressed = fs::read(path).unwrap();
        let (raw, blocks) = decompress_save(&compressed).unwrap();
        assert!(blocks > 0);
        assert_eq!(raw.len(), 59_821_774);
        let (sources, warnings) = parse_sources(&raw).unwrap();
        assert!(!warnings.iter().any(|warning| warning.contains("fallback")));
        let backpack = sources
            .iter()
            .find(|source| source.kind == "backpack")
            .unwrap();
        let backpack_items = backpack
            .items
            .iter()
            .map(|item| (item.asset_name.as_str(), item.quantity))
            .collect::<BTreeMap<_, _>>();
        assert!(backpack_items.is_empty(), "{backpack_items:?}");
        let chest = sources
            .iter()
            .find(|source| source.label == "PruebaItems001")
            .expect("named test chest");
        assert_eq!(chest.items.len(), 26);
        assert_eq!(
            chest
                .items
                .iter()
                .find(|item| item.asset_name == "DA_Food_Nutri-Core")
                .map(|item| item.quantity),
            Some(9)
        );
        assert_eq!(
            chest
                .items
                .iter()
                .find(|item| item.asset_name == "DA_Food_High-FatEnergy")
                .map(|item| item.quantity),
            Some(4)
        );
        assert_eq!(
            chest
                .items
                .iter()
                .find(|item| item.asset_name == "DA_Food_Mind_Surge")
                .map(|item| item.quantity),
            Some(5)
        );
        assert_eq!(
            chest
                .items
                .iter()
                .find(|item| item.asset_name == "DA_Food_PhysiqueFuel")
                .map(|item| item.quantity),
            Some(4)
        );
    }

    #[test]
    fn previous_sample_does_not_contain_the_new_chest_name_when_available() {
        let Some(path) = std::env::var_os("USERPROFILE")
            .map(|home| std::path::PathBuf::from(home).join("Downloads/76561198072091332_0.sav"))
            .filter(|path| path.is_file())
        else {
            return;
        };
        let compressed = fs::read(path).unwrap();
        let (raw, blocks) = decompress_save(&compressed).unwrap();
        assert!(blocks > 0);
        assert_eq!(raw.len(), 58_750_556);
        let (sources, _) = parse_sources(&raw).unwrap();
        assert!(
            !sources
                .iter()
                .any(|source| source.label == "PruebaItems001")
        );
    }

    #[test]
    fn moved_memories_are_counted_once_when_current_save_is_available() {
        let Some(path) = std::env::var_os("TLC_BACKPACK_MIRROR_SAVE")
            .map(std::path::PathBuf::from)
            .filter(|path| path.is_file())
        else {
            return;
        };
        let compressed = fs::read(path).unwrap();
        let (raw, _) = decompress_save(&compressed).unwrap();
        let (sources, warnings) = parse_sources(&raw).unwrap();
        let backpack = sources
            .iter()
            .find(|source| source.kind == "backpack")
            .unwrap();
        let backpack_items = backpack
            .items
            .iter()
            .map(|item| (item.asset_name.as_str(), item.quantity))
            .collect::<BTreeMap<_, _>>();

        assert_eq!(backpack_items.get("DA_Memory_Drawings_Notes"), Some(&1));
        assert_eq!(backpack_items.get("DA_Memory_Drawings_Cards"), Some(&2));
        assert_eq!(backpack_items.len(), 11);
        assert!(!warnings.iter().any(|warning| warning.contains("fallback")));
    }
}

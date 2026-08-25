use super::{CatalogueBundle, CsvDocument};
use crate::domain::settings::atomic_write;
use csv::{ReaderBuilder, WriterBuilder};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub fn catalogue_path(root: &Path, kind: &str) -> Result<PathBuf, String> {
    let file_name = match kind {
        "food" => "Food.csv",
        "memories" => "Memories.csv",
        "humans" => "Humans.csv",
        _ => return Err(format!("Unknown catalogue kind: {kind}")),
    };
    Ok(root.join("data").join(file_name))
}

pub fn load_all(root: &Path) -> Result<CatalogueBundle, String> {
    Ok(CatalogueBundle {
        food: load_document(&catalogue_path(root, "food")?, "food")?,
        memories: load_document(&catalogue_path(root, "memories")?, "memories")?,
        humans: load_document(&catalogue_path(root, "humans")?, "humans")?,
    })
}

pub fn load_document(path: &Path, kind: &str) -> Result<CsvDocument, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    parse_document(
        &bytes,
        kind,
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("data.csv"),
    )
}

pub fn parse_document(bytes: &[u8], kind: &str, file_name: &str) -> Result<CsvDocument, String> {
    let mut reader = ReaderBuilder::new()
        .delimiter(b';')
        .flexible(false)
        .from_reader(bytes);
    let headers = reader
        .headers()
        .map_err(|error| format!("Invalid {kind} CSV header: {error}"))?
        .iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    validate_headers(kind, &headers)?;
    let mut rows = Vec::new();
    for (index, record) in reader.records().enumerate() {
        let record =
            record.map_err(|error| format!("Invalid {kind} CSV row {}: {error}", index + 2))?;
        rows.push(record.iter().map(str::to_string).collect());
    }
    let document = CsvDocument {
        kind: kind.to_string(),
        file_name: file_name.to_string(),
        headers,
        rows,
    };
    validate_document(&document)?;
    Ok(document)
}

fn validate_headers(kind: &str, headers: &[String]) -> Result<(), String> {
    let expected: &[&str] = match kind {
        "food" => &[
            "Food",
            "Height",
            "Intellect",
            "Life Exp",
            "Strength",
            "Weight",
            "TotalAvailability",
        ],
        "memories" => &[
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
        "humans" => &[
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
        _ => return Err(format!("Unknown catalogue kind: {kind}")),
    };
    if headers.iter().map(String::as_str).collect::<Vec<_>>() != expected {
        return Err(format!(
            "Unexpected {kind} CSV columns. Expected: {}",
            expected.join(";")
        ));
    }
    Ok(())
}

pub fn validate_document(document: &CsvDocument) -> Result<(), String> {
    validate_headers(&document.kind, &document.headers)?;
    if document.rows.is_empty() {
        return Err(format!(
            "The {} catalogue must contain at least one row.",
            document.kind
        ));
    }
    let name_column = if document.kind == "humans" { 1 } else { 0 };
    let numeric_start = if document.kind == "humans" { 2 } else { 1 };
    let mut names = HashSet::new();
    for (row_index, row) in document.rows.iter().enumerate() {
        if row.len() != document.headers.len() {
            return Err(format!(
                "Row {} has {} columns; expected {}.",
                row_index + 2,
                row.len(),
                document.headers.len()
            ));
        }
        if document.kind == "humans" && row[0].trim().is_empty() {
            return Err(format!("Row {} has no category.", row_index + 2));
        }
        let name = row[name_column].trim();
        if name.is_empty() {
            return Err(format!("Row {} has no name.", row_index + 2));
        }
        if !names.insert(name.to_lowercase()) {
            return Err(format!(
                "Duplicate name in {} catalogue: {name}",
                document.kind
            ));
        }
        for (column, value) in row.iter().enumerate().skip(numeric_start) {
            if value.trim().is_empty() {
                continue;
            }
            let parsed = value.trim().parse::<i32>().map_err(|_| {
                format!(
                    "{} row {}, column {} must be a whole number.",
                    document.kind,
                    row_index + 2,
                    document.headers[column]
                )
            })?;
            if parsed < 0 {
                return Err(format!(
                    "{} row {}, column {} cannot be negative.",
                    document.kind,
                    row_index + 2,
                    document.headers[column]
                ));
            }
        }
    }
    Ok(())
}

pub fn save_document(root: &Path, document: &CsvDocument) -> Result<CsvDocument, String> {
    validate_document(document)?;
    let bytes = serialize_document(document)?;
    let path = catalogue_path(root, &document.kind)?;
    atomic_write(&path, &bytes, root)?;
    load_document(&path, &document.kind)
}

pub fn serialize_document(document: &CsvDocument) -> Result<Vec<u8>, String> {
    validate_document(document)?;
    let mut writer = WriterBuilder::new()
        .delimiter(b';')
        .terminator(csv::Terminator::CRLF)
        .from_writer(Vec::new());
    writer
        .write_record(&document.headers)
        .map_err(|error| format!("Could not serialize CSV header: {error}"))?;
    for row in &document.rows {
        writer
            .write_record(row)
            .map_err(|error| format!("Could not serialize CSV row: {error}"))?;
    }
    writer
        .into_inner()
        .map_err(|error| format!("Could not finish CSV: {error}"))
}

pub fn import_document(root: &Path, kind: &str, source: &Path) -> Result<CsvDocument, String> {
    let bytes = fs::read(source)
        .map_err(|error| format!("Could not read import file {}: {error}", source.display()))?;
    let document = parse_document(
        &bytes,
        kind,
        source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("import.csv"),
    )?;
    save_document(root, &document)
}

pub fn export_document(root: &Path, kind: &str, target: &Path) -> Result<(), String> {
    let source = catalogue_path(root, kind)?;
    let bytes = fs::read(&source)
        .map_err(|error| format!("Could not read {}: {error}", source.display()))?;
    atomic_write(target, &bytes, root)
}

pub fn load_asset_mappings(root: &Path) -> Result<BTreeMap<String, String>, String> {
    let path = root.join("data/asset-mappings.json");
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let mappings = serde_json::from_str::<BTreeMap<String, String>>(&text)
        .map_err(|error| format!("Invalid asset mapping file {}: {error}", path.display()))?;
    validate_asset_mappings(&mappings, &load_all(root)?)?;
    Ok(mappings)
}

pub fn validate_asset_mappings(
    mappings: &BTreeMap<String, String>,
    catalogues: &CatalogueBundle,
) -> Result<(), String> {
    let food_names = catalogues
        .food
        .rows
        .iter()
        .map(|row| row[0].as_str())
        .collect::<HashSet<_>>();
    let memory_names = catalogues
        .memories
        .rows
        .iter()
        .map(|row| row[0].as_str())
        .collect::<HashSet<_>>();
    for (asset, target) in mappings {
        let target = target.trim();
        let valid = if asset.starts_with("DA_Food_") {
            food_names.contains(target)
        } else if asset.starts_with("DA_Memory_") {
            memory_names.contains(target)
        } else {
            return Err(format!("Unsupported asset mapping key: {asset}"));
        };
        if !valid {
            return Err(format!(
                "Asset {asset} maps to unknown catalogue item: {target}"
            ));
        }
    }
    Ok(())
}

pub fn serialize_asset_mappings(
    mappings: &BTreeMap<String, String>,
    catalogues: &CatalogueBundle,
) -> Result<Vec<u8>, String> {
    validate_asset_mappings(mappings, catalogues)?;
    serde_json::to_vec_pretty(mappings)
        .map_err(|error| format!("Could not serialize asset mappings: {error}"))
}

pub fn save_asset_mappings(
    root: &Path,
    mappings: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, String> {
    let bytes = serialize_asset_mappings(mappings, &load_all(root)?)?;
    atomic_write(&root.join("data/asset-mappings.json"), &bytes, root)?;
    load_asset_mappings(root)
}

pub fn import_asset_mappings(
    root: &Path,
    source: &Path,
) -> Result<BTreeMap<String, String>, String> {
    let text = fs::read_to_string(source)
        .map_err(|error| format!("Could not read import file {}: {error}", source.display()))?;
    let mappings = serde_json::from_str::<BTreeMap<String, String>>(&text)
        .map_err(|error| format!("Invalid asset mapping JSON: {error}"))?;
    save_asset_mappings(root, &mappings)
}

pub fn export_asset_mappings(root: &Path, target: &Path) -> Result<(), String> {
    let source = root.join("data/asset-mappings.json");
    let bytes = fs::read(&source)
        .map_err(|error| format!("Could not read {}: {error}", source.display()))?;
    atomic_write(target, &bytes, root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_duplicate_food_names() {
        let csv = b"Food;Height;Intellect;Life Exp;Strength;Weight;TotalAvailability\nPear;1;1;1;1;1;2\npear;1;1;1;1;1;2\n";
        let error = parse_document(csv, "food", "Food.csv").unwrap_err();
        assert!(error.contains("Duplicate name"));
    }

    #[test]
    fn accepts_empty_stat_cells() {
        let csv = b"Memory;Adaptability;Communication;Creativity;Discipline;Empathy;Focus;Leadership;Logic;Patience;Wisdom;WorldCount\nMaps;1;;;;;;;2;;;3\n";
        assert!(parse_document(csv, "memories", "Memories.csv").is_ok());
    }

    #[test]
    fn shipped_asset_mappings_reference_existing_catalogue_items() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        assert!(load_asset_mappings(root).is_ok());
    }

    #[test]
    fn rejects_asset_mapping_to_the_wrong_catalogue_type() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let catalogues = load_all(root).unwrap();
        let mappings = BTreeMap::from([("DA_Food_Test".to_string(), "Maps".to_string())]);
        let error = validate_asset_mappings(&mappings, &catalogues).unwrap_err();
        assert!(error.contains("unknown catalogue item"));
    }
}

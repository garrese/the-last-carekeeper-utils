use crate::domain::catalog;
use crate::domain::optimizer;
use crate::domain::save;
use crate::domain::settings;
use crate::domain::{
    BootstrapState, CatalogueBundle, CsvDocument, HumanAssessment, InventoryReport, RecipeResult,
    Settings,
};
use std::collections::BTreeMap;
use std::path::Path;

#[tauri::command]
pub fn bootstrap() -> Result<BootstrapState, String> {
    let root = settings::ensure_portable_layout()?;
    Ok(BootstrapState {
        settings: settings::load_settings(&root)?,
        catalogues: catalog::load_all(&root)?,
        asset_mappings: catalog::load_asset_mappings(&root)?,
        portable_root: root.display().to_string(),
        default_save_directory: settings::default_save_directory()
            .map(|path| path.display().to_string()),
    })
}

#[tauri::command]
pub fn save_configuration(configuration: Settings) -> Result<Settings, String> {
    let root = settings::ensure_portable_layout()?;
    if let Some(save_path) = &configuration.save_path {
        let path = Path::new(save_path);
        if !path.is_file() {
            return Err("The selected save file does not exist.".to_string());
        }
        if !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("sav"))
        {
            return Err("The selected file must have a .sav extension.".to_string());
        }
    }
    settings::save_settings(&root, &configuration)?;
    settings::load_settings(&root)
}

#[tauri::command]
pub async fn refresh_inventory(configuration: Settings) -> Result<InventoryReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = settings::ensure_portable_layout()?;
        settings::save_settings(&root, &configuration)?;
        save::import_inventory(&root, &configuration)
    })
    .await
    .map_err(|error| format!("Inventory worker failed: {error}"))?
}

#[tauri::command]
pub fn reload_catalogues() -> Result<CatalogueBundle, String> {
    let root = settings::ensure_portable_layout()?;
    catalog::load_all(&root)
}

#[tauri::command]
pub fn load_asset_mappings() -> Result<BTreeMap<String, String>, String> {
    let root = settings::ensure_portable_layout()?;
    catalog::load_asset_mappings(&root)
}

#[tauri::command]
pub fn save_asset_mappings(
    mappings: BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, String> {
    let root = settings::ensure_portable_layout()?;
    catalog::save_asset_mappings(&root, &mappings)
}

#[tauri::command]
pub fn import_asset_mappings(source_path: String) -> Result<BTreeMap<String, String>, String> {
    let root = settings::ensure_portable_layout()?;
    catalog::import_asset_mappings(&root, Path::new(&source_path))
}

#[tauri::command]
pub fn export_asset_mappings(target_path: String) -> Result<(), String> {
    let root = settings::ensure_portable_layout()?;
    catalog::export_asset_mappings(&root, Path::new(&target_path))
}

#[tauri::command]
pub fn save_catalogue(document: CsvDocument) -> Result<CsvDocument, String> {
    let root = settings::ensure_portable_layout()?;
    catalog::save_document(&root, &document)
}

#[tauri::command]
pub fn import_catalogue(kind: String, source_path: String) -> Result<CsvDocument, String> {
    let root = settings::ensure_portable_layout()?;
    catalog::import_document(&root, &kind, Path::new(&source_path))
}

#[tauri::command]
pub fn export_catalogue(kind: String, target_path: String) -> Result<(), String> {
    let root = settings::ensure_portable_layout()?;
    catalog::export_document(&root, &kind, Path::new(&target_path))
}

#[tauri::command]
pub async fn assess_humans(
    inventory: BTreeMap<String, u32>,
) -> Result<Vec<HumanAssessment>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = settings::ensure_portable_layout()?;
        let catalogues = catalog::load_all(&root)?;
        optimizer::assess_humans(&catalogues, &inventory)
    })
    .await
    .map_err(|error| format!("Assessment worker failed: {error}"))?
}

#[tauri::command]
pub async fn calculate_recipe(
    inventory: BTreeMap<String, u32>,
    profession: String,
    objective: String,
) -> Result<RecipeResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = settings::ensure_portable_layout()?;
        let catalogues = catalog::load_all(&root)?;
        optimizer::calculate_recipe(&catalogues, &inventory, &profession, &objective)
    })
    .await
    .map_err(|error| format!("Optimizer worker failed: {error}"))?
}

pub mod catalog;
pub mod game_sync;
pub mod optimizer;
pub mod save;
pub mod settings;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const STATS: [&str; 15] = [
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
];

pub const FOOD_STATS: [&str; 5] = ["Weight", "Height", "Life Exp", "Strength", "Intellect"];
pub const MEMORY_STATS: [&str; 10] = [
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
];

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub save_path: Option<String>,
    #[serde(default)]
    pub chest_names: Vec<String>,
    #[serde(default)]
    pub game_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CsvDocument {
    pub kind: String,
    pub file_name: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueBundle {
    pub food: CsvDocument,
    pub memories: CsvDocument,
    pub humans: CsvDocument,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportedItem {
    pub asset_name: String,
    pub mapped_name: Option<String>,
    pub quantity: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventorySource {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub items: Vec<ImportedItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryReport {
    pub save_path: String,
    pub file_size: u64,
    pub modified_unix_ms: u128,
    pub raw_bytes: usize,
    pub block_count: usize,
    pub sources: Vec<InventorySource>,
    pub discovered_chests: Vec<String>,
    pub missing_chests: Vec<String>,
    pub inventory: BTreeMap<String, u32>,
    pub unresolved_assets: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapState {
    pub settings: Settings,
    pub catalogues: CatalogueBundle,
    pub asset_mappings: BTreeMap<String, String>,
    pub portable_root: String,
    pub default_save_directory: Option<String>,
    pub default_game_directory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GameSyncSource {
    pub game_path: String,
    pub paks_path: String,
    pub package_count: usize,
    pub extracted_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncChange {
    pub id: String,
    pub section: String,
    pub action: String,
    pub asset_name: Option<String>,
    pub display_name: String,
    pub summary: String,
    pub current: Option<Vec<String>>,
    pub proposed: Option<Vec<String>>,
    pub selected_by_default: bool,
    pub can_apply: bool,
    pub reason: Option<String>,
    pub icon_asset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncSection {
    pub kind: String,
    pub changes: Vec<SyncChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncProposal {
    pub source: GameSyncSource,
    pub sections: Vec<SyncSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncApplyResult {
    pub applied_count: usize,
    pub catalogues: CatalogueBundle,
    pub asset_mappings: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HumanAssessment {
    pub category: String,
    pub profession: String,
    pub achievable: bool,
    pub coverage_percent: f64,
    pub deficits: BTreeMap<String, i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipePick {
    pub item_name: String,
    pub item_type: String,
    pub quantity: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeResult {
    pub profession: String,
    pub feasible: bool,
    pub picks: Vec<RecipePick>,
    pub totals: BTreeMap<String, i32>,
    pub requirements: BTreeMap<String, i32>,
    pub deficits: BTreeMap<String, i32>,
    pub excess: BTreeMap<String, i32>,
    pub item_count: u32,
    pub waste: i32,
    pub matched_professions: Vec<String>,
}

use super::Settings;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_MAPPINGS: &str = r#"{
  "DA_Food_High-FatEnergy": "High-Fat",
  "DA_Food_Mind_Surge": "Mind Surge",
  "DA_Food_Nutri-Core": "Nutri-Core",
  "DA_Food_PhysiqueFuel": "Physique Fuel",
  "DA_Food_Bone-Fortify": "Bone-Fortify",
  "DA_Food_Endura-Growth": "Endura-Growth",
  "DA_Food_ImmuneBoost": "Immune Boost",
  "DA_Food_MuscleFortification": "Muscle Fortification",
  "DA_Food_Neuro-Boost": "Neuro-Boost",
  "DA_Food_Hyper-Evolution": "Hyper-Evolution",
  "DA_Food_MitochondrialSurge": "Mitochondrial Surge",
  "DA_Food_NaniteInfusion": "Nanite Infusion",
  "DA_Food_UltimateGenesis": "Ultimate Genesis",
  "DA_Food_Pear": "Pear",
  "DA_Memory_BasketBall": "Basketball",
  "DA_Memory_Books_Encyclopedia": "Encyclopedia",
  "DA_Memory_Books_FirstAid": "First Aid",
  "DA_Memory_Books_Meditation": "Meditation",
  "DA_Memory_Books_Programming": "Programming Manual",
  "DA_Memory_Books_Sudoku": "Sudoku Book",
  "DA_Memory_Books_SunTzu": "The Art of War",
  "DA_Memory_BowlingBall": "Bowling Ball",
  "DA_Memory_BowlingPin": "Bowling Pin",
  "DA_Memory_Camera": "Camera",
  "DA_Memory_Compass": "Compass",
  "DA_Memory_Crayon": "Crayon",
  "DA_Memory_Drawings_Biology": "Biology Notes",
  "DA_Memory_Drawings_Blueprints": "Blueprints",
  "DA_Memory_Drawings_Cards": "Cards",
  "DA_Memory_Drawings_Kids": "Small Human Art",
  "DA_Memory_Drawings_Letters": "Love Letters",
  "DA_Memory_Drawings_Logs": "Commander's Log",
  "DA_Memory_Drawings_Maps": "Maps",
  "DA_Memory_Drawings_MCogni": "Cognitive Cards",
  "DA_Memory_Drawings_MSurvival": "Survival Diagrams",
  "DA_Memory_Drawings_Music": "Music Notes",
  "DA_Memory_Drawings_Notes": "Travel Journal",
  "DA_Memory_Drawings_Plans": "Plans",
  "DA_Memory_Guitar": "Guitar",
  "DA_Memory_Mirror": "Mirror",
  "DA_Memory_MysteryBox": "Mystery Box",
  "DA_Memory_SmallTree2": "Small Tree",
  "DA_Memory_Stopwatch": "Stopwatch",
  "DA_Memory_Toy": "Teddy Bear"
}"#;

pub fn portable_root() -> Result<PathBuf, String> {
    if cfg!(debug_assertions) {
        return Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "Could not resolve the development project root.".to_string());
    }
    std::env::current_exe()
        .map_err(|error| format!("Could not locate the executable: {error}"))?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Could not resolve the portable application folder.".to_string())
}

pub fn default_save_directory() -> Option<PathBuf> {
    let local_app_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
    let save_games = local_app_data
        .join("Voyage")
        .join("Saved")
        .join("SaveGames");
    Some(if save_games.is_dir() {
        save_games
    } else {
        local_app_data
    })
}

pub fn ensure_portable_layout() -> Result<PathBuf, String> {
    let root = portable_root()?;
    fs::create_dir_all(root.join("data"))
        .map_err(|error| format!("Could not create data folder: {error}"))?;
    fs::create_dir_all(root.join("config"))
        .map_err(|error| format!("Could not create config folder: {error}"))?;
    fs::create_dir_all(root.join("backups"))
        .map_err(|error| format!("Could not create backups folder: {error}"))?;

    ensure_file(
        &root.join("data/Food.csv"),
        include_str!("../../../data/Food.csv"),
    )?;
    ensure_file(
        &root.join("data/Memories.csv"),
        include_str!("../../../data/Memories.csv"),
    )?;
    ensure_file(
        &root.join("data/Humans.csv"),
        include_str!("../../../data/Humans.csv"),
    )?;
    ensure_file(&root.join("data/asset-mappings.json"), DEFAULT_MAPPINGS)?;
    Ok(root)
}

fn ensure_file(path: &Path, contents: &str) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    fs::write(path, contents)
        .map_err(|error| format!("Could not create {}: {error}", path.display()))
}

pub fn load_settings(root: &Path) -> Result<Settings, String> {
    let path = root.join("config/settings.json");
    if !path.exists() {
        let settings = Settings::default();
        save_settings(root, &settings)?;
        return Ok(settings);
    }
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("Invalid settings file {}: {error}", path.display()))
}

pub fn save_settings(root: &Path, settings: &Settings) -> Result<(), String> {
    let mut normalized = settings.clone();
    normalized.chest_names = normalized
        .chest_names
        .iter()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .fold(Vec::new(), |mut names, name| {
            if !names
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&name))
            {
                names.push(name);
            }
            names
        });
    atomic_write_json(&root.join("config/settings.json"), &normalized, root)
}

pub fn atomic_write_json<T: Serialize>(path: &Path, value: &T, root: &Path) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("Could not serialize settings: {error}"))?;
    atomic_write(path, &bytes, root)
}

pub fn atomic_write(path: &Path, bytes: &[u8], root: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Target file has no parent directory.".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("data");
    let temporary = parent.join(format!(".{file_name}.{stamp}.tmp"));
    let mut file = fs::File::create(&temporary)
        .map_err(|error| format!("Could not create temporary file: {error}"))?;
    file.write_all(bytes)
        .map_err(|error| format!("Could not write temporary file: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("Could not flush temporary file: {error}"))?;

    let backup = root
        .join("backups")
        .join(format!("{file_name}.{stamp}.bak"));
    if path.exists() {
        fs::copy(path, &backup)
            .map_err(|error| format!("Could not create backup {}: {error}", backup.display()))?;
        fs::remove_file(path)
            .map_err(|error| format!("Could not replace {}: {error}", path.display()))?;
    }
    if let Err(error) = fs::rename(&temporary, path) {
        if backup.exists() {
            let _ = fs::copy(&backup, path);
        }
        return Err(format!("Could not activate {}: {error}", path.display()));
    }
    Ok(())
}

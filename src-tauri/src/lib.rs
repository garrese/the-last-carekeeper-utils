mod commands;
mod domain;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::save_configuration,
            commands::refresh_inventory,
            commands::reload_catalogues,
            commands::load_asset_mappings,
            commands::save_asset_mappings,
            commands::import_asset_mappings,
            commands::export_asset_mappings,
            commands::save_catalogue,
            commands::import_catalogue,
            commands::export_catalogue,
            commands::scan_game_data,
            commands::apply_game_data_sync,
            commands::assess_humans,
            commands::calculate_recipe,
        ])
        .run(tauri::generate_context!())
        .expect("error while running The Last Carekeeper Utils");
}

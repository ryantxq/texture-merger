pub mod commands;
pub mod composite;
pub mod decode;
pub mod export;
pub mod import;
pub mod model;
pub mod preview;

use std::sync::atomic::AtomicBool;

pub struct ExportState {
    pub cancel: AtomicBool,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ExportState { cancel: AtomicBool::new(false) })
        .invoke_handler(tauri::generate_handler![
            commands::import_files,
            commands::import_folder,
            commands::build_preview,
            commands::export_image,
            commands::cancel_export,
            commands::reveal_in_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

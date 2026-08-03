use std::path::{Path, PathBuf};
use tauri::State;
use crate::decode;
use crate::import;
use crate::model::{ExportOptions, ImportItem, LayerState, PreviewImage, Snapshot};

#[tauri::command]
pub fn import_files(paths: Vec<String>, next_id: u32) -> Result<Vec<ImportItem>, String> {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    Ok(import::import_paths(&paths, next_id))
}

#[tauri::command]
pub fn import_folder(path: String, next_id: u32) -> Result<Vec<ImportItem>, String> {
    let files = import::scan_png_files(Path::new(&path))?;
    Ok(import::import_paths(&files, next_id))
}

#[tauri::command]
pub fn build_preview(snapshot: Snapshot, max_dim: u32) -> Result<PreviewImage, String> {
    crate::preview::build_preview(snapshot, max_dim)
}

#[tauri::command]
pub fn export_image(
    app: tauri::AppHandle,
    snapshot: Snapshot,
    options: ExportOptions,
    save_path: String,
    state: State<'_, crate::ExportState>,
) -> Result<crate::model::ExportStats, String> {
    crate::export::run_export(app, snapshot, options, PathBuf::from(save_path), &state)
}

#[tauri::command]
pub fn cancel_export(state: State<'_, crate::ExportState>) {
    state.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
}

#[tauri::command]
pub fn reveal_in_folder(path: String) {
    let _ = std::process::Command::new("explorer")
        .arg("/select,")
        .arg(&path)
        .spawn();
}

#[tauri::command]
pub fn decode_meta_for_layer(state: LayerState) -> Result<LayerState, String> {
    // 供前端导入已有路径时补全尺寸（当前未使用，保留以备后续）
    let (w, h, _) = decode::load_png_meta(Path::new(&state.path))?;
    Ok(LayerState { width: w, height: h, ..state })
}

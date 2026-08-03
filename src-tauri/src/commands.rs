use std::path::{Path, PathBuf};
use tauri::{Emitter, State};
use crate::decode;
use crate::import;
use crate::model::{ExportFile, ExportOptions, ImportItem, LayerState, PreviewImage, Snapshot};

#[tauri::command]
pub fn import_files(app: tauri::AppHandle, paths: Vec<String>, next_id: u32) -> Result<Vec<ImportItem>, String> {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let total = paths.len();
    let on_progress = |done: usize, _total: usize| {
        let _ = app.emit("import-progress", serde_json::json!({
            "done": done, "total": total
        }));
    };
    Ok(import::import_paths_with_progress(&paths, next_id, &on_progress))
}

#[tauri::command]
pub fn import_folder(app: tauri::AppHandle, path: String, next_id: u32) -> Result<Vec<ImportItem>, String> {
    let files = import::scan_png_files(Path::new(&path))?;
    let total = files.len();
    let on_progress = |done: usize, _total: usize| {
        let _ = app.emit("import-progress", serde_json::json!({
            "done": done, "total": total
        }));
    };
    Ok(import::import_paths_with_progress(&files, next_id, &on_progress))
}

#[tauri::command]
pub fn build_preview(snapshot: Snapshot, cache: State<'_, crate::preview::PreviewCache>) -> Result<PreviewImage, String> {
    crate::preview::build_preview(snapshot, cache.inner())
}

#[tauri::command]
pub fn get_layer_mask(layer: LayerState, cache: State<'_, crate::preview::PreviewCache>) -> Result<PreviewImage, String> {
    let (w, h, rgba) = crate::preview::layer_preview(&layer, cache.inner())?;
    // 编码 PNG data URL：前端以 multiply 叠加做「颜色加深」图层像素定位
    let mut buf = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut buf, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        let mut writer = enc.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&rgba).map_err(|e| e.to_string())?;
    }
    Ok(PreviewImage {
        width: w,
        height: h,
        data_url: decode::rgba8_to_data_url(&buf),
    })
}

#[tauri::command]
pub fn export_image(
    app: tauri::AppHandle,
    snapshot: Snapshot,
    options: ExportOptions,
    dir: String,
    base_stem: String,
    state: State<'_, crate::ExportState>,
) -> Result<Vec<ExportFile>, String> {
    crate::export::run_export(app, snapshot, options, PathBuf::from(dir), base_stem, &state)
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

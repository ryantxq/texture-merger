use std::path::{Path, PathBuf};
use tauri::{Emitter, Manager, State};
use crate::decode;
use crate::import;
use crate::model::{ExportFile, ExportOptions, ImportItem, LayerState, PreviewImage, Snapshot};

/// 长耗时命令统一走 async + spawn_blocking：
/// 同步命令在 Tauri 主线程执行会阻塞 IPC 事件投递（进度事件被积压到命令结束才送达），
/// 改为阻塞池执行后事件可实时送达前端。
#[tauri::command]
pub async fn import_files(app: tauri::AppHandle, paths: Vec<String>, next_id: u32) -> Result<Vec<ImportItem>, String> {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let total = paths.len();
    let handle = app.clone();
    Ok(tauri::async_runtime::spawn_blocking(move || {
        let on_progress = |done: usize, _total: usize| {
            let _ = handle.emit("import-progress", serde_json::json!({ "done": done, "total": total }));
        };
        import::import_paths_with_progress(&paths, next_id, &on_progress)
    })
    .await
    .map_err(|e| e.to_string())?)
}

#[tauri::command]
pub async fn import_folder(app: tauri::AppHandle, path: String, next_id: u32) -> Result<Vec<ImportItem>, String> {
    let files = import::scan_png_files(Path::new(&path))?;
    let total = files.len();
    let handle = app.clone();
    Ok(tauri::async_runtime::spawn_blocking(move || {
        let on_progress = |done: usize, _total: usize| {
            let _ = handle.emit("import-progress", serde_json::json!({ "done": done, "total": total }));
        };
        import::import_paths_with_progress(&files, next_id, &on_progress)
    })
    .await
    .map_err(|e| e.to_string())?)
}

#[tauri::command]
pub async fn build_preview(app: tauri::AppHandle, snapshot: Snapshot) -> Result<PreviewImage, String> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let cache = handle.state::<crate::preview::PreviewCache>();
        crate::preview::build_preview(snapshot, cache.inner())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_layer_mask(app: tauri::AppHandle, layer: LayerState) -> Result<PreviewImage, String> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let cache = handle.state::<crate::preview::PreviewCache>();
        let (w, h, rgba) = crate::preview::layer_preview(&layer, cache.inner())?;
        // 编码 PNG data URL：前端以高亮色染色叠加做图层像素定位
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
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn export_image(
    app: tauri::AppHandle,
    snapshot: Snapshot,
    options: ExportOptions,
    dir: String,
    base_stem: String,
) -> Result<Vec<ExportFile>, String> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::export::run_export(handle, snapshot, options, PathBuf::from(dir), base_stem)
    })
    .await
    .map_err(|e| e.to_string())?
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

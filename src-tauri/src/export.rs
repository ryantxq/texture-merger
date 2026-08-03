use std::path::PathBuf;
use crate::model::{ExportOptions, ExportStats, Snapshot};
use crate::ExportState;

/// 全分辨率导出（Task 6 实现）。
#[allow(unused_variables)]
pub fn run_export(
    app: tauri::AppHandle,
    snapshot: Snapshot,
    options: ExportOptions,
    save_path: PathBuf,
    state: &ExportState,
) -> Result<ExportStats, String> {
    Err("not implemented".into())
}

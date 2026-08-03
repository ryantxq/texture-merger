use std::path::{Path, PathBuf};
use crate::decode;
use crate::model::ImportItem;

/// 递归收集目录下所有 .png/.PNG 文件，路径排序保证确定性。
pub fn scan_png_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    collect(dir, &mut out).map_err(|e| e.to_string())?;
    out.sort();
    Ok(out)
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            collect(&p, out)?;
        } else if p.extension().map(|e| e.to_string_lossy().to_lowercase()) == Some("png".into()) {
            out.push(p);
        }
    }
    Ok(())
}

/// 按给定路径列表导入：每个文件生成 Ok(元信息+缩略图) 或 Error(失败原因)。
/// `next_id` 为起始图层 id（全局递增由调用方维护）。
pub fn import_paths(paths: &[PathBuf], next_id: u32) -> Vec<ImportItem> {
    paths
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let id = next_id + i as u32;
            match import_one(p, id) {
                Ok(item) => item,
                Err(reason) => ImportItem::Error {
                    path: p.to_string_lossy().into_owned(),
                    reason,
                },
            }
        })
        .collect()
}

fn import_one(p: &Path, id: u32) -> Result<ImportItem, String> {
    let (w, h, _is16) = decode::load_png_meta(p)?;
    let (tw, th, thumb_bytes) = decode::make_thumbnail(p, 128)?;
    let _ = (tw, th);
    Ok(ImportItem::Ok {
        id,
        path: p.to_string_lossy().into_owned(),
        name: p.file_stem().unwrap_or_default().to_string_lossy().into_owned(),
        width: w,
        height: h,
        has_alpha: true,
        thumbnail: decode::rgba8_to_data_url(&thumb_bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, name: &str, data: &[u8]) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, data).unwrap();
        p
    }

    #[test]
    fn scan_folder_recursive_finds_png() {
        let dir = std::env::temp_dir().join("tm_scan");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        touch(&dir, "a.PNG", b"x");
        touch(&dir, "b.png", b"x");
        touch(&dir, "c.jpg", b"x");
        touch(&dir.join("sub"), "d.png", b"x");
        let files = scan_png_files(&dir).unwrap();
        let names: Vec<String> = files.iter().map(|f| f.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert!(names.contains(&"a.PNG".into()));
        assert!(names.contains(&"b.png".into()));
        assert!(names.contains(&"d.png".into()));
        assert!(!names.contains(&"c.jpg".into()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_bad_file_reports_error_item() {
        let dir = std::env::temp_dir().join("tm_import");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let good = touch(&dir, "good.png", &super::super::decode::make_png_rgba8(2, 2, &[0u8; 16]));
        let bad = touch(&dir, "bad.png", b"not png");
        let items = import_paths(&[good, bad], 0);
        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|i| matches!(i, crate::model::ImportItem::Ok { .. })));
        assert!(items.iter().any(|i| matches!(i, crate::model::ImportItem::Error { .. })));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

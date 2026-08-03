use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use rayon::prelude::*;
use tauri::{Emitter, Manager};
use crate::composite;
use crate::decode;
use crate::model::{ExportFile, ExportOptions, LayerState, Snapshot};
use crate::ExportState;

/// 供命令调用：全分辨率合成一次 → 各目标尺寸并行编码写盘，期间发进度事件。
/// 文件名：`{base_stem}_{实际输出长边}.png`（如 atlas_2048.png）。
/// 进度：合成阶段 0..0.5（按层），编码阶段 0.5..1.0（按尺寸）。
pub fn run_export(
    app: tauri::AppHandle,
    snapshot: Snapshot,
    options: ExportOptions,
    dir: PathBuf,
    base_stem: String,
) -> Result<Vec<ExportFile>, String> {
    let state = app.state::<ExportState>();
    let on_progress = |phase: &str, p: f32, info: &str| {
        let _ = app.emit("export-progress", serde_json::json!({
            "phase": phase, "progress": p, "info": info
        }));
    };
    run_export_inner(&snapshot, options, &dir, &base_stem, &on_progress, &state.cancel)
}

/// 内部导出（纯函数，便于测试）：清洗 sizes → 全分辨率合成一次 → 各尺寸 rayon 并行编码写盘。
pub fn run_export_inner(
    snapshot: &Snapshot,
    options: ExportOptions,
    dir: &Path,
    base_stem: &str,
    on_progress: &dyn Fn(&str, f32, &str),
    cancel: &AtomicBool,
) -> Result<Vec<ExportFile>, String> {
    let layers = &snapshot.layers;
    let (out_w, out_h) = composite::output_size(layers).ok_or("图层尺寸不一致或为空")?;
    let n = layers.iter().filter(|l| l.visible).count() as u32;
    if n == 0 {
        return Err("没有可见图层".into());
    }

    // ---------- 清洗 sizes：过滤 0、去重；s >= 长边按全分辨率（scale=1，不放大） ----------
    let long_edge = out_w.max(out_h);
    // (请求尺寸, scale, 输出宽, 输出高)
    let mut entries: Vec<(u32, u32, u32, u32)> = Vec::new();
    for &s in &options.sizes {
        if s == 0 {
            continue;
        }
        let scale = ((long_edge + s - 1) / s).max(1);
        let ow = out_w / scale;
        let oh = out_h / scale;
        // 按实际输出尺寸去重，避免同名文件互相覆盖
        if entries.iter().any(|&(_, _, a, b)| a == ow && b == oh) {
            continue;
        }
        entries.push((s, scale, ow, oh));
    }
    if entries.is_empty() {
        return Err("导出尺寸列表为空".into());
    }

    let last_progress = std::cell::Cell::new(-1.0f32);
    let progress = |phase: &str, p: f32, info: &str| {
        if (p - last_progress.get()).abs() >= 0.01 {
            last_progress.set(p);
            on_progress(phase, p, info);
        }
    };

    // ---------- 全分辨率合成一次（合成阶段进度 0..0.5） ----------
    let (full8, full16): (Vec<u8>, Vec<u16>) = if options.bit_depth == 16 {
        let mut acc = vec![0u16; (out_w * out_h * 4) as usize];
        let mut i = 0u32;
        for layer in composite::visible_bottom_up(layers) {
            if cancel.load(Ordering::Relaxed) {
                return Err("已取消".into());
            }
            let (w, h, rgba16) = decode::load_png_rgba16_scaled(std::path::Path::new(&layer.path))?;
            let bytes: Vec<u8> = rgba16.iter().flat_map(|v| v.to_le_bytes()).collect();
            let transformed = composite::transform_into(&bytes, w, h, layer.rotate, layer.flip_h, layer.flip_v);
            blend_layer16(&mut acc, &transformed, out_w, out_h, layer);
            i += 1;
            progress("composite", 0.5 * (i as f32 / n as f32), &format!("合成 {i}/{n}"));
        }
        for px in acc.chunks_exact_mut(4) {
            let p = [px[0], px[1], px[2], px[3]];
            let up = composite::unpremultiply16(p);
            px.copy_from_slice(&up);
        }
        (Vec::new(), acc)
    } else {
        let mut acc = vec![0u8; (out_w * out_h * 4) as usize];
        let mut i = 0u32;
        for layer in composite::visible_bottom_up(layers) {
            if cancel.load(Ordering::Relaxed) {
                return Err("已取消".into());
            }
            let (w, h, rgba) = decode::load_png_rgba8(std::path::Path::new(&layer.path))?;
            let transformed = composite::transform_into(&rgba, w, h, layer.rotate, layer.flip_h, layer.flip_v);
            blend_layer8(&mut acc, &transformed, out_w, out_h, layer);
            i += 1;
            progress("composite", 0.5 * (i as f32 / n as f32), &format!("合成 {i}/{n}"));
        }
        for px in acc.chunks_exact_mut(4) {
            let p = [px[0], px[1], px[2], px[3]];
            let up = composite::unpremultiply8(p);
            px.copy_from_slice(&up);
        }
        (acc, Vec::new())
    };
    let full16_be: Vec<u8> = full16.iter().flat_map(|v| v.to_be_bytes()).collect();
    let depth: u8 = if options.bit_depth == 16 { 16 } else { 8 };
    let compression = options.compression.png_level();

    // ---------- 各尺寸 rayon 并行编码写盘（编码阶段进度 0.5..1.0，按尺寸） ----------
    let count = entries.len();
    let results: Vec<Result<ExportFile, String>> = entries
        .par_iter()
        .map(|&(req, scale, ow, oh)| {
            if cancel.load(Ordering::Relaxed) {
                return Err("已取消".into());
            }
            let out_long_edge = ow.max(oh);
            let save_path = dir.join(format!("{base_stem}_{out_long_edge}.png"));
            let t = Instant::now();
            if depth == 16 {
                let bytes: Vec<u8> = if scale == 1 {
                    full16_be.clone()
                } else {
                    crate::preview::downsample16(&full16, out_w, out_h, ow, oh)
                        .iter()
                        .flat_map(|v| v.to_be_bytes())
                        .collect()
                };
                write_png(&bytes, ow, oh, 16, compression, &save_path, cancel)?;
            } else {
                let bytes: Vec<u8> = if scale == 1 {
                    full8.clone()
                } else {
                    crate::preview::downsample(&full8, out_w, out_h, ow, oh)
                };
                write_png(&bytes, ow, oh, 8, compression, &save_path, cancel)?;
            }
            let bytes_written = std::fs::metadata(&save_path).map(|m| m.len()).unwrap_or(0);
            Ok(ExportFile {
                size: req,
                width: ow,
                height: oh,
                bytes_written,
                duration_ms: t.elapsed().as_millis(),
            })
        })
        .collect();

    let mut files = Vec::with_capacity(count);
    for (i, r) in results.into_iter().enumerate() {
        let f = r?;
        files.push(f);
        progress("encode", 0.5 + 0.5 * ((i + 1) as f32 / count as f32), &format!("编码 {}/{}", i + 1, count));
    }
    Ok(files)
}

/// 把变换后的层（RGBA8，尺寸=输出尺寸）预乘混合进 acc。
fn blend_layer8(acc: &mut [u8], layer: &[u8], out_w: u32, out_h: u32, _l: &LayerState) {
    for y in 0..out_h {
        for x in 0..out_w {
            let off = ((y * out_w + x) * 4) as usize;
            let px = [layer[off], layer[off + 1], layer[off + 2], layer[off + 3]];
            let dst: &mut [u8; 4] = (&mut acc[off..off + 4]).try_into().unwrap();
            composite::blend_premul8(dst, px);
        }
    }
}

fn blend_layer16(acc: &mut [u16], layer: &[u8], out_w: u32, out_h: u32, _l: &LayerState) {
    for y in 0..out_h {
        for x in 0..out_w {
            let off = ((y * out_w + x) * 4) as usize;
            let px = [
                u16::from_le_bytes([layer[off * 2], layer[off * 2 + 1]]),
                u16::from_le_bytes([layer[off * 2 + 2], layer[off * 2 + 3]]),
                u16::from_le_bytes([layer[off * 2 + 4], layer[off * 2 + 5]]),
                u16::from_le_bytes([layer[off * 2 + 6], layer[off * 2 + 7]]),
            ];
            let dst: &mut [u16; 4] = (&mut acc[off..off + 4]).try_into().unwrap();
            composite::blend_premul16(dst, px);
        }
    }
}

/// 编码 PNG 并写盘；编码按行分块检查取消（取消时删除半成品文件）。
fn write_png(
    rgba: &[u8],
    w: u32,
    h: u32,
    depth: u8,
    comp: png::Compression,
    save_path: &Path,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let file = std::fs::File::create(save_path).map_err(|e| format!("创建文件失败: {e}"))?;
    let mut enc = png::Encoder::new(file, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(if depth == 16 { png::BitDepth::Sixteen } else { png::BitDepth::Eight });
    enc.set_compression(comp);
    // png 0.18 无逐行 Writer API，用 StreamWriter 逐行写入以支持编码期取消检查
    let writer = enc.write_header().map_err(|e| e.to_string())?;
    let mut stream = writer.into_stream_writer().map_err(|e| e.to_string())?;
    let row_bytes = (w * 4 * (depth / 8) as u32) as usize;
    for row in rgba.chunks(row_bytes) {
        if cancel.load(Ordering::Relaxed) {
            let _ = std::fs::remove_file(save_path);
            return Err("已取消".into());
        }
        stream.write_all(row).map_err(|e| format!("写入失败: {e}"))?;
    }
    stream.finish().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Compression;
    use std::path::PathBuf;

    fn tmp_png(name: &str, px: &[u8]) -> PathBuf {
        let p = std::env::temp_dir().join(name);
        std::fs::write(&p, decode::make_png_rgba8(2, 2, px)).unwrap();
        p
    }

    fn snapshot() -> Snapshot {
        let bottom = tmp_png("ex_bottom.png", &[255,0,0,255, 255,0,0,255, 255,0,0,255, 255,0,0,255]);
        let top = tmp_png("ex_top.png", &[0,0,255,255, 0,0,0,0, 0,0,0,0, 0,0,0,0]);
        Snapshot {
            layers: vec![
                LayerState { id: 0, path: top.to_string_lossy().into_owned(), width: 2, height: 2, rotate: 0, flip_h: false, flip_v: false, visible: true },
                LayerState { id: 1, path: bottom.to_string_lossy().into_owned(), width: 2, height: 2, rotate: 0, flip_h: false, flip_v: false, visible: true },
            ],
        }
    }

    fn tmp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn export_8bit_roundtrip() {
        let dir = tmp_dir("ex_out8");
        let files = run_export_inner(
            &snapshot(),
            ExportOptions { bit_depth: 8, compression: Compression::Fast, sizes: vec![2] },
            &dir,
            "atlas",
            &|_, _, _| {},
            &AtomicBool::new(false),
        ).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!((files[0].width, files[0].height), (2, 2));
        let out = dir.join("atlas_2.png");
        assert!(out.exists());
        let (_, _, rgba) = decode::load_png_rgba8(&out).unwrap();
        assert_eq!(&rgba[0..4], &[0, 0, 255, 255]);
        assert_eq!(&rgba[4..8], &[255, 0, 0, 255]);
        assert!(rgba[3] == 255);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_16bit_roundtrip() {
        let dir = tmp_dir("ex_out16");
        let files = run_export_inner(
            &snapshot(),
            ExportOptions { bit_depth: 16, compression: Compression::Balanced, sizes: vec![2] },
            &dir,
            "atlas",
            &|_, _, _| {},
            &AtomicBool::new(false),
        ).unwrap();
        assert_eq!(files.len(), 1);
        let out = dir.join("atlas_2.png");
        assert!(out.exists());
        let (_, _, rgba16) = decode::load_png_rgba16(&out).unwrap();
        // 8bit 源经线性放大(v*257)后合成到 16bit 输出：左上角=蓝，其余=红（全值域 65535）
        assert_eq!(&rgba16[0..4], &[0, 0, 65535, 65535]);
        assert_eq!(&rgba16[4..8], &[65535, 0, 0, 65535]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_multiple_sizes_box_average() {
        let dir = tmp_dir("ex_multi");
        let files = run_export_inner(
            &snapshot(),
            ExportOptions { bit_depth: 8, compression: Compression::Fast, sizes: vec![2, 1] },
            &dir,
            "atlas",
            &|_, _, _| {},
            &AtomicBool::new(false),
        ).unwrap();
        assert_eq!(files.len(), 2);
        assert!(dir.join("atlas_2.png").exists());
        let out1 = dir.join("atlas_1.png");
        assert!(out1.exists());
        let (w1, h1, rgba1) = decode::load_png_rgba8(&out1).unwrap();
        assert_eq!((w1, h1), (1, 1));
        // 2x2 = [蓝, 红, 红, 红] 箱式平均 → 期望约 [191, 0, 64, 255]，允许 ±2
        let expect = [191u8, 0, 64, 255];
        for c in 0..4 {
            assert!(
                (expect[c] as i32 - rgba1[c] as i32).abs() <= 2,
                "通道 {c}: 期望约 {}，实际 {}",
                expect[c],
                rgba1[c]
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_cancel_returns_error() {
        let dir = tmp_dir("ex_cancel");
        let cancel = AtomicBool::new(true);
        let r = run_export_inner(
            &snapshot(),
            ExportOptions { bit_depth: 8, compression: Compression::Fast, sizes: vec![2] },
            &dir,
            "atlas",
            &|_, _, _| {},
            &cancel,
        );
        assert!(r.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tauri::Emitter;
use crate::composite;
use crate::decode;
use crate::model::{ExportOptions, ExportStats, LayerState, Snapshot};
use crate::ExportState;

/// 供命令调用：全分辨率合成 → PNG 编码 → 写盘，期间发进度事件。
pub fn run_export(
    app: tauri::AppHandle,
    snapshot: Snapshot,
    options: ExportOptions,
    save_path: PathBuf,
    state: &ExportState,
) -> Result<ExportStats, String> {
    let on_progress = |phase: &str, p: f32, info: &str| {
        let _ = app.emit("export-progress", serde_json::json!({
            "phase": phase, "progress": p, "info": info
        }));
    };
    run_export_inner(&snapshot, options, &save_path, &on_progress, &state.cancel)
}

/// 内部导出（纯函数，便于测试）。
pub fn run_export_inner(
    snapshot: &Snapshot,
    options: ExportOptions,
    save_path: &PathBuf,
    on_progress: &dyn Fn(&str, f32, &str),
    cancel: &AtomicBool,
) -> Result<ExportStats, String> {
    let t0 = Instant::now();
    let layers = &snapshot.layers;
    let (out_w, out_h) = composite::output_size(layers).ok_or("图层尺寸不一致或为空")?;
    let n = layers.iter().filter(|l| l.visible).count() as u32;
    if n == 0 {
        return Err("没有可见图层".into());
    }

    let last_progress = std::cell::Cell::new(-1.0f32);
    let progress = |phase: &str, p: f32, info: &str| {
        if (p - last_progress.get()).abs() >= 0.01 || phase == "encode" {
            last_progress.set(p);
            on_progress(phase, p, info);
        }
    };

    // ---------- 合成 ----------
    if options.bit_depth == 16 {
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
            progress("composite", i as f32 / n as f32, &format!("合成 {i}/{n}"));
        }
        // 预乘 → 直通
        for px in acc.chunks_exact_mut(4) {
            let p = [px[0], px[1], px[2], px[3]];
            let up = composite::unpremultiply16(p);
            px.copy_from_slice(&up);
        }
        let out_bytes: Vec<u8> = acc.iter().flat_map(|v| v.to_be_bytes()).collect();
        write_png(&out_bytes, out_w, out_h, 16, options.compression.png_level(), save_path, cancel, &progress)?;
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
            progress("composite", i as f32 / n as f32, &format!("合成 {i}/{n}"));
        }
        for px in acc.chunks_exact_mut(4) {
            let p = [px[0], px[1], px[2], px[3]];
            let up = composite::unpremultiply8(p);
            px.copy_from_slice(&up);
        }
        write_png(&acc, out_w, out_h, 8, options.compression.png_level(), save_path, cancel, &progress)?;
    }

    let bytes_written = std::fs::metadata(save_path).map(|m| m.len()).unwrap_or(0);
    Ok(ExportStats {
        width: out_w,
        height: out_h,
        bytes_written,
        duration_ms: t0.elapsed().as_millis(),
    })
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

/// 编码 PNG 并写盘；编码按行分块检查取消。
fn write_png(
    rgba: &[u8],
    w: u32,
    h: u32,
    depth: u8,
    comp: png::Compression,
    save_path: &PathBuf,
    cancel: &AtomicBool,
    progress: &dyn Fn(&str, f32, &str),
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
    let total = h as usize;
    for (i, row) in rgba.chunks(row_bytes).enumerate() {
        if cancel.load(Ordering::Relaxed) {
            let _ = std::fs::remove_file(save_path);
            return Err("已取消".into());
        }
        stream.write_all(row).map_err(|e| format!("写入失败: {e}"))?;
        if i % 8 == 0 {
            progress("encode", 0.8 + 0.2 * (i as f32 / total as f32), &format!("编码 {i}/{total}"));
        }
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

    #[test]
    fn export_8bit_roundtrip() {
        let out = std::env::temp_dir().join("ex_out8.png");
        let stats = run_export_inner(&snapshot(), ExportOptions { bit_depth: 8, compression: Compression::Fast }, &out, &|_, _, _| {}, &std::sync::atomic::AtomicBool::new(false)).unwrap();
        assert_eq!((stats.width, stats.height), (2, 2));
        let (_, _, rgba) = decode::load_png_rgba8(&out).unwrap();
        assert_eq!(&rgba[0..4], &[0, 0, 255, 255]);
        assert_eq!(&rgba[4..8], &[255, 0, 0, 255]);
        assert!(rgba[3] == 255);
    }

    #[test]
    fn export_16bit_roundtrip() {
        let out = std::env::temp_dir().join("ex_out16.png");
        let stats = run_export_inner(&snapshot(), ExportOptions { bit_depth: 16, compression: Compression::Balanced }, &out, &|_, _, _| {}, &std::sync::atomic::AtomicBool::new(false)).unwrap();
        assert_eq!((stats.width, stats.height), (2, 2));
        let (_, _, rgba16) = decode::load_png_rgba16(&out).unwrap();
        // 8bit 源经线性放大(v*257)后合成到 16bit 输出：左上角=蓝，其余=红（全值域 65535）
        assert_eq!(&rgba16[0..4], &[0, 0, 65535, 65535]);
        assert_eq!(&rgba16[4..8], &[65535, 0, 0, 65535]);
    }

    #[test]
    fn export_cancel_returns_error() {
        let out = std::env::temp_dir().join("ex_cancel.png");
        let cancel = std::sync::atomic::AtomicBool::new(true);
        let r = run_export_inner(&snapshot(), ExportOptions { bit_depth: 8, compression: Compression::Fast }, &out, &|_, _, _| {}, &cancel);
        assert!(r.is_err());
    }
}

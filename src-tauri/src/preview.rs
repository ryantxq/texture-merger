use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use rayon::prelude::*;
use crate::composite;
use crate::decode;
use crate::model::{LayerBbox, LayerState, PreviewImage, Snapshot};

/// 预览长边上限：缓存与输出统一使用该值。
pub const PREVIEW_SCALE: u32 = 512;

/// 预览缓冲缓存：键 = (layer_id, rotate, flip_h, flip_v)，值 = 变换后缩小到长边≤512 的 RGBA8 缓冲。
/// 总量超限时整体清空并重置计数（简单 LRU 替代：构建成本可接受）。
pub struct PreviewCache {
    map: Mutex<HashMap<(u32, u8, bool, bool), Arc<Vec<u8>>>>,
    bytes: AtomicUsize,
    max_bytes: usize,
}

impl PreviewCache {
    pub fn new() -> Self {
        PreviewCache::with_max(512 * 1024 * 1024)
    }

    pub fn with_max(max_bytes: usize) -> Self {
        PreviewCache {
            map: Mutex::new(HashMap::new()),
            bytes: AtomicUsize::new(0),
            max_bytes,
        }
    }

    pub fn get(&self, key: (u32, u8, bool, bool)) -> Option<Arc<Vec<u8>>> {
        self.map.lock().unwrap().get(&key).cloned()
    }

    /// 插入缓存；若插入后总字节超限则整体清空后重新插入本次。
    pub fn insert(&self, key: (u32, u8, bool, bool), buf: Vec<u8>) -> Arc<Vec<u8>> {
        let size = buf.len();
        let mut map = self.map.lock().unwrap();
        if self.bytes.load(Ordering::Relaxed) + size > self.max_bytes {
            map.clear();
            self.bytes.store(0, Ordering::Relaxed);
        }
        let arc = Arc::new(buf);
        self.bytes.fetch_add(size, Ordering::Relaxed);
        map.insert(key, arc.clone());
        arc
    }
}

impl Default for PreviewCache {
    fn default() -> Self {
        Self::new()
    }
}

/// 计算图层非透明像素边界（供测试/工具使用；当前 UI 改用蒙版叠加，保留函数与测试）。
#[allow(dead_code)]
fn scan_bbox(buf: &[u8], w: u32, h: u32) -> Option<LayerBbox> {
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut found = false;
    for y in 0..h {
        for x in 0..w {
            if buf[((y * w + x) * 4 + 3) as usize] != 0 {
                found = true;
                if x < min_x {
                    min_x = x;
                }
                if x > max_x {
                    max_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }
    }
    if found {
        Some(LayerBbox {
            x: min_x,
            y: min_y,
            w: max_x - min_x + 1,
            h: max_y - min_y + 1,
        })
    } else {
        None
    }
}

/// 取单个图层的预览级 RGBA8 缓冲（变换后缩小到长边≤PREVIEW_SCALE）；缓存缺失时构建并入库。
/// 供「选中图层颜色加深」蒙版叠加使用，同时预热预览缓存。
pub fn layer_preview(layer: &LayerState, cache: &PreviewCache) -> Result<(u32, u32, Vec<u8>), String> {
    let key = (layer.id, layer.rotate, layer.flip_h, layer.flip_v);
    if let Some(arc) = cache.get(key) {
        let (tw, th) = transformed_dims(layer);
        let lscale = ((tw.max(th) + PREVIEW_SCALE - 1) / PREVIEW_SCALE).max(1);
        return Ok((tw / lscale, th / lscale, arc.as_ref().clone()));
    }
    let (w, h, rgba) = decode::load_png_rgba8(Path::new(&layer.path))?;
    let transformed = composite::transform_into(&rgba, w, h, layer.rotate, layer.flip_h, layer.flip_v);
    let (tw, th) = transformed_dims(layer);
    let lscale = ((tw.max(th) + PREVIEW_SCALE - 1) / PREVIEW_SCALE).max(1);
    let lw = tw / lscale;
    let lh = th / lscale;
    let small = downsample(&transformed, tw, th, lw, lh);
    let _ = cache.insert(key, small.clone());
    Ok((lw, lh, small))
}

fn transformed_dims(layer: &LayerState) -> (u32, u32) {
    if layer.rotate % 2 == 1 {
        (layer.height, layer.width)
    } else {
        (layer.width, layer.height)
    }
}

/// 合成预览图（长边≤PREVIEW_SCALE），输出 PNG data URL。
/// 1) 遍历可见层，缺缓存的收集为"待构建"列表；
/// 2) 待构建列表 rayon 并行：解码 → 变换 → 箱式缩小 → 存入 cache；
/// 3) 按层序（底→顶）用缓存缓冲预乘合成，输出行 rayon 并行；
/// 4) 预乘转直通、编码 PNG data URL。
pub fn build_preview(snapshot: Snapshot, cache: &PreviewCache) -> Result<PreviewImage, String> {
    let layers = snapshot.layers;
    let (out_w, out_h) = composite::output_size(&layers).ok_or("图层尺寸不一致")?;
    let scale = ((out_w.max(out_h) + PREVIEW_SCALE - 1) / PREVIEW_SCALE).max(1);
    let pv_w = out_w / scale;
    let pv_h = out_h / scale;
    let order = composite::visible_bottom_up(&layers); // 底 → 顶

    // 1) 收集缺缓存的层
    let pending: Vec<&LayerState> = order
        .iter()
        .copied()
        .filter(|layer| cache.get((layer.id, layer.rotate, layer.flip_h, layer.flip_v)).is_none())
        .collect();

    // 2) 并行构建缓存
    if !pending.is_empty() {
        let results: Vec<Result<(), String>> = pending
            .par_iter()
            .map(|layer| {
                let (w, h, rgba) = decode::load_png_rgba8(Path::new(&layer.path))?;
                let transformed =
                    composite::transform_into(&rgba, w, h, layer.rotate, layer.flip_h, layer.flip_v);
                let (tw, th) = if layer.rotate % 2 == 1 { (h, w) } else { (w, h) };
                let lscale = ((tw.max(th) + PREVIEW_SCALE - 1) / PREVIEW_SCALE).max(1);
                let lw = tw / lscale;
                let lh = th / lscale;
                let small = downsample(&transformed, tw, th, lw, lh);
                cache.insert((layer.id, layer.rotate, layer.flip_h, layer.flip_v), small);
                Ok(())
            })
            .collect();
        for r in results {
            r?;
        }
    }

    // 3) 按层序合成（每行独立，rayon 行级并行）
    let mut acc = vec![0u8; (pv_w * pv_h * 4) as usize];
    let nw = pv_w as usize;
    let row_bytes = nw * 4;
    for layer in order {
        let key = (layer.id, layer.rotate, layer.flip_h, layer.flip_v);
        let buf = cache.get(key).ok_or("预览缓存缺失")?;
        let (w, h) = (layer.width, layer.height);
        let (tw, th) = if layer.rotate % 2 == 1 { (h, w) } else { (w, h) };
        let lscale = ((tw.max(th) + PREVIEW_SCALE - 1) / PREVIEW_SCALE).max(1);
        let lw = (tw / lscale) as usize;
        let lh = (th / lscale) as usize;
        acc.par_chunks_mut(row_bytes).enumerate().for_each(|(y, row)| {
            for x in 0..nw {
                let sx = x % lw;
                let sy = y % lh;
                let s_off = (sy * lw + sx) * 4;
                let px4 = [buf[s_off], buf[s_off + 1], buf[s_off + 2], buf[s_off + 3]];
                let dst: &mut [u8; 4] = (&mut row[x * 4..x * 4 + 4]).try_into().unwrap();
                composite::blend_premul8(dst, px4);
            }
        });
    }

    // 4) 预乘 → 直通
    for px in acc.chunks_exact_mut(4) {
        let p = [px[0], px[1], px[2], px[3]];
        let up = composite::unpremultiply8(p);
        px.copy_from_slice(&up);
    }
    // 编码 PNG data URL
    let mut buf = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut buf, pv_w, pv_h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        let mut writer = enc.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&acc).map_err(|e| e.to_string())?;
    }
    Ok(PreviewImage {
        width: pv_w,
        height: pv_h,
        data_url: decode::rgba8_to_data_url(&buf),
    })
}

/// 箱式平均缩小（8bit RGBA）。供预览缓存与 8bit 多分辨率导出共用。
pub fn downsample(src: &[u8], w: u32, h: u32, nw: u32, nh: u32) -> Vec<u8> {
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    for dy in 0..nh {
        for dx in 0..nw {
            let mut acc = [0u64; 4];
            let mut cnt = 0u64;
            let y0 = dy * h / nh;
            let y1 = ((dy + 1) * h / nh).min(h);
            let x0 = dx * w / nw;
            let x1 = ((dx + 1) * w / nw).min(w);
            for y in y0..y1 {
                for x in x0..x1 {
                    let off = ((y * w + x) * 4) as usize;
                    for c in 0..4 {
                        acc[c] += src[off + c] as u64;
                    }
                    cnt += 1;
                }
            }
            let off = ((dy * nw + dx) * 4) as usize;
            for c in 0..4 {
                out[off + c] = (acc[c] / cnt.max(1)) as u8;
            }
        }
    }
    out
}

/// 箱式平均缩小（16bit RGBA）。供 16bit 多分辨率导出使用。
pub fn downsample16(src: &[u16], w: u32, h: u32, nw: u32, nh: u32) -> Vec<u16> {
    let mut out = vec![0u16; (nw * nh * 4) as usize];
    for dy in 0..nh {
        for dx in 0..nw {
            let mut acc = [0u64; 4];
            let mut cnt = 0u64;
            let y0 = dy * h / nh;
            let y1 = ((dy + 1) * h / nh).min(h);
            let x0 = dx * w / nw;
            let x1 = ((dx + 1) * w / nw).min(w);
            for y in y0..y1 {
                for x in x0..x1 {
                    let off = ((y * w + x) * 4) as usize;
                    for c in 0..4 {
                        acc[c] += src[off + c] as u64;
                    }
                    cnt += 1;
                }
            }
            let off = ((dy * nw + dx) * 4) as usize;
            for c in 0..4 {
                out[off + c] = (acc[c] / cnt.max(1)) as u16;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LayerState;
    use std::path::PathBuf;

    fn tmp_png(name: &str, px: &[u8]) -> PathBuf {
        let p = std::env::temp_dir().join(name);
        std::fs::write(&p, decode::make_png_rgba8(2, 2, px)).unwrap();
        p
    }

    #[test]
    fn preview_composites_two_layers() {
        // 底层全红不透明，顶层只左上角蓝
        let bottom = tmp_png("pv_bottom.png", &[255,0,0,255, 255,0,0,255, 255,0,0,255, 255,0,0,255]);
        let top = tmp_png("pv_top.png", &[0,0,255,255, 0,0,0,0, 0,0,0,0, 0,0,0,0]);
        let snapshot = Snapshot {
            layers: vec![
                LayerState { id: 0, path: top.to_string_lossy().into_owned(), width: 2, height: 2, rotate: 0, flip_h: false, flip_v: false, visible: true },
                LayerState { id: 1, path: bottom.to_string_lossy().into_owned(), width: 2, height: 2, rotate: 0, flip_h: false, flip_v: false, visible: true },
            ],
        };
        let cache = PreviewCache::new();
        let img = build_preview(snapshot.clone(), &cache).unwrap();
        assert_eq!((img.width, img.height), (2, 2));
        // 解码回来验证像素
        let png_bytes = data_url_to_bytes(&img.data_url);
        let tmp = std::env::temp_dir().join("pv_out.png");
        std::fs::write(&tmp, png_bytes).unwrap();
        let (_, _, rgba) = decode::load_png_rgba8(&tmp).unwrap();
        // 左上角 = 蓝，其余 = 红
        assert_eq!(&rgba[0..4], &[0, 0, 255, 255]);
        assert_eq!(&rgba[4..8], &[255, 0, 0, 255]);
        // 缓存复用：再次构建不应报错且结果一致
        let img2 = build_preview(snapshot.clone(), &cache).unwrap();
        assert_eq!((img2.width, img2.height), (2, 2));
    }

    #[test]
    fn bbox_scans_opaque_bounds() {
        // 4x4：仅 (1,2)-(2,3) 有非透明像素
        let mut px = vec![0u8; 4 * 4 * 4];
        for y in 2..4 {
            for x in 1..3 {
                let off = (y * 4 + x) * 4;
                px[off + 3] = 255;
            }
        }
        let bbox = scan_bbox(&px, 4, 4).unwrap();
        assert_eq!((bbox.x, bbox.y, bbox.w, bbox.h), (1, 2, 2, 2));
        // 全透明 → None
        assert!(scan_bbox(&vec![0u8; 16], 2, 2).is_none());
    }

    fn data_url_to_bytes(url: &str) -> Vec<u8> {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let b64 = url.split(',').nth(1).unwrap();
        STANDARD.decode(b64).unwrap()
    }
}

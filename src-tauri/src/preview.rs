use std::path::Path;
use crate::composite;
use crate::decode;
use crate::model::{LayerState, PreviewImage, Snapshot};

/// 用缩略图/低分辨率源合成预览图，输出 PNG data URL。
/// 简化实现：直接用全分辨率源按目标尺寸等比缩小（max_dim 限制长边）。
/// 每个可见层独立解码→缩小→预乘混合到累加器。
pub fn build_preview(snapshot: Snapshot, max_dim: u32) -> Result<PreviewImage, String> {
    let layers = snapshot.layers;
    // 输出尺寸 = 顶层尺寸（同尺寸约定），此处以第一个可见层为准
    let (out_w, out_h) = composite::output_size(&layers).ok_or("图层尺寸不一致")?;
    let scale = ((out_w.max(out_h) + max_dim - 1) / max_dim).max(1);
    let pv_w = out_w / scale;
    let pv_h = out_h / scale;

    let mut acc = vec![0u8; (pv_w * pv_h * 4) as usize];
    let order = composite::visible_bottom_up(&layers);
    for layer in order {
        let (w, h, rgba) = decode::load_png_rgba8(Path::new(&layer.path))?;
        // 变换
        let transformed = composite::transform_into(&rgba, w, h, layer.rotate, layer.flip_h, layer.flip_v);
        let (tw, th) = if layer.rotate % 2 == 1 { (h, w) } else { (w, h) };
        // 缩小到预览尺寸（与输出同比例）
        let lscale = ((tw.max(th) + max_dim - 1) / max_dim).max(1);
        let lw = tw / lscale;
        let lh = th / lscale;
        let small = downsample(&transformed, tw, th, lw, lh);
        // 平铺到预览画布（与输出对齐；同尺寸时即 1:1）
        for py in 0..pv_h {
            for px in 0..pv_w {
                let sx = px % lw;
                let sy = py % lh;
                let s_off = ((sy * lw + sx) * 4) as usize;
                let d_off = ((py * pv_w + px) * 4) as usize;
                let px4 = [small[s_off], small[s_off + 1], small[s_off + 2], small[s_off + 3]];
                let dst: &mut [u8; 4] = (&mut acc[d_off..d_off + 4]).try_into().unwrap();
                composite::blend_premul8(dst, px4);
            }
        }
    }
    // 预乘 → 直通
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

/// 箱式平均缩小。
fn downsample(src: &[u8], w: u32, h: u32, nw: u32, nh: u32) -> Vec<u8> {
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

#[cfg(test)]
mod tests {
    use super::*;
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
        let img = build_preview(snapshot, 512).unwrap();
        assert_eq!((img.width, img.height), (2, 2));
        // 解码回来验证像素
        let png_bytes = data_url_to_bytes(&img.data_url);
        let tmp = std::env::temp_dir().join("pv_out.png");
        std::fs::write(&tmp, png_bytes).unwrap();
        let (_, _, rgba) = decode::load_png_rgba8(&tmp).unwrap();
        // 左上角 = 蓝，其余 = 红
        assert_eq!(&rgba[0..4], &[0, 0, 255, 255]);
        assert_eq!(&rgba[4..8], &[255, 0, 0, 255]);
    }

    fn data_url_to_bytes(url: &str) -> Vec<u8> {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let b64 = url.split(',').nth(1).unwrap();
        STANDARD.decode(b64).unwrap()
    }
}

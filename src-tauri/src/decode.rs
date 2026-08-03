use std::io::{BufReader, Cursor};
use std::fs::File;
use std::path::Path;
use png::{BitDepth, ColorType, Decoder, Encoder, Transformations};

/// 读取 PNG 元信息：返回 (宽, 高, 是否为16bit)。
pub fn load_png_meta(path: &Path) -> Result<(u32, u32, bool), String> {
    let file = BufReader::new(File::open(path).map_err(|e| format!("打开失败: {e}"))?);
    let mut dec = Decoder::new(file);
    let reader = dec.read_info().map_err(|e| format!("读取PNG信息失败: {e}"))?;
    let info = reader.info();
    Ok((info.width, info.height, info.bit_depth == BitDepth::Sixteen))
}

/// 解码为 RGBA8（16bit 源高位舍入到低 8 位；灰度/调色板经 EXPAND 展开）。
pub fn load_png_rgba8(path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    let file = BufReader::new(File::open(path).map_err(|e| format!("打开失败: {e}"))?);
    let mut dec = Decoder::new(file);
    dec.set_transformations(Transformations::EXPAND);
    let mut reader = dec.read_info().map_err(|e| format!("解码失败: {e}"))?;
    let w = reader.info().width;
    let h = reader.info().height;
    let mut data = vec![0u8; (w * h * 4) as usize];
    let mut buf = vec![0u8; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).map_err(|e| format!("解码帧失败: {e}"))?;
    if info.color_type == ColorType::Rgba && info.bit_depth == BitDepth::Eight {
        data.copy_from_slice(&buf);
    } else if info.bit_depth == BitDepth::Sixteen {
        // 16bit → 8bit：取高位字节
        for (i, chunk) in buf.chunks_exact(2).enumerate() {
            data[i] = chunk[0];
        }
    } else {
        return Err("不支持的像素格式".into());
    }
    Ok((w, h, data))
}

/// 解码为 RGBA16（8bit 源保留 0..255 范围；16bit 源完整保留）。
pub fn load_png_rgba16(path: &Path) -> Result<(u32, u32, Vec<u16>), String> {
    let file = BufReader::new(File::open(path).map_err(|e| format!("打开失败: {e}"))?);
    let mut dec = Decoder::new(file);
    dec.set_transformations(Transformations::EXPAND);
    let mut reader = dec.read_info().map_err(|e| format!("解码失败: {e}"))?;
    let w = reader.info().width;
    let h = reader.info().height;
    let mut buf = vec![0u8; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).map_err(|e| format!("解码帧失败: {e}"))?;
    if info.color_type != ColorType::Rgba {
        return Err("不支持的像素格式".into());
    }
    let n = (w * h * 4) as usize;
    let mut data = vec![0u16; n];
    if info.bit_depth == BitDepth::Sixteen {
        for (i, chunk) in buf.chunks_exact(2).take(n).enumerate() {
            data[i] = u16::from_be_bytes([chunk[0], chunk[1]]);
        }
    } else {
        for (i, b) in buf.iter().take(n).enumerate() {
            data[i] = *b as u16;
        }
    }
    Ok((w, h, data))
}

/// 生成 ≤max_edge 的正方形缩略图（RGBA8 PNG 字节）。长边等比缩放，箱式平均。
pub fn make_thumbnail(path: &Path, max_edge: u32) -> Result<(u32, u32, Vec<u8>), String> {
    let (w, h, rgba) = load_png_rgba8(path)?;
    let scale = ((w.max(h) + max_edge - 1) / max_edge).max(1);
    let nw = w / scale;
    let nh = h / scale;
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    for dy in 0..nh {
        for dx in 0..nw {
            let mut acc = [0u32; 4];
            let mut cnt = 0u32;
            for sy in dy * scale..((dy + 1) * scale).min(h) {
                for sx in dx * scale..((dx + 1) * scale).min(w) {
                    let off = ((sy * w + sx) * 4) as usize;
                    for c in 0..4 {
                        acc[c] += rgba[off + c] as u32;
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
    // 编码为 PNG
    let mut buf = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut buf, nw, nh);
        enc.set_color(ColorType::Rgba);
        enc.set_depth(BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        let mut writer = enc.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&out).map_err(|e| e.to_string())?;
    }
    Ok((nw, nh, buf))
}

/// RGBA8 数据 → base64 data URL。
pub fn rgba8_to_data_url(png_bytes: &[u8]) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    format!("data:image/png;base64,{}", STANDARD.encode(png_bytes))
}

/// 供测试解码器读取的便捷方法。
pub fn _cursor(c: Vec<u8>) -> Cursor<Vec<u8>> {
    Cursor::new(c)
}

/// RGBA8 PNG 夹具（写入内存）。
pub fn make_png_rgba8(w: u32, h: u32, pixels: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut enc = Encoder::new(&mut buf, w, h);
        enc.set_color(ColorType::Rgba);
        enc.set_depth(BitDepth::Eight);
        let mut writer = enc.write_header().unwrap();
        writer.write_image_data(pixels).unwrap();
    }
    buf
}

/// RGBA16 PNG 夹具。
pub fn make_png_rgba16(w: u32, h: u32, pixels: &[u16]) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut enc = Encoder::new(&mut buf, w, h);
        enc.set_color(ColorType::Rgba);
        enc.set_depth(BitDepth::Sixteen);
        let mut writer = enc.write_header().unwrap();
        let bytes: Vec<u8> = pixels.iter().flat_map(|v| v.to_be_bytes()).collect();
        writer.write_image_data(&bytes).unwrap();
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_meta_8bit() {
        let png = make_png_rgba8(4, 2, &[0u8; 32]);
        let tmp = std::env::temp_dir().join("tm_meta_8.png");
        std::fs::write(&tmp, &png).unwrap();
        let (w, h, is16) = load_png_meta(&tmp).unwrap();
        assert_eq!((w, h, is16), (4, 2, false));
    }

    #[test]
    fn load_meta_16bit() {
        let png = make_png_rgba16(3, 3, &[0u16; 36]);
        let tmp = std::env::temp_dir().join("tm_meta_16.png");
        std::fs::write(&tmp, &png).unwrap();
        let (w, h, is16) = load_png_meta(&tmp).unwrap();
        assert_eq!((w, h, is16), (3, 3, true));
    }

    #[test]
    fn load_rgba8_values() {
        let png = make_png_rgba8(2, 1, &[1, 2, 3, 255, 9, 8, 7, 128]);
        let tmp = std::env::temp_dir().join("tm_val8.png");
        std::fs::write(&tmp, &png).unwrap();
        let (w, h, data) = load_png_rgba8(&tmp).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(&data[..], &[1, 2, 3, 255, 9, 8, 7, 128]);
    }

    #[test]
    fn load_rgba16_from_8bit_keeps_range() {
        let png = make_png_rgba8(1, 1, &[200, 100, 50, 255]);
        let tmp = std::env::temp_dir().join("tm_val16.png");
        std::fs::write(&tmp, &png).unwrap();
        let (w, h, data) = load_png_rgba16(&tmp).unwrap();
        assert_eq!((w, h), (1, 1));
        assert_eq!(&data[..], &[200, 100, 50, 255]);
    }

    #[test]
    fn thumbnail_size_capped() {
        let png = make_png_rgba8(512, 512, &vec![0u8; 512 * 512 * 4]);
        let tmp = std::env::temp_dir().join("tm_thumb.png");
        std::fs::write(&tmp, &png).unwrap();
        let (w, h, out) = make_thumbnail(&tmp, 128).unwrap();
        assert!(w <= 128 && h <= 128);
        assert!(!out.is_empty());
    }

    #[test]
    fn invalid_file_errors() {
        let tmp = std::env::temp_dir().join("tm_bad.png");
        std::fs::write(&tmp, b"not a png at all").unwrap();
        assert!(load_png_meta(&tmp).is_err());
    }
}

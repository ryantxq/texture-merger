use crate::model::LayerState;

/// 8bit 预乘 source-over：把 src 混合进 acc（acc 以预乘形式存储）。
#[inline]
pub fn blend_premul8(acc: &mut [u8; 4], src: [u8; 4]) {
    let a = src[3] as u32;
    if a == 0 {
        return;
    }
    let da = acc[3] as u32;
    if a == 255 {
        acc.copy_from_slice(&src);
        return;
    }
    let inv = 255 - a;
    for i in 0..3 {
        let sp = (src[i] as u32 * a) / 255;
        acc[i] = ((sp + (acc[i] as u32 * inv) / 255).min(255)) as u8;
    }
    acc[3] = ((a + (da * inv) / 255).min(255)) as u8;
}

/// 16bit 预乘 source-over。
#[inline]
pub fn blend_premul16(acc: &mut [u16; 4], src: [u16; 4]) {
    let a = src[3] as u64;
    if a == 0 {
        return;
    }
    let da = acc[3] as u64;
    if a == 65535 {
        acc.copy_from_slice(&src);
        return;
    }
    let inv = 65535 - a;
    for i in 0..3 {
        let sp = (src[i] as u64 * a) / 65535;
        acc[i] = ((sp + (acc[i] as u64 * inv) / 65535).min(65535)) as u16;
    }
    acc[3] = ((a + (da * inv) / 65535).min(65535)) as u16;
}

/// 按变换复制像素：rotate=0/1/2/3 顺时针旋转次数，flip_h/flip_v 水平/垂直翻转。
/// bpp 由 src.len()/(w*h) 推断（4=RGBA8，8=RGBA16）。
pub fn transform_into(src: &[u8], w: u32, h: u32, rotate: u8, flip_h: bool, flip_v: bool) -> Vec<u8> {
    let rotated = rotate % 2 == 1;
    let nw = if rotated { h } else { w };
    let nh = if rotated { w } else { h };
    let bpp = (src.len() as u32 / (w * h)) as usize;
    let mut dst = vec![0u8; (nw * nh) as usize * bpp];
    for y in 0..h {
        for x in 0..w {
            let src_off = ((y * w + x) as usize) * bpp;
            let (dx, dy) = match rotate % 4 {
                0 => (x, y),
                1 => (y, w - 1 - x),
                2 => (w - 1 - x, h - 1 - y),
                _ => (h - 1 - y, x),
            };
            let dx = if flip_h { nw - 1 - dx } else { dx };
            let dy = if flip_v { nh - 1 - dy } else { dy };
            let dst_off = ((dy * nw + dx) as usize) * bpp;
            dst[dst_off..dst_off + bpp].copy_from_slice(&src[src_off..src_off + bpp]);
        }
    }
    dst
}

/// 预乘累加结果 → 直通 alpha 输出（8bit）。
#[inline]
pub fn unpremultiply8(px: [u8; 4]) -> [u8; 4] {
    let a = px[3] as u32;
    if a == 0 || a == 255 {
        return px;
    }
    [
        ((px[0] as u32 * 255) / a).min(255) as u8,
        ((px[1] as u32 * 255) / a).min(255) as u8,
        ((px[2] as u32 * 255) / a).min(255) as u8,
        px[3],
    ]
}

/// 预乘累加结果 → 直通 alpha 输出（16bit）。
#[inline]
pub fn unpremultiply16(px: [u16; 4]) -> [u16; 4] {
    let a = px[3] as u64;
    if a == 0 || a == 65535 {
        return px;
    }
    [
        ((px[0] as u64 * 65535) / a).min(65535) as u16,
        ((px[1] as u64 * 65535) / a).min(65535) as u16,
        ((px[2] as u64 * 65535) / a).min(65535) as u16,
        px[3],
    ]
}

/// 从图层列表中提取「从底到顶、可见」的层（供各处复用）。
pub fn visible_bottom_up(layers: &[LayerState]) -> Vec<&LayerState> {
    layers.iter().rev().filter(|l| l.visible).collect()
}

/// 供外部校验输出尺寸（所有可见层必须同尺寸，旋转只影响宽高交换）。
pub fn output_size(layers: &[LayerState]) -> Option<(u32, u32)> {
    let mut size: Option<(u32, u32)> = None;
    for l in layers.iter().filter(|l| l.visible) {
        let (w, h) = if l.rotate % 2 == 1 { (l.height, l.width) } else { (l.width, l.height) };
        match size {
            None => size = Some((w, h)),
            Some(s) if s == (w, h) => {}
            Some(_) => return None,
        }
    }
    size
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_premul8_src_alpha0_keeps_dst() {
        let mut acc = [100u8, 50, 200, 255];
        blend_premul8(&mut acc, [255, 0, 0, 0]);
        assert_eq!(acc, [100, 50, 200, 255]);
    }

    #[test]
    fn blend_premul8_src_opaque_overwrites() {
        let mut acc = [100u8, 50, 200, 128];
        blend_premul8(&mut acc, [10, 20, 30, 255]);
        assert_eq!(acc, [10, 20, 30, 255]);
    }

    #[test]
    fn blend_premul8_half_alpha() {
        // dst: 半透明红(200,0,0,128)，src: 半透明蓝(0,0,200,128)
        let mut acc = [200u8, 0, 0, 128];
        blend_premul8(&mut acc, [0, 0, 200, 128]);
        // 红通道：(200*127)/255 取整 = 99（计划允许取整差 1 微调断言）
        assert!((99..=100).contains(&acc[0]));
        assert_eq!(acc[1], 0);
        assert_eq!(acc[2], 100);
        assert_eq!(acc[3], 191);
    }

    #[test]
    fn transform_rotate90_swaps_dims() {
        // 2x1 [[1],[2]] 顺时针90° → 1x2 [[2],[1]]
        let src: Vec<u8> = vec![1, 1, 1, 255, 2, 2, 2, 255];
        let out = transform_into(&src, 2, 1, 1, false, false);
        assert_eq!(out.len(), 8);
        assert_eq!(&out[0..4], &[2, 2, 2, 255]);
        assert_eq!(&out[4..8], &[1, 1, 1, 255]);
    }

    #[test]
    fn transform_flip_h() {
        let src: Vec<u8> = vec![1, 1, 1, 255, 2, 2, 2, 255];
        let out = transform_into(&src, 2, 1, 0, true, false);
        assert_eq!(&out[0..4], &[2, 2, 2, 255]);
        assert_eq!(&out[4..8], &[1, 1, 1, 255]);
    }

    #[test]
    fn composite_skips_hidden_and_follows_order() {
        // 底层不透明红，顶层半透明蓝；中间隐藏层不影响
        let layers = vec![
            LayerState { id: 3, path: "".into(), width: 1, height: 1, rotate: 0, flip_h: false, flip_v: false, visible: false },
            LayerState { id: 2, path: "".into(), width: 1, height: 1, rotate: 0, flip_h: false, flip_v: false, visible: true },
            LayerState { id: 1, path: "".into(), width: 1, height: 1, rotate: 0, flip_h: false, flip_v: false, visible: true },
        ];
        let mut acc = [0u8; 4];
        for layer in layers.iter().rev() {
            if !layer.visible { continue; }
            let px = if layer.id == 1 { [255, 0, 0, 255] } else { [0, 0, 255, 128] };
            blend_premul8(&mut acc, px);
        }
        // 先红后蓝(半透明)
        assert_eq!(acc[0], 255 - 128); // 红被压到约 127（(255*128)/255 取整舍入）
        assert_eq!(acc[2], 128);
        assert!(acc[3] == 255);
    }
}

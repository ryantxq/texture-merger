//! 性能基准：300 张 2048×2048 RGBA8 PNG 全量合成 + 导出（8bit / Fast 压缩）。
//!
//! 运行：`cargo run --release --example bench`（release 编译 tauri 依赖较慢，耐心等待）
//! 临时 PNG 生成在 %TEMP%\tm_bench 下，可重复运行（每次自动重建）。
//! 清理临时数据：`Remove-Item "$env:TEMP\tm_bench" -Recurse -Force`

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use texture_merger_lib::decode;
use texture_merger_lib::export;
use texture_merger_lib::model::{Compression, ExportOptions, LayerState, Snapshot};

const COUNT: u32 = 300;
const SIZE: u32 = 2048;

/// 简单 LCG 伪随机（避免引入 rand 依赖）。
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed)
    }
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }
    fn range(&mut self, lo: u32, hi: u32) -> u32 {
        lo + self.next() % (hi - lo + 1)
    }
}

fn main() {
    let dir = std::env::temp_dir().join("tm_bench");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // a) 生成 300 张 PNG（逐张生成，避免同时持有 300×16MB 缓冲）
    println!("== 准备：生成 {COUNT} 张 {SIZE}×{SIZE} RGBA8 PNG 到 {:?}", dir);
    let t_gen = Instant::now();
    let mut rng = Lcg::new(0x9E37_79B9_7F4A_7C15);
    let mut paths: Vec<PathBuf> = Vec::with_capacity(COUNT as usize);
    for i in 0..COUNT {
        // 每张：随机矩形区域外 alpha=0，区域内随机 RGB 且 alpha=255
        let x0 = rng.range(0, SIZE / 2);
        let y0 = rng.range(0, SIZE / 2);
        let x1 = rng.range(SIZE / 2, SIZE - 1);
        let y1 = rng.range(SIZE / 2, SIZE - 1);
        let mut pixels = vec![0u8; (SIZE * SIZE * 4) as usize];
        for y in 0..SIZE {
            for x in 0..SIZE {
                if x >= x0 && x <= x1 && y >= y0 && y <= y1 {
                    let off = ((y * SIZE + x) * 4) as usize;
                    pixels[off] = rng.next() as u8;
                    pixels[off + 1] = rng.next() as u8;
                    pixels[off + 2] = rng.next() as u8;
                    pixels[off + 3] = 255;
                }
            }
        }
        let png = decode::make_png_rgba8(SIZE, SIZE, &pixels);
        let p = dir.join(format!("layer_{i:03}.png"));
        std::fs::write(&p, &png).unwrap();
        paths.push(p);
        if (i + 1) % 50 == 0 {
            println!("  已生成 {}/{}", i + 1, COUNT);
        }
    }
    let gen_ms = t_gen.elapsed().as_millis();
    println!("生成耗时：{} ms（仅准备，不计入指标）", gen_ms);

    // b) 构建 Snapshot 并全量合成 + 导出
    let layers: Vec<LayerState> = paths
        .iter()
        .enumerate()
        .map(|(i, p)| LayerState {
            id: i as u32,
            path: p.to_string_lossy().into_owned(),
            width: SIZE,
            height: SIZE,
            rotate: 0,
            flip_h: false,
            flip_v: false,
            visible: true,
        })
        .collect();
    let snapshot = Snapshot { layers };
    let out = std::env::temp_dir().join("tm_bench_merged.png");
    let options = ExportOptions {
        bit_depth: 8,
        compression: Compression::Fast,
    };
    let cancel = AtomicBool::new(false);

    println!("== 全量合成 + 导出（8bit / Fast 压缩）...");
    let t_export = Instant::now();
    let stats =
        export::run_export_inner(&snapshot, options, &out, &|_, _, _| {}, &cancel).expect("导出失败");
    let export_ms = t_export.elapsed().as_millis();

    // c) 结果
    let out_mb = SIZE * SIZE * 4 / 1024 / 1024;
    println!("== 结果 ==");
    println!("输出尺寸：{}×{}", stats.width, stats.height);
    println!(
        "文件体积：{} 字节（{:.2} MB）",
        stats.bytes_written,
        stats.bytes_written as f64 / 1024.0 / 1024.0
    );
    println!("导出耗时：{} ms（含 300 层解码 + 合成 + PNG 编码 + 写盘）", export_ms);
    println!(
        "内存理论值：输出缓冲 {} MB + 单张解码 {} MB = {} MB（逐层解码复用，不随层数累积）",
        out_mb,
        out_mb,
        out_mb * 2
    );

    // 清理输出文件（bench 可重复运行，临时 PNG 留在 %TEMP% 供查看，脚本外清理）
    let _ = std::fs::remove_file(&out);
    println!("== 完成（临时 PNG 保留在 {:?}，可用 Remove-Item 清理）", dir);
}

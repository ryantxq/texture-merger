use serde::{Deserialize, Serialize};

/// 图层：index 0 = 最上层（合成时从尾部遍历）。
/// 说明：`rename_all = "camelCase"` 使前端传来的 flipH/flipV 等驼峰字段
/// 能正确反序列化为 flip_h/flip_v（Tauri v2 仅对命令顶层参数自动驼峰转换，
/// 嵌套结构需结构体自身声明）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerState {
    pub id: u32,
    pub path: String,
    pub width: u32,
    pub height: u32,
    /// 顺时针旋转次数 0/1/2/3
    pub rotate: u8,
    pub flip_h: bool,
    pub flip_v: bool,
    pub visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub layers: Vec<LayerState>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Compression {
    Fast,
    Balanced,
    Best,
}

impl Compression {
    pub fn png_level(&self) -> png::Compression {
        match self {
            Compression::Fast => png::Compression::Fast,
            Compression::Balanced => png::Compression::Balanced,
            Compression::Best => png::Compression::High,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    pub bit_depth: u8, // 8 或 16
    pub compression: Compression,
}

/// 导入结果：ok 带图层元信息（含 base64 缩略图）；error 标记失败项。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum ImportItem {
    Ok {
        id: u32,
        path: String,
        name: String,
        width: u32,
        height: u32,
        #[serde(rename = "hasAlpha")]
        has_alpha: bool,
        thumbnail: String, // data:image/png;base64,...
    },
    Error {
        path: String,
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewImage {
    pub width: u32,
    pub height: u32,
    pub data_url: String, // data:image/png;base64,...
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportStats {
    pub width: u32,
    pub height: u32,
    pub bytes_written: u64,
    pub duration_ms: u128,
}

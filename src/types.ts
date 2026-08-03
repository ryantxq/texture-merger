// src/types.ts
export type LayerMeta = {
  status: "ok";
  id: number;
  path: string;
  name: string;
  width: number;
  height: number;
  hasAlpha: boolean;
  thumbnail: string;
};
export type ImportErrorItem = { status: "error"; path: string; reason: string };
export type ImportItem = LayerMeta | ImportErrorItem;

export type LayerState = {
  id: number;
  path: string;
  name: string;
  width: number;
  height: number;
  rotate: 0 | 1 | 2 | 3;
  flipH: boolean;
  flipV: boolean;
  visible: boolean;
  /** 前端补挂的缩略图 data URL（导入时由 ImportItem.thumbnail 赋值），供图层列表直接使用 */
  thumbnailUrl?: string;
};

export type ExportOptions = {
  bitDepth: 8 | 16;
  compression: "fast" | "balanced" | "best";
  /** 目标长边像素尺寸列表（含原尺寸；0 会被后端清洗掉） */
  sizes: number[];
};
export type Snapshot = { layers: LayerState[] };

/** 预览/缩略图背景：棋盘格（checkerA/B 两色）或单色 */
export type PreviewBg = {
  mode: "checker" | "solid";
  checkerA: string;
  checkerB: string;
  solid: string;
};

/** 单个分辨率导出结果（sizes 中每个目标尺寸对应一项） */
export type ExportFile = {
  size: number;
  width: number;
  height: number;
  bytesWritten: number;
  durationMs: number;
};

export type AppState = {
  layers: LayerState[];
  selectedId: number | null;
  soloMode: boolean;
  exportOptions: ExportOptions;
  status: string;
  previewProgress: number;
  exportProgress: number | null;
  /** 导入进度（按图片数），无导入任务时为 null */
  importProgress: { done: number; total: number } | null;
  /** 预览背景（棋盘格/单色） */
  previewBg: PreviewBg;
};

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

export type ExportOptions = { bitDepth: 8 | 16; compression: "fast" | "balanced" | "best" };
export type Snapshot = { layers: LayerState[] };

export type AppState = {
  layers: LayerState[];
  selectedId: number | null;
  soloMode: boolean;
  exportOptions: ExportOptions;
  status: string;
  previewProgress: number;
  exportProgress: number | null;
};

// src/api.ts
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ExportFile, ExportOptions, ImportItem, LayerBbox, LayerState, Snapshot } from "./types";

export async function importFiles(paths: string[], nextId: number): Promise<ImportItem[]> {
  return invoke("import_files", { paths, nextId });
}

export async function importFolder(path: string, nextId: number): Promise<ImportItem[]> {
  return invoke("import_folder", { path, nextId });
}

export async function buildPreview(snapshot: Snapshot): Promise<{ width: number; height: number; dataUrl: string }> {
  return invoke("build_preview", { snapshot });
}

export async function exportImage(
  snapshot: Snapshot,
  options: ExportOptions,
  dir: string,
  baseStem: string
): Promise<ExportFile[]> {
  return invoke("export_image", { snapshot, options, dir, baseStem });
}

export async function getLayerBbox(layer: LayerState): Promise<LayerBbox | null> {
  return invoke("get_layer_bbox", { layer });
}

export async function cancelExport(): Promise<void> {
  return invoke("cancel_export");
}

export async function revealInFolder(path: string): Promise<void> {
  return invoke("reveal_in_folder", { path });
}

export async function onExportProgress(
  cb: (p: { phase: string; progress: number; info: string }) => void
): Promise<UnlistenFn> {
  return listen("export-progress", (e) => cb(e.payload as { phase: string; progress: number; info: string }));
}

export async function onImportProgress(
  cb: (p: { done: number; total: number }) => void
): Promise<UnlistenFn> {
  return listen("import-progress", (e) => cb(e.payload as { done: number; total: number }));
}

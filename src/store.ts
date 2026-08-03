// src/store.ts
import type { AppState, ImportItem, LayerState } from "./types";

export const initialState: AppState = {
  layers: [],
  selectedId: null,
  soloMode: false,
  exportOptions: { bitDepth: 8, compression: "balanced", sizes: [] },
  status: "就绪",
  previewProgress: 0,
  exportProgress: null,
  importProgress: null,
  previewBg: { mode: "checker", checkerA: "#e3e6ea", checkerB: "#ffffff", solid: "#ffffff" },
};

export type Action =
  | { type: "addLayers"; items: ImportItem[] }
  | { type: "removeLayer"; id: number }
  | { type: "replaceLayer"; id: number; item: Extract<ImportItem, { status: "ok" }> }
  | { type: "moveLayer"; from: number; to: number }
  | { type: "toggleVisible"; id: number }
  | { type: "rotate"; id: number }
  | { type: "flipH"; id: number }
  | { type: "flipV"; id: number }
  | { type: "clearLayers" }
  | { type: "select"; id: number | null }
  | { type: "setSolo"; solo: boolean }
  | { type: "setExportOptions"; options: AppState["exportOptions"] }
  | { type: "setStatus"; status: string }
  | { type: "setExportProgress"; progress: number | null }
  | { type: "setImportProgress"; progress: { done: number; total: number } | null }
  | { type: "setPreviewBg"; bg: AppState["previewBg"] };

export function reducer(state: AppState, action: Action): AppState {
  switch (action.type) {
    case "addLayers": {
      const okItems = action.items.filter(
        (i): i is Extract<ImportItem, { status: "ok" }> => i.status === "ok"
      );
      const newLayers: LayerState[] = okItems.map((i) => ({
        id: i.id,
        path: i.path,
        name: i.name,
        width: i.width,
        height: i.height,
        rotate: 0,
        flipH: false,
        flipV: false,
        visible: true,
        thumbnailUrl: i.thumbnail,
      }));
      // 新图层加到最上层（数组头部）
      return { ...state, layers: [...newLayers, ...state.layers] };
    }
    case "removeLayer":
      return {
        ...state,
        layers: state.layers.filter((l) => l.id !== action.id),
        selectedId: state.selectedId === action.id ? null : state.selectedId,
      };
    case "replaceLayer":
      return {
        ...state,
        layers: state.layers.map((l) =>
          l.id === action.id
            ? { ...l, path: action.item.path, name: action.item.name, width: action.item.width, height: action.item.height }
            : l
        ),
      };
    case "moveLayer": {
      const arr = [...state.layers];
      const [moved] = arr.splice(action.from, 1);
      arr.splice(action.to, 0, moved);
      return { ...state, layers: arr };
    }
    case "toggleVisible":
      return {
        ...state,
        layers: state.layers.map((l) => (l.id === action.id ? { ...l, visible: !l.visible } : l)),
      };
    case "rotate":
      return {
        ...state,
        layers: state.layers.map((l) =>
          l.id === action.id ? { ...l, rotate: ((l.rotate + 1) % 4) as 0 | 1 | 2 | 3 } : l
        ),
      };
    case "flipH":
      return { ...state, layers: state.layers.map((l) => (l.id === action.id ? { ...l, flipH: !l.flipH } : l)) };
    case "flipV":
      return { ...state, layers: state.layers.map((l) => (l.id === action.id ? { ...l, flipV: !l.flipV } : l)) };
    case "clearLayers":
      return { ...state, layers: [], selectedId: null, soloMode: false };
    case "select":
      return { ...state, selectedId: action.id };
    case "setSolo":
      return { ...state, soloMode: action.solo };
    case "setExportOptions":
      return { ...state, exportOptions: action.options };
    case "setStatus":
      return { ...state, status: action.status };
    case "setExportProgress":
      return { ...state, exportProgress: action.progress };
    case "setImportProgress":
      return { ...state, importProgress: action.progress };
    case "setPreviewBg":
      return { ...state, previewBg: action.bg };
    default:
      return state;
  }
}

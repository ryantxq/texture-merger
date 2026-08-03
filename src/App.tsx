// src/App.tsx
import { useEffect, useMemo, useReducer, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { buildPreview, importFiles, importFolder, onExportProgress, onImportProgress } from "./api";
import { initialState, reducer } from "./store";
import type { PreviewBg, Snapshot } from "./types";
import Toolbar from "./components/Toolbar";
import LayerList from "./components/LayerList";
import PreviewCanvas from "./components/PreviewCanvas";
import ExportDialog from "./components/ExportDialog";
import AboutDialog from "./components/AboutDialog";
import StatusBar from "./components/StatusBar";

const THEME_KEY = "theme";
const PREVIEW_BG_KEY = "previewBg";
const PANEL_WIDTH_KEY = "panelWidth";
const MIN_PANEL = 220;
const MAX_PANEL = 520;
type Theme = "light" | "dark";

function initialTheme(): Theme {
  const saved = localStorage.getItem(THEME_KEY);
  if (saved === "light" || saved === "dark") return saved;
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

/** 读取持久化的预览背景，缺省字段回退默认值 */
function initialPreviewBg(): PreviewBg {
  const saved = localStorage.getItem(PREVIEW_BG_KEY);
  if (saved) {
    try {
      const p = JSON.parse(saved) as Partial<PreviewBg>;
      return {
        mode: p.mode === "solid" ? "solid" : "checker",
        checkerA: typeof p.checkerA === "string" ? p.checkerA : initialState.previewBg.checkerA,
        checkerB: typeof p.checkerB === "string" ? p.checkerB : initialState.previewBg.checkerB,
        solid: typeof p.solid === "string" ? p.solid : initialState.previewBg.solid,
      };
    } catch {
      // 解析失败回退默认
    }
  }
  return initialState.previewBg;
}

function initialPanelWidth(): number {
  const saved = Number(localStorage.getItem(PANEL_WIDTH_KEY));
  if (Number.isFinite(saved)) return Math.min(MAX_PANEL, Math.max(MIN_PANEL, saved));
  return 280;
}

export default function App() {
  const [state, dispatch] = useReducer(reducer, initialState, (s) => ({ ...s, previewBg: initialPreviewBg() }));
  const [theme, setTheme] = useState<Theme>(initialTheme);
  const [panelWidth, setPanelWidth] = useState(initialPanelWidth);
  const [preview, setPreview] = useState<{ dataUrl: string; width: number; height: number } | null>(null);
  const [showExport, setShowExport] = useState(false);
  const [showAbout, setShowAbout] = useState(false);
  const idRef = useRef(0);
  const previewSeq = useRef(0);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem(THEME_KEY, theme);
  }, [theme]);

  // 预览背景持久化
  useEffect(() => {
    localStorage.setItem(PREVIEW_BG_KEY, JSON.stringify(state.previewBg));
  }, [state.previewBg]);

  // 图层面板宽度持久化
  useEffect(() => {
    localStorage.setItem(PANEL_WIDTH_KEY, String(panelWidth));
  }, [panelWidth]);

  // 导出进度监听
  useEffect(() => {
    let un: (() => void) | undefined;
    onExportProgress((p) => dispatch({ type: "setExportProgress", progress: p.progress })).then((fn) => (un = fn));
    return () => un?.();
  }, []);

  // 导入进度监听
  useEffect(() => {
    let un: (() => void) | undefined;
    onImportProgress((p) => dispatch({ type: "setImportProgress", progress: p })).then((fn) => (un = fn));
    return () => un?.();
  }, []);

  // 防抖预览刷新：任何图层变化后 300ms 重建预览（Solo 模式忽略）
  const snapshot = useMemo<Snapshot>(
    () => ({ layers: state.layers.map((l) => ({ ...l })) }),
    [state.layers]
  );
  useEffect(() => {
    if (state.layers.length === 0) {
      setPreview(null);
      return;
    }
    const seq = ++previewSeq.current;
    dispatch({ type: "setStatus", status: "预览刷新中…" });
    const timer = setTimeout(async () => {
      try {
        const layers = state.soloMode && state.selectedId != null
          ? snapshot.layers.filter((l) => l.id === state.selectedId)
          : snapshot.layers;
        const img = await buildPreview({ layers });
        if (seq === previewSeq.current) {
          setPreview(img);
          dispatch({ type: "setStatus", status: "就绪" });
        }
      } catch (e) {
        dispatch({ type: "setStatus", status: `预览失败: ${e}` });
      }
    }, 300);
    return () => clearTimeout(timer);
  }, [snapshot, state.soloMode, state.selectedId]);

  // 拖入文件：注册 Tauri 拖放事件（文件→导入，文件夹→扫描导入）
  useEffect(() => {
    const un = getCurrentWindow().onDragDropEvent((event) => {
      if (event.payload.type === "over") return;
      if (event.payload.type === "drop") {
        const paths = event.payload.paths;
        const folders = paths.filter((p) => !/\.png$/i.test(p));
        const files = paths.filter((p) => /\.png$/i.test(p));
        if (files.length > 0) void handleImport(files);
        for (const f of folders) void handleImportFolder(f);
      }
    });
    return () => { un.then((f) => f()); };
  }, []);

  async function handleImport(paths: string[]) {
    if (paths.length === 0) return;
    dispatch({ type: "setStatus", status: "导入中…" });
    // 立即显示初始进度（后端事件到达后更新）
    dispatch({ type: "setImportProgress", progress: { done: 0, total: paths.length } });
    try {
      const items = await importFiles(paths, idRef.current);
      const okCount = items.filter((i) => i.status === "ok").length;
      idRef.current += okCount;
      dispatch({ type: "addLayers", items });
      const bad = items.filter((i) => i.status === "error");
      if (bad.length > 0) {
        dispatch({ type: "setStatus", status: `导入完成：成功 ${okCount}，失败 ${bad.length}（${bad[0].path.split(/[\\/]/).pop()}）` });
      } else {
        dispatch({ type: "setStatus", status: `已导入 ${okCount} 张` });
      }
    } catch (e) {
      dispatch({ type: "setStatus", status: `导入失败: ${e}` });
    } finally {
      dispatch({ type: "setImportProgress", progress: null });
    }
  }

  async function handleImportFolder(path: string) {
    dispatch({ type: "setStatus", status: "扫描文件夹…" });
    // 文件总数需扫描后才知道：先用 1 占位，后端扫描完成后事件会带真实 total 更新
    dispatch({ type: "setImportProgress", progress: { done: 0, total: 1 } });
    try {
      const items = await importFolder(path, idRef.current);
      const okCount = items.filter((i) => i.status === "ok").length;
      idRef.current += okCount;
      dispatch({ type: "addLayers", items });
      dispatch({ type: "setStatus", status: `已从文件夹导入 ${okCount} 张` });
    } catch (e) {
      dispatch({ type: "setStatus", status: `导入失败: ${e}` });
    } finally {
      dispatch({ type: "setImportProgress", progress: null });
    }
  }

  return (
    <div className="app">
      <Toolbar
        onImportFiles={handleImport}
        onImportFolder={handleImportFolder}
        onExport={() => setShowExport(true)}
        onAbout={() => setShowAbout(true)}
        theme={theme}
        onToggleTheme={() => setTheme((t) => (t === "light" ? "dark" : "light"))}
        exportOptions={state.exportOptions}
        onExportOptions={(o) => dispatch({ type: "setExportOptions", options: o })}
      />
      <div className="main">
        <LayerList
          layers={state.layers}
          selectedId={state.selectedId}
          onSelect={(id) => dispatch({ type: "select", id })}
          onSolo={(solo) => dispatch({ type: "setSolo", solo })}
          onMove={(from, to) => dispatch({ type: "moveLayer", from, to })}
          onRemove={(id) => dispatch({ type: "removeLayer", id })}
          onReplace={(id, item) => dispatch({ type: "replaceLayer", id, item })}
          onToggleVisible={(id) => dispatch({ type: "toggleVisible", id })}
          onRotate={(id) => dispatch({ type: "rotate", id })}
          onFlipV={(id) => dispatch({ type: "flipV", id })}
          onClear={() => dispatch({ type: "clearLayers" })}
          panelWidth={panelWidth}
          onPanelWidth={setPanelWidth}
          previewBg={state.previewBg}
        />
        <div className="canvas-area">
          <PreviewCanvas
            preview={preview}
            solo={state.soloMode}
            soloName={state.layers.find((l) => l.id === state.selectedId)?.name}
            selectedLayer={state.layers.find((l) => l.id === state.selectedId) ?? null}
            onExitSolo={() => dispatch({ type: "setSolo", solo: false })}
            previewBg={state.previewBg}
            onPreviewBg={(bg) => dispatch({ type: "setPreviewBg", bg })}
          />
        </div>
      </div>
      <StatusBar
        layerCount={state.layers.length}
        outputSize={state.layers[0] ? { width: state.layers[0].width, height: state.layers[0].height } : null}
        previewSize={preview ? { width: preview.width, height: preview.height } : null}
        status={state.status}
        exportProgress={state.exportProgress}
        importProgress={state.importProgress}
        bitDepth={state.exportOptions.bitDepth}
      />
      {showExport && <ExportDialog state={state} snapshot={snapshot} onClose={() => setShowExport(false)} />}
      {showAbout && <AboutDialog onClose={() => setShowAbout(false)} />}
    </div>
  );
}

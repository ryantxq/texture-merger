// src/App.tsx
import { useEffect, useMemo, useReducer, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { buildPreview, importFiles, importFolder, onExportProgress } from "./api";
import { initialState, reducer } from "./store";
import type { Snapshot } from "./types";
import Toolbar from "./components/Toolbar";
import LayerList from "./components/LayerList";
import PreviewCanvas from "./components/PreviewCanvas";
import ExportDialog from "./components/ExportDialog";
import AboutDialog from "./components/AboutDialog";
import StatusBar from "./components/StatusBar";

const THEME_KEY = "theme";
type Theme = "light" | "dark";

function initialTheme(): Theme {
  const saved = localStorage.getItem(THEME_KEY);
  if (saved === "light" || saved === "dark") return saved;
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export default function App() {
  const [state, dispatch] = useReducer(reducer, initialState);
  const [theme, setTheme] = useState<Theme>(initialTheme);
  const [preview, setPreview] = useState<{ dataUrl: string; width: number; height: number } | null>(null);
  const [showExport, setShowExport] = useState(false);
  const [showAbout, setShowAbout] = useState(false);
  const idRef = useRef(0);
  const previewSeq = useRef(0);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem(THEME_KEY, theme);
  }, [theme]);

  // 导出进度监听
  useEffect(() => {
    let un: (() => void) | undefined;
    onExportProgress((p) => dispatch({ type: "setExportProgress", progress: p.progress })).then((fn) => (un = fn));
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
        const img = await buildPreview({ layers }, 1024);
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
  }

  async function handleImportFolder(path: string) {
    dispatch({ type: "setStatus", status: "扫描文件夹…" });
    const items = await importFolder(path, idRef.current);
    const okCount = items.filter((i) => i.status === "ok").length;
    idRef.current += okCount;
    dispatch({ type: "addLayers", items });
    dispatch({ type: "setStatus", status: `已从文件夹导入 ${okCount} 张` });
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
          onFlipH={(id) => dispatch({ type: "flipH", id })}
          onFlipV={(id) => dispatch({ type: "flipV", id })}
          onClear={() => dispatch({ type: "clearLayers" })}
        />
        <div className="canvas-area">
          <PreviewCanvas
            preview={preview}
            solo={state.soloMode}
            soloName={state.layers.find((l) => l.id === state.selectedId)?.name}
            onExitSolo={() => dispatch({ type: "setSolo", solo: false })}
          />
        </div>
      </div>
      <StatusBar
        layerCount={state.layers.length}
        outputSize={state.layers[0] ? { width: state.layers[0].width, height: state.layers[0].height } : null}
        previewSize={preview ? { width: preview.width, height: preview.height } : null}
        status={state.status}
        exportProgress={state.exportProgress}
        bitDepth={state.exportOptions.bitDepth}
      />
      {showExport && <ExportDialog state={state} snapshot={snapshot} onClose={() => setShowExport(false)} />}
      {showAbout && <AboutDialog onClose={() => setShowAbout(false)} />}
    </div>
  );
}

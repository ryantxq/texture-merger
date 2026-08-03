// src/components/LayerList.tsx
import { useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { open } from "@tauri-apps/plugin-dialog";
import type { ImportItem, LayerState, PreviewBg } from "../types";
import { importFiles } from "../api";

type Props = {
  layers: LayerState[];
  selectedId: number | null;
  onSelect: (id: number | null) => void;
  onSolo: (solo: boolean) => void;
  onMove: (from: number, to: number) => void;
  onRemove: (id: number) => void;
  onReplace: (id: number, item: Extract<ImportItem, { status: "ok" }>) => void;
  onToggleVisible: (id: number) => void;
  onRotate: (id: number) => void;
  onFlipV: (id: number) => void;
  onClear: () => void;
  panelWidth: number;
  onPanelWidth: (w: number) => void;
  previewBg: PreviewBg;
};

const ROW_HEIGHT = 52;
const MIN_PANEL = 220;
const MAX_PANEL = 520;

/** 可见性图标：睁眼/闭眼（闭眼加斜线） */
function EyeIcon({ open: eyeOpen }: { open: boolean }) {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
      <path d="M1 12s4-7 11-7 11 7 11 7-4 7-11 7S1 12 1 12z" />
      <circle cx="12" cy="12" r="3" />
      {!eyeOpen && <line x1="3" y1="3" x2="21" y2="21" />}
    </svg>
  );
}

export default function LayerList(props: Props) {
  const parentRef = useRef<HTMLDivElement>(null);
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [overIndex, setOverIndex] = useState<number | null>(null);
  const [showClearConfirm, setShowClearConfirm] = useState(false);
  const [resizing, setResizing] = useState(false);
  // 拖拽调宽：ref 保存回调，避免 window 监听器闭包陈旧
  const onPanelWidthRef = useRef(props.onPanelWidth);
  onPanelWidthRef.current = props.onPanelWidth;
  const resizeRef = useRef<{ startX: number; startW: number } | null>(null);

  const virtualizer = useVirtualizer({
    count: props.layers.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 10,
  });

  async function handleReplace(id: number) {
    const selected = await open({ multiple: false, filters: [{ name: "PNG 贴图", extensions: ["png"] }] });
    if (!selected || Array.isArray(selected)) return;
    const items = await importFiles([selected], 0);
    const ok = items.find((i) => i.status === "ok");
    if (ok && ok.status === "ok") props.onReplace(id, ok);
  }

  function startResize(e: React.MouseEvent) {
    e.preventDefault();
    resizeRef.current = { startX: e.clientX, startW: props.panelWidth };
    setResizing(true);
    const onMove = (ev: MouseEvent) => {
      if (!resizeRef.current) return;
      const w = Math.min(MAX_PANEL, Math.max(MIN_PANEL, resizeRef.current.startW + (ev.clientX - resizeRef.current.startX)));
      onPanelWidthRef.current(w);
    };
    const onUp = () => {
      resizeRef.current = null;
      setResizing(false);
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }

  const thumbStyle =
    props.previewBg.mode === "checker"
      ? {
          backgroundImage: `repeating-conic-gradient(${props.previewBg.checkerA} 0% 25%, ${props.previewBg.checkerB} 0% 50%)`,
          backgroundSize: "10px 10px",
        }
      : { backgroundColor: props.previewBg.solid };

  return (
    <div className="layer-panel" style={{ width: props.panelWidth }}>
      <div className={"panel-resizer" + (resizing ? " active" : "")} onMouseDown={startResize} title="拖拽调整宽度" />
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "8px 12px", borderBottom: "1px solid var(--border-2)", color: "var(--text-3)", fontSize: 12 }}>
        <span><b style={{ color: "var(--text)" }}>图层</b> · 共 {props.layers.length}</span>
        <button className="btn" style={{ padding: "2px 8px" }} onClick={() => setShowClearConfirm(true)}>清空</button>
      </div>
      <div ref={parentRef} style={{ flex: 1, overflow: "auto" }}>
        <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
          {virtualizer.getVirtualItems().map((vi) => {
            const layer = props.layers[vi.index];
            return (
              <div
                key={layer.id}
                data-index={vi.index}
                draggable
                onDragStart={(e) => {
                  setDragIndex(vi.index);
                  e.dataTransfer.effectAllowed = "move";
                }}
                onDragOver={(e) => {
                  e.preventDefault();
                  setOverIndex(vi.index);
                }}
                onDrop={(e) => {
                  e.preventDefault();
                  if (dragIndex != null && dragIndex !== vi.index) props.onMove(dragIndex, vi.index);
                  setDragIndex(null);
                  setOverIndex(null);
                }}
                onDragEnd={() => { setDragIndex(null); setOverIndex(null); }}
                onClick={() => props.onSelect(layer.id)}
                onDoubleClick={() => {
                  props.onSelect(layer.id);
                  props.onSolo(true);
                }}
                className={
                  "layer-item" +
                  (props.selectedId === layer.id ? " selected" : "") +
                  (overIndex === vi.index ? " over" : "")
                }
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: "100%",
                  transform: `translateY(${vi.start}px)`,
                  opacity: dragIndex === vi.index ? 0.4 : 1,
                  borderTop: overIndex === vi.index && dragIndex != null && dragIndex < vi.index ? "2px solid var(--accent)" : undefined,
                  borderBottom: overIndex === vi.index && dragIndex != null && dragIndex > vi.index ? "2px solid var(--accent)" : undefined,
                }}
              >
                <button
                  className={"vis-btn" + (layer.visible ? "" : " hidden")}
                  title="显示/隐藏"
                  onClick={(e) => { e.stopPropagation(); props.onToggleVisible(layer.id); }}
                >
                  <EyeIcon open={layer.visible} />
                </button>
                <img className="thumb" src={layer.thumbnailUrl} alt="" style={thumbStyle} />
                <span style={{ flex: 1, minWidth: 0, overflow: "hidden" }}>
                  <span style={{ display: "block", overflow: "hidden", whiteSpace: "nowrap", textOverflow: "ellipsis" }}>
                    {layer.name}
                  </span>
                  <span style={{ display: "block", color: "var(--text-3)", fontSize: 11, marginTop: 2 }}>
                    {layer.width}×{layer.height}
                  </span>
                </span>
                <span className="ops">
                  <button className="btn icon" title="旋转90°" onClick={(e) => { e.stopPropagation(); props.onRotate(layer.id); }}>↻</button>
                  <button className="btn icon" title="垂直翻转" onClick={(e) => { e.stopPropagation(); props.onFlipV(layer.id); }}>⇵</button>
                  <button className="btn icon" title="替换" onClick={(e) => { e.stopPropagation(); handleReplace(layer.id); }}>⟳</button>
                  <button className="btn icon" title="移除" onClick={(e) => { e.stopPropagation(); props.onRemove(layer.id); }}>×</button>
                </span>
              </div>
            );
          })}
        </div>
      </div>
      <div style={{ padding: "6px 12px", color: "var(--text-3)", fontSize: 11, borderTop: "1px solid var(--border-2)" }}>
        双击图层可单独查看 · 拖拽调整顺序
      </div>
      {showClearConfirm && (
        <div className="modal-mask" onClick={() => setShowClearConfirm(false)}>
          <div className="modal" style={{ width: 320 }} onClick={(e) => e.stopPropagation()}>
            <div className="modal-head">
              清空图层
              <button className="btn icon" onClick={() => setShowClearConfirm(false)}>✕</button>
            </div>
            <div className="modal-body">确定清空全部 {props.layers.length} 个图层吗？此操作不可恢复。</div>
            <div className="modal-foot">
              <button className="btn" onClick={() => setShowClearConfirm(false)}>取消</button>
              <button
                className="btn primary"
                onClick={() => {
                  setShowClearConfirm(false);
                  props.onClear();
                }}
              >
                清空
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

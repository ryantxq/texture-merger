// src/components/LayerList.tsx
import { useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { open } from "@tauri-apps/plugin-dialog";
import type { ImportItem, LayerState } from "../types";
import { importFiles } from "../api";

type Props = {
  layers: LayerState[];
  selectedId: number | null;
  onSelect: (id: number) => void;
  onSolo: (solo: boolean) => void;
  onMove: (from: number, to: number) => void;
  onRemove: (id: number) => void;
  onReplace: (id: number, item: Extract<ImportItem, { status: "ok" }>) => void;
  onToggleVisible: (id: number) => void;
  onRotate: (id: number) => void;
  onFlipH: (id: number) => void;
  onFlipV: (id: number) => void;
  onClear: () => void;
};

export default function LayerList(props: Props) {
  const parentRef = useRef<HTMLDivElement>(null);
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [overIndex, setOverIndex] = useState<number | null>(null);

  const virtualizer = useVirtualizer({
    count: props.layers.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 46,
    overscan: 10,
  });

  async function handleReplace(id: number) {
    const selected = await open({ multiple: false, filters: [{ name: "PNG 贴图", extensions: ["png"] }] });
    if (!selected || Array.isArray(selected)) return;
    const items = await importFiles([selected], 0);
    const ok = items.find((i) => i.status === "ok");
    if (ok && ok.status === "ok") props.onReplace(id, ok);
  }

  return (
    <div className="layer-panel">
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "8px 12px", borderBottom: "1px solid var(--border-2)", color: "var(--text-3)", fontSize: 12 }}>
        <span><b style={{ color: "var(--text)" }}>图层</b> · 共 {props.layers.length}</span>
        <button className="btn" style={{ padding: "2px 8px" }} onClick={props.onClear}>清空</button>
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
                  className="btn icon"
                  title="显示/隐藏"
                  style={{ width: 24, padding: 0, opacity: layer.visible ? 1 : 0.3 }}
                  onClick={(e) => { e.stopPropagation(); props.onToggleVisible(layer.id); }}
                >
                  {layer.visible ? "●" : "○"}
                </button>
                <img className="thumb" src={layer.thumbnailUrl} alt="" />
                <span style={{ flex: 1, overflow: "hidden", whiteSpace: "nowrap", textOverflow: "ellipsis" }}>
                  {layer.name}
                  <span style={{ color: "var(--text-3)", marginLeft: 4, fontSize: 11 }}>
                    {layer.width}×{layer.height}
                  </span>
                </span>
                <span style={{ display: "flex", gap: 2, color: "var(--text-3)", fontSize: 12 }}>
                  <button className="btn icon" title="旋转90°" onClick={(e) => { e.stopPropagation(); props.onRotate(layer.id); }}>↻</button>
                  <button className="btn icon" title="水平翻转" onClick={(e) => { e.stopPropagation(); props.onFlipH(layer.id); }}>⇋</button>
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
    </div>
  );
}

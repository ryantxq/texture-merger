// src/components/PreviewCanvas.tsx
import { useEffect, useRef, useState } from "react";
import { getLayerBbox } from "../api";
import type { LayerBbox, LayerState } from "../types";

type Props = {
  preview: { dataUrl: string; width: number; height: number } | null;
  solo: boolean;
  soloName?: string;
  selectedLayer: LayerState | null;
  onExitSolo: () => void;
};

/** bbox 模块级缓存：键 = `${id}_${rotate}_${flipH}_${flipV}`（与后端 bbox 缓存键一致，避免重复 IPC/解码） */
const bboxCache = new Map<string, LayerBbox | null>();

function bboxKey(layer: LayerState): string {
  return `${layer.id}_${layer.rotate}_${layer.flipH}_${layer.flipV}`;
}

let cachedAccent: string | null = null;
function accentColor(): string {
  if (cachedAccent) return cachedAccent;
  cachedAccent = getComputedStyle(document.documentElement).getPropertyValue("--accent").trim() || "#3b7bff";
  return cachedAccent;
}

export default function PreviewCanvas({ preview, solo, soloName, selectedLayer, onExitSolo }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const [zoom, setZoom] = useState(1);
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const imgRef = useRef<HTMLImageElement | null>(null);
  const dragRef = useRef<{ startX: number; startY: number; ox: number; oy: number } | null>(null);
  // 选中图层的高亮框（{ key, bbox }）；bbox 为变换后全分辨率源像素坐标，null 表示全透明层
  const [highlight, setHighlight] = useState<{ key: string; bbox: LayerBbox | null } | null>(null);

  // 加载预览图
  useEffect(() => {
    if (!preview) {
      const c = canvasRef.current;
      c?.getContext("2d")?.clearRect(0, 0, c.width, c.height);
      imgRef.current = null;
      return;
    }
    const img = new Image();
    img.onload = () => {
      imgRef.current = img;
      fit();
      draw();
    };
    img.src = preview.dataUrl;
  }, [preview]);

  // 选中图层定位：非 solo 且有选中层时异步获取 bbox（命中模块级缓存则直接复用）
  useEffect(() => {
    const layer = selectedLayer;
    if (solo || !layer) {
      setHighlight(null);
      return;
    }
    const key = bboxKey(layer);
    const cached = bboxCache.get(key);
    if (cached !== undefined) {
      setHighlight({ key, bbox: cached });
      return;
    }
    let cancelled = false;
    getLayerBbox(layer)
      .then((bbox) => {
        bboxCache.set(key, bbox);
        if (!cancelled) setHighlight({ key, bbox });
      })
      .catch(() => {
        if (!cancelled) setHighlight(null);
      });
    return () => {
      cancelled = true;
    };
  }, [selectedLayer?.id, selectedLayer?.rotate, selectedLayer?.flipH, selectedLayer?.flipV, solo]);

  function fit() {
    const wrap = wrapRef.current;
    if (!wrap || !imgRef.current) return;
    const img = imgRef.current;
    const z = Math.min(wrap.clientWidth / img.width, wrap.clientHeight / img.height) * 0.95;
    setZoom(z);
    setOffset({ x: (wrap.clientWidth - img.width * z) / 2, y: (wrap.clientHeight - img.height * z) / 2 });
  }

  function draw() {
    const canvas = canvasRef.current;
    const wrap = wrapRef.current;
    const img = imgRef.current;
    if (!canvas || !wrap || !img || !preview) return;
    canvas.width = wrap.clientWidth;
    canvas.height = wrap.clientHeight;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.imageSmoothingEnabled = true;
    ctx.drawImage(img, offset.x, offset.y, img.width * zoom, img.height * zoom);

    // 图层定位高亮框：solo 模式不绘制；bbox 命中且属于当前选中层才画
    if (!solo && selectedLayer && highlight && highlight.key === bboxKey(selectedLayer) && highlight.bbox) {
      const { x, y, w, h } = highlight.bbox;
      const rotateOdd = selectedLayer.rotate % 2 === 1;
      // 变换后层尺寸（全分辨率）；预览按长边≤512 等比缩小，源像素 → 预览像素的比例
      const tw = rotateOdd ? selectedLayer.height : selectedLayer.width;
      const th = rotateOdd ? selectedLayer.width : selectedLayer.height;
      const sx = preview.width / tw;
      const sy = preview.height / th;
      const rx = offset.x + x * sx * zoom;
      const ry = offset.y + y * sy * zoom;
      const rw = w * sx * zoom;
      const rh = h * sy * zoom;
      ctx.save();
      ctx.strokeStyle = accentColor();
      ctx.lineWidth = 1.5;
      ctx.setLineDash([6, 4]);
      ctx.strokeRect(rx, ry, rw, rh);
      ctx.restore();
    }
  }

  useEffect(() => {
    draw();
  }, [zoom, offset, preview, highlight, solo, selectedLayer]);

  useEffect(() => {
    const onResize = () => { if (imgRef.current) fit(); };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  function onWheel(e: React.WheelEvent) {
    const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
    setZoom((z) => Math.min(8, Math.max(0.02, z * factor)));
  }

  return (
    <div
      ref={wrapRef}
      className="canvas-wrap"
      style={{ cursor: dragRef.current ? "grabbing" : "grab" }}
      onWheel={onWheel}
      onMouseDown={(e) => {
        dragRef.current = { startX: e.clientX, startY: e.clientY, ox: offset.x, oy: offset.y };
      }}
      onMouseMove={(e) => {
        if (dragRef.current) {
          setOffset({
            x: dragRef.current.ox + (e.clientX - dragRef.current.startX),
            y: dragRef.current.oy + (e.clientY - dragRef.current.startY),
          });
        }
      }}
      onMouseUp={() => (dragRef.current = null)}
      onMouseLeave={() => (dragRef.current = null)}
      onClick={(e) => {
        if (solo && e.target === e.currentTarget) onExitSolo();
      }}
    >
      <canvas ref={canvasRef} id="preview" />
      {solo && soloName && <div className="solo-badge">仅查看：{soloName} · 点击空白处返回全部</div>}
      <div className="zoom-bar">
        <button className="btn icon" onClick={() => setZoom((z) => Math.max(0.02, z / 1.25))}>−</button>
        <span>{Math.round(zoom * 100)}%</span>
        <button className="btn icon" onClick={() => setZoom((z) => Math.min(8, z * 1.25))}>＋</button>
        <button className="btn icon" onClick={fit}>适应</button>
      </div>
    </div>
  );
}

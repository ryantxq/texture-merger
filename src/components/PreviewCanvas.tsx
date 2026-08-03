// src/components/PreviewCanvas.tsx
import { useEffect, useRef, useState } from "react";

type Props = {
  preview: { dataUrl: string; width: number; height: number } | null;
  solo: boolean;
  soloName?: string;
  onExitSolo: () => void;
};

export default function PreviewCanvas({ preview, solo, soloName, onExitSolo }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const [zoom, setZoom] = useState(1);
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const imgRef = useRef<HTMLImageElement | null>(null);
  const dragRef = useRef<{ startX: number; startY: number; ox: number; oy: number } | null>(null);

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
    if (!canvas || !wrap || !img) return;
    canvas.width = wrap.clientWidth;
    canvas.height = wrap.clientHeight;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.imageSmoothingEnabled = true;
    ctx.drawImage(img, offset.x, offset.y, img.width * zoom, img.height * zoom);
  }

  useEffect(() => {
    draw();
  }, [zoom, offset, preview]);

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

// src/components/PreviewCanvas.tsx
import { useEffect, useRef, useState } from "react";
import { getLayerMask } from "../api";
import type { LayerState, PreviewBg } from "../types";

type Props = {
  preview: { dataUrl: string; width: number; height: number } | null;
  solo: boolean;
  soloName?: string;
  selectedLayer: LayerState | null;
  onExitSolo: () => void;
  previewBg: PreviewBg;
  onPreviewBg: (bg: PreviewBg) => void;
  highlightColor: string;
  onHighlightColor: (c: string) => void;
};

/** 蒙版模块级缓存：键 = `${id}_${rotate}_${flipH}_${flipV}`，避免重复 IPC/解码 */
const maskCache = new Map<string, HTMLImageElement>();
/** 染色蒙版缓存：键 = `${id}_${rotate}_${flipH}_${flipV}_${highlightColor}`，颜色或蒙版变化时失效 */
const tintCache = new Map<string, HTMLCanvasElement>();

function maskKey(layer: LayerState): string {
  return `${layer.id}_${layer.rotate}_${layer.flipH}_${layer.flipV}`;
}

export default function PreviewCanvas({
  preview,
  solo,
  soloName,
  selectedLayer,
  onExitSolo,
  previewBg,
  onPreviewBg,
  highlightColor,
  onHighlightColor,
}: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const [zoom, setZoom] = useState(1);
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const imgRef = useRef<HTMLImageElement | null>(null);
  const dragRef = useRef<{ startX: number; startY: number; ox: number; oy: number } | null>(null);
  // 选中图层蒙版图像（与 preview 同尺寸，multiply 叠加做颜色加深定位）；null 表示无蒙版
  const [maskImg, setMaskImg] = useState<HTMLImageElement | null>(null);
  const [showBgPicker, setShowBgPicker] = useState(false);

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

  // 选中图层蒙版：非 solo 且有选中层时异步获取（命中模块级缓存则直接复用）；否则清除蒙版状态
  useEffect(() => {
    const layer = selectedLayer;
    if (solo || !layer) {
      setMaskImg(null);
      return;
    }
    const key = maskKey(layer);
    const cached = maskCache.get(key);
    if (cached) {
      setMaskImg(cached);
      return;
    }
    let cancelled = false;
    getLayerMask(layer)
      .then((res) => {
        const img = new Image();
        img.onload = () => {
          maskCache.set(key, img);
          if (!cancelled) setMaskImg(img);
        };
        img.src = res.dataUrl;
      })
      .catch(() => {
        if (!cancelled) setMaskImg(null);
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

    // 选中层蒙版叠加：solo 模式不绘制；用高亮色染色蒙版半透明叠加（保留蒙版 alpha）
    if (!solo && maskImg && selectedLayer) {
      const key = maskKey(selectedLayer);
      const tintKey = `${key}_${highlightColor}`;
      let tinted = tintCache.get(tintKey);
      if (!tinted) {
        tinted = document.createElement("canvas");
        tinted.width = maskImg.width;
        tinted.height = maskImg.height;
        const tctx = tinted.getContext("2d");
        if (tctx) {
          tctx.drawImage(maskImg, 0, 0);
          tctx.globalCompositeOperation = "source-in";
          tctx.fillStyle = highlightColor;
          tctx.fillRect(0, 0, tinted.width, tinted.height);
        }
        tintCache.set(tintKey, tinted);
      }
      ctx.save();
      ctx.globalAlpha = 0.6;
      ctx.globalCompositeOperation = "source-over";
      ctx.drawImage(tinted, offset.x, offset.y, img.width * zoom, img.height * zoom);
      ctx.restore();
    }
  }

  useEffect(() => {
    draw();
  }, [zoom, offset, preview, maskImg, solo, selectedLayer, highlightColor]);

  useEffect(() => {
    const onResize = () => { if (imgRef.current) fit(); };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  function onWheel(e: React.WheelEvent) {
    const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
    setZoom((z) => Math.min(8, Math.max(0.02, z * factor)));
  }

  function setBg(patch: Partial<PreviewBg>) {
    onPreviewBg({ ...previewBg, ...patch });
  }

  return (
    <div
      ref={wrapRef}
      className="canvas-wrap"
      style={{
        cursor: dragRef.current ? "grabbing" : "grab",
        ...(previewBg.mode === "checker"
          ? {
              backgroundImage: `repeating-conic-gradient(${previewBg.checkerA} 0% 25%, ${previewBg.checkerB} 0% 50%)`,
              backgroundSize: "16px 16px",
            }
          : { backgroundColor: previewBg.solid }),
      }}
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
      {solo && soloName && (
        <button
          className="solo-badge"
          title="退出仅查看"
          onClick={(e) => {
            e.stopPropagation();
            onExitSolo();
          }}
        >
          仅查看：{soloName} <span>✕</span>
        </button>
      )}
      <div className="zoom-bar">
        <button
          className="btn icon"
          title="画布设置"
          onClick={(e) => {
            e.stopPropagation();
            setShowBgPicker((s) => !s);
          }}
        >
          画布设置
        </button>
        {showBgPicker && (
          <div className="bg-picker" onClick={(e) => e.stopPropagation()}>
            <div className="bg-picker-title">画布设置</div>
            <label className="bg-picker-row">
              <input
                type="radio"
                checked={previewBg.mode === "checker"}
                onChange={() => setBg({ mode: "checker" })}
              />
              棋盘格
            </label>
            <label className="bg-picker-row">
              <input
                type="radio"
                checked={previewBg.mode === "solid"}
                onChange={() => setBg({ mode: "solid" })}
              />
              单色
            </label>
            {previewBg.mode === "checker" ? (
              <>
                <label className="bg-picker-row">
                  A
                  <input
                    type="color"
                    value={previewBg.checkerA}
                    onChange={(e) => setBg({ checkerA: e.target.value })}
                  />
                </label>
                <label className="bg-picker-row">
                  B
                  <input
                    type="color"
                    value={previewBg.checkerB}
                    onChange={(e) => setBg({ checkerB: e.target.value })}
                  />
                </label>
              </>
            ) : (
              <label className="bg-picker-row">
                颜色
                <input
                  type="color"
                  value={previewBg.solid}
                  onChange={(e) => setBg({ solid: e.target.value })}
                />
              </label>
            )}
            <label className="bg-picker-row">
              高亮颜色
              <input
                type="color"
                value={highlightColor}
                onChange={(e) => onHighlightColor(e.target.value)}
              />
            </label>
          </div>
        )}
        <button className="btn icon" onClick={() => setZoom((z) => Math.max(0.02, z / 1.25))}>−</button>
        <span>{Math.round(zoom * 100)}%</span>
        <button className="btn icon" onClick={() => setZoom((z) => Math.min(8, z * 1.25))}>＋</button>
        <button className="btn icon" onClick={fit}>适应</button>
      </div>
    </div>
  );
}

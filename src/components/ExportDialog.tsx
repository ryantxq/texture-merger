// src/components/ExportDialog.tsx
import { useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { cancelExport, exportImage, revealInFolder } from "../api";
import type { AppState, Snapshot } from "../types";

type Props = { state: AppState; snapshot: Snapshot; onClose: () => void };

export default function ExportDialog({ state, snapshot, onClose }: Props) {
  const [phase, setPhase] = useState<"idle" | "running" | "done" | "error">("idle");
  const [info, setInfo] = useState("");
  const [savedPath, setSavedPath] = useState("");
  const [errorMsg, setErrorMsg] = useState("");

  async function start() {
    const path = await save({
      defaultPath: "merged.png",
      filters: [{ name: "PNG", extensions: ["png"] }],
    });
    if (!path) return;
    setSavedPath(path);
    setPhase("running");
    setInfo("");
    try {
      const stats = await exportImage(snapshot, state.exportOptions, path);
      setInfo(`导出完成：${stats.width}×${stats.height}，${(stats.bytesWritten / 1024 / 1024).toFixed(1)}MB，耗时 ${(stats.durationMs / 1000).toFixed(1)}s`);
      setPhase("done");
    } catch (e) {
      setErrorMsg(String(e));
      setPhase("error");
    }
  }

  async function doCancel() {
    await cancelExport();
    setPhase("idle");
    setInfo("已取消导出");
  }

  return (
    <div className="modal-mask">
      <div className="modal">
        <div className="modal-head">
          导出 PNG
          <button className="btn icon" onClick={onClose}>✕</button>
        </div>
        <div className="modal-body">
          <div>输出尺寸：{state.layers[0] ? `${state.layers[0].width}×${state.layers[0].height}` : "-"}（=单张尺寸）</div>
          <div>位深：{state.exportOptions.bitDepth}bit · 压缩：{state.exportOptions.compression}</div>
          <div style={{ marginTop: 8 }}>
            {phase === "running" && state.exportProgress != null && (
              <div>
                进度 {Math.round(state.exportProgress * 100)}%
                <div style={{ height: 8, background: "var(--border-2)", borderRadius: 4, marginTop: 4 }}>
                  <div style={{ height: 8, width: `${state.exportProgress * 100}%`, background: "var(--accent)", borderRadius: 4 }} />
                </div>
              </div>
            )}
            {info && <div style={{ marginTop: 6 }}>{info}</div>}
            {phase === "error" && <div style={{ marginTop: 6, color: "#d64545" }}>导出失败：{errorMsg}</div>}
          </div>
        </div>
        <div className="modal-foot">
          {phase === "done" ? (
            <>
              <button className="btn" onClick={() => revealInFolder(savedPath)}>打开所在文件夹</button>
              <button className="btn primary" onClick={onClose}>完成</button>
            </>
          ) : phase === "running" ? (
            <button className="btn" onClick={doCancel}>取消导出</button>
          ) : (
            <>
              <button className="btn" onClick={onClose}>关闭</button>
              <button className="btn primary" onClick={start}>开始导出</button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

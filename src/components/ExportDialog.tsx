// src/components/ExportDialog.tsx
import { useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { cancelExport, exportImage, revealInFolder } from "../api";
import type { AppState, ExportFile, Snapshot } from "../types";

type Props = { state: AppState; snapshot: Snapshot; onClose: () => void };

/** 把 save 对话框返回的完整路径拆成目录与去扩展名的文件名 */
function splitPath(path: string): { dir: string; baseStem: string } {
  const idx = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  if (idx < 0) return { dir: path, baseStem: path };
  return { dir: path.slice(0, idx), baseStem: path.slice(idx + 1).replace(/\.png$/i, "") };
}

function joinPath(dir: string, file: string): string {
  return dir.replace(/[\\/]+$/, "") + (dir.includes("/") ? "/" : "\\") + file;
}

export default function ExportDialog({ state, snapshot, onClose }: Props) {
  const first = state.layers[0];
  // 默认 3 级：原尺寸长边、1/2、1/4（去重且 ≥1）
  const longEdge = first ? Math.max(first.width, first.height) : 0;
  const levels = Array.from(new Set([longEdge, Math.floor(longEdge / 2), Math.floor(longEdge / 4)])).filter((s) => s >= 1);

  const [checked, setChecked] = useState<Record<number, boolean>>(() =>
    Object.fromEntries(levels.map((s) => [s, true]))
  );
  const [phase, setPhase] = useState<"idle" | "running" | "done" | "error">("idle");
  const [info, setInfo] = useState("");
  const [errorMsg, setErrorMsg] = useState("");
  const [savedDir, setSavedDir] = useState("");
  const [savedBaseStem, setSavedBaseStem] = useState("");
  const [files, setFiles] = useState<ExportFile[]>([]);

  const canExport = levels.some((s) => checked[s]);

  async function start() {
    const path = await save({
      defaultPath: "atlas.png",
      filters: [{ name: "PNG", extensions: ["png"] }],
    });
    if (!path) return;
    const { dir, baseStem } = splitPath(path);
    const sizes = levels.filter((s) => checked[s]);
    if (sizes.length === 0) return;
    setSavedDir(dir);
    setSavedBaseStem(baseStem);
    setPhase("running");
    setInfo("");
    try {
      const results = await exportImage(snapshot, { ...state.exportOptions, sizes }, dir, baseStem);
      setFiles(results);
      const lines = results.map((f) => `${f.size}px: ${(f.bytesWritten / 1024 / 1024).toFixed(1)}MB`);
      setInfo(`导出完成（${results.length} 个分辨率）\n${lines.join("\n")}`);
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

  function revealFirst() {
    if (files.length > 0) {
      const outLongEdge = Math.max(files[0].width, files[0].height);
      revealInFolder(joinPath(savedDir, `${savedBaseStem}_${outLongEdge}.png`));
    } else if (savedDir) {
      revealInFolder(savedDir);
    }
  }

  return (
    <div className="modal-mask">
      <div className="modal">
        <div className="modal-head">
          导出 PNG
          <button className="btn icon" onClick={onClose}>✕</button>
        </div>
        <div className="modal-body">
          <div>输出尺寸：{first ? `${first.width}×${first.height}` : "-"}（=单张尺寸）</div>
          <div>位深：{state.exportOptions.bitDepth}bit · 压缩：{state.exportOptions.compression}</div>
          {levels.length > 0 && (
            <div style={{ marginTop: 8 }}>
              <div style={{ marginBottom: 4, color: "var(--text-2)" }}>导出分辨率（长边）：</div>
              {levels.map((s) => (
                <label key={s} style={{ display: "flex", alignItems: "center", gap: 6, padding: "2px 0", cursor: "pointer" }}>
                  <input
                    type="checkbox"
                    checked={!!checked[s]}
                    onChange={(e) => setChecked((c) => ({ ...c, [s]: e.target.checked }))}
                  />
                  {s}px
                </label>
              ))}
            </div>
          )}
          <div style={{ marginTop: 8 }}>
            {phase === "running" && state.exportProgress != null && (
              <div>
                进度 {Math.round(state.exportProgress * 100)}%
                <div style={{ height: 8, background: "var(--border-2)", borderRadius: 4, marginTop: 4 }}>
                  <div style={{ height: 8, width: `${state.exportProgress * 100}%`, background: "var(--accent)", borderRadius: 4 }} />
                </div>
              </div>
            )}
            {info && <div style={{ marginTop: 6, whiteSpace: "pre-line" }}>{info}</div>}
            {phase === "error" && <div style={{ marginTop: 6, color: "#d64545" }}>导出失败：{errorMsg}</div>}
          </div>
        </div>
        <div className="modal-foot">
          {phase === "done" ? (
            <>
              <button className="btn" onClick={revealFirst}>打开所在文件夹</button>
              <button className="btn primary" onClick={onClose}>完成</button>
            </>
          ) : phase === "running" ? (
            <button className="btn" onClick={doCancel}>取消导出</button>
          ) : (
            <>
              <button className="btn" onClick={onClose}>关闭</button>
              <button className="btn primary" onClick={start} disabled={!canExport}>开始导出</button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

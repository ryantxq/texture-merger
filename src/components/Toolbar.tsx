// src/components/Toolbar.tsx
import { open } from "@tauri-apps/plugin-dialog";
import type { ExportOptions } from "../types";

type Props = {
  onImportFiles: (paths: string[]) => void;
  onImportFolder: (path: string) => void;
  onExport: () => void;
  onAbout: () => void;
  theme: "light" | "dark";
  onToggleTheme: () => void;
  exportOptions: ExportOptions;
  onExportOptions: (o: ExportOptions) => void;
};

export default function Toolbar(props: Props) {
  const { exportOptions } = props;

  async function pickFiles() {
    const selected = await open({ multiple: true, filters: [{ name: "PNG 贴图", extensions: ["png"] }] });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    props.onImportFiles(paths);
  }

  async function pickFolder() {
    const dir = await open({ directory: true });
    if (!dir) return;
    props.onImportFolder(dir);
  }

  return (
    <div className="toolbar">
      <button className="btn primary" onClick={pickFiles}>＋ 导入</button>
      <button className="btn" onClick={pickFolder}>选择文件夹</button>
      <span style={{ width: 1, height: 18, background: "var(--border)" }} />
      <label style={{ display: "flex", alignItems: "center", gap: 4, color: "var(--text-2)" }}>
        位深
        <select
          className="select"
          value={String(exportOptions.bitDepth)}
          onChange={(e) => props.onExportOptions({ ...exportOptions, bitDepth: Number(e.target.value) as 8 | 16 })}
        >
          <option value="8">8bit</option>
          <option value="16">16bit</option>
        </select>
      </label>
      <label style={{ display: "flex", alignItems: "center", gap: 4, color: "var(--text-2)" }}>
        压缩
        <select
          className="select"
          value={exportOptions.compression}
          onChange={(e) =>
            props.onExportOptions({ ...exportOptions, compression: e.target.value as ExportOptions["compression"] })
          }
        >
          <option value="fast">快速</option>
          <option value="balanced">均衡</option>
          <option value="best">最大</option>
        </select>
      </label>
      <button className="btn" onClick={props.onAbout}>ⓘ 关于</button>
      <span style={{ flex: 1 }} />
      <button className="btn icon" title="切换亮/暗主题" onClick={props.onToggleTheme}>
        {props.theme === "light" ? "☾" : "☀"}
      </button>
      <button className="btn primary" onClick={props.onExport}>导出 PNG</button>
    </div>
  );
}

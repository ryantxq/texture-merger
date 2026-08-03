// src/components/StatusBar.tsx
type Props = {
  layerCount: number;
  outputSize: { width: number; height: number } | null;
  previewSize: { width: number; height: number } | null;
  status: string;
  exportProgress: number | null;
  importProgress: { done: number; total: number } | null;
  bitDepth: 8 | 16;
};

export default function StatusBar({ layerCount, outputSize, previewSize, status, exportProgress, importProgress, bitDepth }: Props) {
  const memMB = outputSize
    ? Math.round(((outputSize.width * outputSize.height * 4 * (bitDepth / 8)) / 1024 / 1024) * 10) / 10
    : 0;
  return (
    <div className="statusbar">
      <span>
        {importProgress != null && (
          <span style={{ marginRight: 10, color: "var(--accent)" }}>导入中 {importProgress.done}/{importProgress.total}</span>
        )}
        图层 {layerCount}
        {outputSize && <span> · 输出 {outputSize.width}×{outputSize.height} · 预估 {memMB}MB</span>}
        {previewSize && <span> · 预览 {previewSize.width}×{previewSize.height}</span>}
      </span>
      <span>
        {exportProgress != null && <span style={{ marginRight: 10, color: "var(--accent)" }}>导出 {Math.round(exportProgress * 100)}%</span>}
        {status}
      </span>
    </div>
  );
}

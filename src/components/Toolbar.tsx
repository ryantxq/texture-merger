// src/components/Toolbar.tsx（占位，Task 9 实现）
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
  void props;
  return null;
}

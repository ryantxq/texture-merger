// src/components/LayerList.tsx（占位，Task 10 实现）
import type { ImportItem, LayerState } from "../types";

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
  void props;
  return null;
}

interface ToolbarProps {
  onAddFiles: () => void;
  onRemoveSelected: () => void;
  onStartCompression: () => void;
  selectedCount: number;
  disabled?: boolean;
}

export default function Toolbar({
  onAddFiles,
  onRemoveSelected,
  onStartCompression,
  selectedCount,
  disabled
}: ToolbarProps) {
  return (
    <div className="toolbar">
      <button onClick={onAddFiles} disabled={disabled}>
        添加文件
      </button>
      <button onClick={onRemoveSelected} disabled={disabled || selectedCount === 0}>
        移除选中
      </button>
      <button className="primary" onClick={onStartCompression} disabled={disabled}>
        开始压缩
      </button>
    </div>
  );
}

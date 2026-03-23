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
      <button className="toolbar-btn" onClick={onAddFiles} disabled={disabled}>
        添加文件
      </button>
      <button className="toolbar-btn" onClick={onRemoveSelected} disabled={disabled || selectedCount === 0}>
        移除选中 {selectedCount > 0 ? `(${selectedCount})` : ''}
      </button>
      <button className="toolbar-btn primary" onClick={onStartCompression} disabled={disabled}>
        {disabled ? '压缩中…' : '开始压缩'}
      </button>
    </div>
  );
}

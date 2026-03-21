import type { CompressionMode } from '../types';

interface ModeSelectorProps {
  mode: CompressionMode;
  onChange: (mode: CompressionMode) => void;
  descriptions: Record<CompressionMode, string>;
}

const options: Array<{ value: CompressionMode; label: string; short: string }> = [
  { value: 0, label: '无损', short: '仅结构优化' },
  { value: 1, label: '轻度', short: '图片轻压缩' },
  { value: 2, label: '标准', short: '删除冗余内容' },
  { value: 3, label: '极限', short: '最大压缩率' }
];

export default function ModeSelector({ mode, onChange }: ModeSelectorProps) {
  return (
    <div className="mode-selector">
      {options.map((opt) => (
        <button
          key={opt.value}
          className={`mode-btn${mode === opt.value ? ' active' : ''}`}
          type="button"
          onClick={() => onChange(opt.value)}
        >
          <strong>{opt.label}</strong>
          <span>{opt.short}</span>
        </button>
      ))}
    </div>
  );
}

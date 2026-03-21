import type { CompressionMode } from '../types';

interface ModeSelectorProps {
  mode: CompressionMode;
  onChange: (mode: CompressionMode) => void;
  descriptions: Record<CompressionMode, string>;
}

const options: Array<{ value: CompressionMode; label: string; short: string }> = [
  { value: 0, label: '无损', short: '仅优化结构' },
  { value: 1, label: '轻度', short: '200 DPI' },
  { value: 2, label: '标准', short: '150 DPI' },
  { value: 3, label: '极限', short: '100 DPI' }
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

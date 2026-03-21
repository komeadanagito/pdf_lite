import type { CompressionMode } from '../types';

interface ModeSelectorProps {
  mode: CompressionMode;
  onChange: (mode: CompressionMode) => void;
  descriptions: Record<CompressionMode, string>;
}

const options: Array<{ value: CompressionMode; label: string }> = [
  { value: 0, label: '无损' },
  { value: 1, label: '轻度' },
  { value: 2, label: '标准' },
  { value: 3, label: '极限' }
];

export default function ModeSelector({ mode, onChange, descriptions }: ModeSelectorProps) {
  return (
    <div className="mode-selector">
      {options.map((option) => (
        <button
          key={option.value}
          className={`mode-button ${mode === option.value ? 'active' : ''}`}
          type="button"
          onClick={() => onChange(option.value)}
        >
          <strong>{option.label}</strong>
          <span>{descriptions[option.value]}</span>
        </button>
      ))}
    </div>
  );
}

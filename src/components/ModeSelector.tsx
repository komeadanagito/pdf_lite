import type { CompressionMode } from '../types';

interface ModeSelectorProps {
  mode: CompressionMode;
  onChange: (mode: CompressionMode) => void;
  options: Array<{ value: CompressionMode; label: string; short: string }>;
}

export default function ModeSelector({ mode, onChange, options }: ModeSelectorProps) {
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

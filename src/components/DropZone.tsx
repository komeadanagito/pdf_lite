import type { DragEvent, ReactNode } from 'react';

interface DropZoneProps {
  active: boolean;
  onDragOver: () => void;
  onDragLeave: () => void;
  onDrop: (event: DragEvent<HTMLDivElement>) => void;
  children: ReactNode;
}

export default function DropZone({ active, onDragOver, onDragLeave, onDrop, children }: DropZoneProps) {
  return (
    <div
      className="drop-zone"
      onDragOver={(e) => { e.preventDefault(); onDragOver(); }}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
    >
      {children}
      {active && (
        <div className="drop-overlay">
          <div className="drop-overlay-content">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              <polyline points="7 10 12 15 17 10" />
              <line x1="12" y1="15" x2="12" y2="3" />
            </svg>
            <span>松开以添加 PDF</span>
          </div>
        </div>
      )}
    </div>
  );
}

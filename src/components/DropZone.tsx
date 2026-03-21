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
      {active && <div className="drop-overlay">松开鼠标以添加 PDF</div>}
    </div>
  );
}

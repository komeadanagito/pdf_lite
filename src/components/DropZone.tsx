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
      onDragOver={(event) => {
        event.preventDefault();
        onDragOver();
      }}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
    >
      {children}
      {active ? <div className="drop-overlay">释放以添加 PDF 文件</div> : null}
    </div>
  );
}

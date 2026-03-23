import type { FileItem } from '../types';
import { formatBytes } from '../types';

export type ColumnId = 'name' | 'pages' | 'size' | 'compressed' | 'ratio' | 'status';

interface FileTableProps {
  files: FileItem[];
  columns: ColumnId[];
  onToggleItem: (id: string) => void;
  onToggleAll: (selected: boolean) => void;
  emptyMessage?: string;
  emptyHint?: string;
}

const STATUS_LABEL: Record<FileItem['status'], string> = {
  ready: '待处理',
  queued: '排队中',
  compressing: '压缩中',
  done: '已完成',
  error: '失败',
};

const COL_CONFIG: Record<ColumnId, { label: string; width?: number }> = {
  name: { label: '文件名' },
  pages: { label: '页数', width: 64 },
  size: { label: '原始大小', width: 88 },
  compressed: { label: '压缩后', width: 88 },
  ratio: { label: '节省', width: 76 },
  status: { label: '状态', width: 80 },
};

export default function FileTable({ files, columns, onToggleItem, onToggleAll, emptyMessage, emptyHint }: FileTableProps) {
  const allSelected = files.length > 0 && files.every((f) => f.selected);

  if (files.length === 0) {
    return (
      <div className="empty-state">
        <div className="empty-state-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
            <polyline points="14 2 14 8 20 8" />
            <line x1="12" y1="18" x2="12" y2="12" />
            <line x1="9" y1="15" x2="15" y2="15" />
          </svg>
        </div>
        <p>{emptyMessage ?? '拖入文件，或点击上方「添加文件」'}</p>
        {emptyHint && <span className="empty-hint">{emptyHint}</span>}
      </div>
    );
  }

  return (
    <table className="file-table">
      <thead>
        <tr>
          <th style={{ width: 44 }}>
            <input type="checkbox" checked={allSelected} onChange={(e) => onToggleAll(e.target.checked)} />
          </th>
          {columns.map((col) => (
            <th key={col} style={COL_CONFIG[col].width ? { width: COL_CONFIG[col].width } : undefined}>
              {COL_CONFIG[col].label}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {files.map((file) => {
          const ratio = file.compressedSize ? (1 - file.compressedSize / file.size) * 100 : undefined;
          return (
            <tr key={file.id} className={file.selected ? 'row-selected' : ''}>
              <td>
                <input type="checkbox" checked={file.selected} onChange={() => onToggleItem(file.id)} />
              </td>
              {columns.map((col) => {
                switch (col) {
                  case 'name':
                    return (
                      <td key={col}>
                        <div className="file-name">{file.name}</div>
                        <div className="file-path">{file.path}</div>
                        {file.error ? <div className="row-error-msg">{file.error}</div> : null}
                      </td>
                    );
                  case 'pages':
                    return <td key={col}>{file.pages ?? '-'}</td>;
                  case 'size':
                    return <td key={col}>{formatBytes(file.size)}</td>;
                  case 'compressed':
                    return <td key={col}>{file.compressedSize ? formatBytes(file.compressedSize) : '-'}</td>;
                  case 'ratio':
                    return (
                      <td key={col}>
                        {ratio !== undefined ? (
                          <span style={{ color: ratio >= 0 ? 'var(--green)' : 'var(--red)', fontWeight: 600 }}>
                            {ratio >= 0 ? `↓ ${ratio.toFixed(1)}%` : `↑ ${Math.abs(ratio).toFixed(1)}%`}
                          </span>
                        ) : '-'}
                      </td>
                    );
                  case 'status':
                    return (
                      <td key={col}>
                        <span className={`status-pill ${file.status}`}>{STATUS_LABEL[file.status]}</span>
                      </td>
                    );
                  default:
                    return null;
                }
              })}
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

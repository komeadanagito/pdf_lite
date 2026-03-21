import type { PdfFileItem } from '../types';

interface FileTableProps {
  files: PdfFileItem[];
  onToggleItem: (id: string) => void;
  onToggleAll: (selected: boolean) => void;
}

function formatBytes(bytes: number): string {
  const units = ['B', 'KB', 'MB', 'GB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value >= 100 || unit === 0 ? 0 : 1)} ${units[unit]}`;
}

const STATUS_LABEL: Record<PdfFileItem['status'], string> = {
  ready: '待处理',
  queued: '排队中',
  compressing: '压缩中',
  done: '已完成',
  error: '失败'
};

export default function FileTable({ files, onToggleItem, onToggleAll }: FileTableProps) {
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
        <p>拖入 PDF 文件，或点击"添加文件"</p>
      </div>
    );
  }

  return (
    <table className="file-table">
      <thead>
        <tr>
          <th style={{ width: 48 }}>
            <input type="checkbox" checked={allSelected} onChange={(e) => onToggleAll(e.target.checked)} />
          </th>
          <th>文件名</th>
          <th style={{ width: 100 }}>页数</th>
          <th style={{ width: 100 }}>原始大小</th>
          <th style={{ width: 100 }}>压缩后</th>
          <th style={{ width: 90 }}>节省</th>
          <th style={{ width: 90 }}>状态</th>
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
              <td>
                <div className="file-name">{file.name}</div>
                <div className="file-path">{file.path}</div>
                {file.error ? <div className="row-error-msg">{file.error}</div> : null}
              </td>
              <td>{file.pages ?? '-'}</td>
              <td>{formatBytes(file.size)}</td>
              <td>{file.compressedSize ? formatBytes(file.compressedSize) : '-'}</td>
              <td>
                {ratio !== undefined
                  ? <span style={{ color: ratio >= 0 ? 'var(--green)' : 'var(--red)', fontWeight: 500 }}>
                      {ratio >= 0 ? `↓ ${ratio.toFixed(1)}%` : `↑ ${Math.abs(ratio).toFixed(1)}%`}
                    </span>
                  : '-'}
              </td>
              <td>
                <span className={`status-pill ${file.status}`}>{STATUS_LABEL[file.status]}</span>
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

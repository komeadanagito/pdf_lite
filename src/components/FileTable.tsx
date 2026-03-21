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

function statusLabel(status: PdfFileItem['status']): string {
  switch (status) {
    case 'ready':
      return '待处理';
    case 'queued':
      return '排队中';
    case 'compressing':
      return '压缩中';
    case 'done':
      return '已完成';
    case 'error':
      return '失败';
    default:
      return status;
  }
}

export default function FileTable({ files, onToggleItem, onToggleAll }: FileTableProps) {
  const allSelected = files.length > 0 && files.every((file) => file.selected);

  return (
    <table className="file-table">
      <thead>
        <tr>
          <th style={{ width: 56 }}>
            <input type="checkbox" checked={allSelected} onChange={(event) => onToggleAll(event.target.checked)} />
          </th>
          <th>文件名</th>
          <th style={{ width: 120 }}>原始大小</th>
          <th style={{ width: 120 }}>压缩后大小</th>
          <th style={{ width: 100 }}>压缩率</th>
          <th style={{ width: 110 }}>状态</th>
        </tr>
      </thead>
      <tbody>
        {files.length === 0 ? (
          <tr>
            <td colSpan={6} style={{ textAlign: 'center', padding: '48px 16px', color: 'var(--muted)' }}>
              拖入 PDF 文件或点击“添加文件”开始
            </td>
          </tr>
        ) : (
          files.map((file) => {
            const ratio = file.compressedSize ? (1 - file.compressedSize / file.size) * 100 : undefined;
            return (
              <tr key={file.id} className={file.selected ? 'selected' : ''}>
                <td>
                  <input type="checkbox" checked={file.selected} onChange={() => onToggleItem(file.id)} />
                </td>
                <td>
                  <div className="file-name">{file.name}</div>
                  <div style={{ color: 'var(--muted)', fontSize: 12 }}>{file.path}</div>
                </td>
                <td>{formatBytes(file.size)}</td>
                <td>{file.compressedSize ? formatBytes(file.compressedSize) : '-'}</td>
                <td>{ratio !== undefined ? `${ratio >= 0 ? '+' : ''}${ratio.toFixed(1)}%` : '-'}</td>
                <td>
                  <span className={`status-badge ${file.status}`}>{statusLabel(file.status)}</span>
                  {file.error ? <div style={{ color: 'var(--danger)', fontSize: 12, marginTop: 6 }}>{file.error}</div> : null}
                </td>
              </tr>
            );
          })
        )}
      </tbody>
    </table>
  );
}

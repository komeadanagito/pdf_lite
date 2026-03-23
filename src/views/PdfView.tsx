import { forwardRef, useEffect, useImperativeHandle, useMemo, useReducer, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import Toolbar from '../components/Toolbar';
import FileTable from '../components/FileTable';
import ModeSelector from '../components/ModeSelector';
import type { CompressResult, CompressionMode, FileItem, PdfInfo } from '../types';
import { formatBytes, formatRatio } from '../types';

type Action =
  | { type: 'add'; files: FileItem[] }
  | { type: 'remove_selected' }
  | { type: 'toggle'; id: string }
  | { type: 'toggle_all'; selected: boolean }
  | { type: 'set_status'; id: string; status: FileItem['status']; error?: string }
  | { type: 'apply_result'; id: string; result: CompressResult };

function reducer(state: FileItem[], action: Action): FileItem[] {
  switch (action.type) {
    case 'add':
      return [...state, ...action.files];
    case 'remove_selected':
      return state.filter((f) => !f.selected);
    case 'toggle':
      return state.map((f) => (f.id === action.id ? { ...f, selected: !f.selected } : f));
    case 'toggle_all':
      return state.map((f) => ({ ...f, selected: action.selected }));
    case 'set_status':
      return state.map((f) => (f.id === action.id ? { ...f, status: action.status, error: action.error } : f));
    case 'apply_result':
      return state.map((f) =>
        f.id === action.id
          ? { ...f, status: 'done', compressedSize: action.result.compressed_size, outputPath: action.result.output_path, error: undefined }
          : f
      );
    default:
      return state;
  }
}

const modeDescriptions: Record<CompressionMode, string> = {
  0: '无损：保持原始画质，仅优化结构',
  1: '轻度：200 DPI，高清打印质量',
  2: '标准：150 DPI，屏幕阅读最佳',
  3: '极限：100 DPI，最大压缩率',
};

const modeOptions: Array<{ value: CompressionMode; label: string; short: string }> = [
  { value: 0, label: '无损', short: '仅优化结构' },
  { value: 1, label: '轻度', short: '200 DPI' },
  { value: 2, label: '标准', short: '150 DPI' },
  { value: 3, label: '极限', short: '100 DPI' },
];

export interface PdfViewHandle {
  addFiles: (paths: string[]) => void;
}

interface PdfViewProps {
  onFileCountChange: (count: number) => void;
}

const PdfView = forwardRef<PdfViewHandle, PdfViewProps>(function PdfView({ onFileCountChange }, ref) {
  const [files, dispatch] = useReducer(reducer, []);
  const [mode, setMode] = useState<CompressionMode>(1);
  const [busy, setBusy] = useState(false);

  const selectedCount = useMemo(() => files.filter((f) => f.selected).length, [files]);
  const totals = useMemo(() => {
    const original = files.reduce((s, f) => s + f.size, 0);
    const compressed = files.reduce((s, f) => s + (f.compressedSize ?? 0), 0);
    return { original, compressed };
  }, [files]);

  useEffect(() => {
    onFileCountChange(files.length);
  }, [files.length, onFileCountChange]);

  async function loadAndAddFiles(paths: string[]) {
    const pdfs = paths.filter((p) => p.toLowerCase().endsWith('.pdf'));
    if (pdfs.length === 0) return;
    const next = await Promise.all(
      pdfs.map(async (path) => {
        const info = await invoke<PdfInfo>('get_pdf_info', { path });
        return {
          id: `${path}-${Date.now()}-${Math.random().toString(16).slice(2)}`,
          path,
          name: info.file_name,
          size: info.size_bytes,
          pages: info.pages,
          category: 'pdf' as const,
          status: 'ready' as const,
          selected: false,
        };
      })
    );
    dispatch({ type: 'add', files: next });
  }

  async function openAndAdd() {
    const chosen = await open({ multiple: true, filters: [{ name: 'PDF', extensions: ['pdf'] }] });
    if (!chosen) return;
    const list = Array.isArray(chosen) ? chosen : [chosen];
    await loadAndAddFiles(list);
  }

  useImperativeHandle(ref, () => ({
    addFiles: (paths: string[]) => void loadAndAddFiles(paths),
  }));

  async function compressAll() {
    if (busy || files.length === 0) return;
    setBusy(true);
    try {
      for (const item of files.filter((f) => f.status !== 'compressing')) {
        dispatch({ type: 'set_status', id: item.id, status: 'compressing' });
        try {
          const result = await invoke<CompressResult>('compress_pdf', { path: item.path, mode });
          dispatch({ type: 'apply_result', id: item.id, result });
        } catch (error) {
          dispatch({ type: 'set_status', id: item.id, status: 'error', error: error instanceof Error ? error.message : String(error) });
        }
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <Toolbar
        onAddFiles={() => void openAndAdd()}
        onRemoveSelected={() => dispatch({ type: 'remove_selected' })}
        onStartCompression={() => void compressAll()}
        selectedCount={selectedCount}
        disabled={busy}
      />
      <section className="workspace">
        <FileTable
          files={files}
          columns={['name', 'pages', 'size', 'compressed', 'ratio', 'status']}
          onToggleItem={(id) => dispatch({ type: 'toggle', id })}
          onToggleAll={(selected) => dispatch({ type: 'toggle_all', selected })}
          emptyMessage="拖入 PDF 文件，或点击上方「添加文件」"
          emptyHint="支持批量添加与压缩"
        />
      </section>
      <footer className="bottom-bar">
        <ModeSelector mode={mode} onChange={setMode} options={modeOptions} />
        <div className="summary">
          <div className="summary-item">
            <div className="s-label">原始大小</div>
            <div className="s-value">{formatBytes(totals.original)}</div>
          </div>
          <div className="summary-item">
            <div className="s-label">压缩后</div>
            <div className="s-value">{formatBytes(totals.compressed)}</div>
          </div>
          <div className="summary-item">
            <div className="s-label">节省</div>
            <div className="s-value">{formatRatio(totals.original, totals.compressed)}</div>
          </div>
        </div>
      </footer>
      <p className="hint">当前模式：{modeDescriptions[mode]}</p>
    </>
  );
});

export default PdfView;

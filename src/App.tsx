import { useEffect, useMemo, useReducer, useState, type DragEvent } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { open } from '@tauri-apps/plugin-dialog';
import Toolbar from './components/Toolbar';
import FileTable from './components/FileTable';
import ModeSelector from './components/ModeSelector';
import DropZone from './components/DropZone';
import type { CompressResult, CompressionMode, CompressionProgress, PdfFileItem, PdfInfo } from './types';

type Action =
  | { type: 'add'; files: PdfFileItem[] }
  | { type: 'remove_selected' }
  | { type: 'toggle'; id: string }
  | { type: 'toggle_all'; selected: boolean }
  | { type: 'set_status'; id: string; status: PdfFileItem['status']; error?: string }
  | { type: 'apply_result'; id: string; result: CompressResult }
  | { type: 'clear_error'; id: string }
  | { type: 'reset_selection' };

function reducer(state: PdfFileItem[], action: Action): PdfFileItem[] {
  switch (action.type) {
    case 'add':
      return [...state, ...action.files];
    case 'remove_selected':
      return state.filter((item) => !item.selected);
    case 'toggle':
      return state.map((item) => (item.id === action.id ? { ...item, selected: !item.selected } : item));
    case 'toggle_all':
      return state.map((item) => ({ ...item, selected: action.selected }));
    case 'set_status':
      return state.map((item) =>
        item.id === action.id ? { ...item, status: action.status, error: action.error } : item
      );
    case 'apply_result':
      return state.map((item) =>
        item.id === action.id
          ? {
              ...item,
              status: 'done',
              compressedSize: action.result.compressed_size,
              outputPath: action.result.output_path,
              error: undefined
            }
          : item
      );
    case 'clear_error':
      return state.map((item) => (item.id === action.id ? { ...item, error: undefined } : item));
    case 'reset_selection':
      return state.map((item) => ({ ...item, selected: false }));
    default:
      return state;
  }
}

const modeDescriptions: Record<CompressionMode, string> = {
  0: '无损：结构优化与流重压缩',
  1: '轻度：无损 + JPEG 重压缩',
  2: '标准：轻度 + 删除书签/注释/表单',
  3: '极限：标准 + 去元数据 + 更激进图片压缩'
};

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes === 0) return '-';
  const units = ['B', 'KB', 'MB', 'GB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value >= 100 || unit === 0 ? 0 : 1)} ${units[unit]}`;
}

function formatRatio(original: number, compressed?: number): string {
  if (!compressed || original <= 0) return '-';
  const saved = (1 - compressed / original) * 100;
  return saved >= 0 ? `↓ ${saved.toFixed(1)}%` : `↑ ${Math.abs(saved).toFixed(1)}%`;
}

function createFileItem(path: string, info: PdfInfo): PdfFileItem {
  return {
    id: `${path}-${Date.now()}-${Math.random().toString(16).slice(2)}`,
    path,
    name: info.file_name,
    size: info.size_bytes,
    pages: info.pages,
    status: 'ready',
    selected: false
  };
}

export default function App() {
  const [files, dispatch] = useReducer(reducer, []);
  const [mode, setMode] = useState<CompressionMode>(1);
  const [dragActive, setDragActive] = useState(false);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<CompressionProgress | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<CompressionProgress>('compress-progress', (event) => {
      setProgress(event.payload);
    }).then((cleanup) => {
      unlisten = cleanup;
    });
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const payload = event.payload as { type: string; paths: string[] };
        if (payload.type === 'enter' || payload.type === 'over') setDragActive(true);
        if (payload.type !== 'enter' && payload.type !== 'over') setDragActive(false);
        if (payload.type === 'drop') {
          setDragActive(false);
          void addFiles(payload.paths.filter((p) => p.toLowerCase().endsWith('.pdf')));
        }
      })
      .then((cleanup) => { unlisten = cleanup; });
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const selectedCount = useMemo(() => files.filter((f) => f.selected).length, [files]);
  const totals = useMemo(() => {
    const original = files.reduce((s, f) => s + f.size, 0);
    const compressed = files.reduce((s, f) => s + (f.compressedSize ?? 0), 0);
    return { original, compressed };
  }, [files]);

  async function addFiles(paths?: string[]) {
    const chosen = paths ?? (await open({ multiple: true, filters: [{ name: 'PDF', extensions: ['pdf'] }] }));
    if (!chosen) return;
    const list = Array.isArray(chosen) ? chosen : [chosen];
    const pdfs = list.filter((item) => item.toLowerCase().endsWith('.pdf'));
    const next = await Promise.all(
      pdfs.map(async (path) => {
        const info = await invoke<PdfInfo>('get_pdf_info', { path });
        return createFileItem(path, info);
      })
    );
    dispatch({ type: 'add', files: next });
  }

  async function compressAll() {
    if (busy || files.length === 0) return;
    setBusy(true);
    try {
      const targets = files.filter((f) => f.status !== 'compressing');
      for (const item of targets) {
        dispatch({ type: 'set_status', id: item.id, status: 'compressing' });
        try {
          const result = await invoke<CompressResult>('compress_pdf', { path: item.path, mode });
          dispatch({ type: 'apply_result', id: item.id, result });
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error);
          dispatch({ type: 'set_status', id: item.id, status: 'error', error: message });
        }
      }
    } finally {
      setBusy(false);
      setProgress(null);
    }
  }

  function handleDrop(event: DragEvent<HTMLDivElement>) {
    event.preventDefault();
    setDragActive(false);
  }

  return (
    <DropZone active={dragActive} onDragOver={() => setDragActive(true)} onDragLeave={() => setDragActive(false)} onDrop={handleDrop}>
      <div className="app-shell">

        <header className="app-header">
          <div className="header-left">
            <div className="app-icon">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
                <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
                <polyline points="14 2 14 8 20 8" />
                <line x1="9" y1="15" x2="15" y2="15" />
              </svg>
            </div>
            <div>
              <div className="app-title">PDF Lite</div>
              <div className="app-subtitle">
                {progress
                  ? `${progress.stage}… ${progress.completed}/${progress.total}`
                  : '点击添加或拖入 PDF 文件'}
              </div>
            </div>
          </div>
          <div className="header-stats">
            <div className="stat-item">
              <div className="stat-value">{files.length}</div>
              <div className="stat-label">文件数</div>
            </div>
            <div className="stat-item">
              <div className="stat-value">{selectedCount}</div>
              <div className="stat-label">已选</div>
            </div>
          </div>
        </header>

        <Toolbar
          onAddFiles={() => void addFiles()}
          onRemoveSelected={() => dispatch({ type: 'remove_selected' })}
          onStartCompression={() => void compressAll()}
          selectedCount={selectedCount}
          disabled={busy}
        />

        <section className="workspace">
          <FileTable
            files={files}
            onToggleItem={(id) => dispatch({ type: 'toggle', id })}
            onToggleAll={(selected) => dispatch({ type: 'toggle_all', selected })}
          />
        </section>

        <footer className="bottom-bar">
          <ModeSelector mode={mode} onChange={setMode} descriptions={modeDescriptions} />
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

      </div>
    </DropZone>
  );
}

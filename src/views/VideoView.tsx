import { forwardRef, useEffect, useImperativeHandle, useMemo, useReducer, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import Toolbar from '../components/Toolbar';
import FileTable from '../components/FileTable';
import ModeSelector from '../components/ModeSelector';
import type { CompressResult, CompressionMode, FileItem } from '../types';
import { VIDEO_EXTENSIONS, formatBytes, formatRatio, getFileExtension } from '../types';

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
  0: '无损：仅重封装，不重编码',
  1: '轻度：CRF 23，接近原始画质',
  2: '标准：CRF 28 + 1080p，平衡体积',
  3: '极限：CRF 32 + 720p，最大压缩',
};

const modeOptions: Array<{ value: CompressionMode; label: string; short: string }> = [
  { value: 0, label: '无损', short: '重封装' },
  { value: 1, label: '轻度', short: 'CRF 23' },
  { value: 2, label: '标准', short: 'CRF 28' },
  { value: 3, label: '极限', short: 'CRF 32' },
];

export interface VideoViewHandle {
  addFiles: (paths: string[]) => void;
}

interface VideoViewProps {
  onFileCountChange: (count: number) => void;
}

const VideoView = forwardRef<VideoViewHandle, VideoViewProps>(function VideoView({ onFileCountChange }, ref) {
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
    const valid = paths.filter((p) => VIDEO_EXTENSIONS.includes(getFileExtension(p)));
    if (valid.length === 0) return;
    const next: FileItem[] = [];
    for (const path of valid) {
      const name = path.split(/[/\\]/).pop() ?? 'video';
      const stat = await invoke<{ size: number }>('get_file_size', { path }).catch(() => ({ size: 0 }));
      next.push({
        id: `${path}-${Date.now()}-${Math.random().toString(16).slice(2)}`,
        path,
        name,
        size: stat.size,
        category: 'video',
        status: 'ready',
        selected: false,
      });
    }
    dispatch({ type: 'add', files: next });
  }

  async function openAndAdd() {
    const chosen = await open({
      multiple: true,
      filters: [{ name: '视频', extensions: [...VIDEO_EXTENSIONS] }],
    });
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
          const result = await invoke<CompressResult>('compress_video', { path: item.path, mode });
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
          columns={['name', 'size', 'compressed', 'ratio', 'status']}
          onToggleItem={(id) => dispatch({ type: 'toggle', id })}
          onToggleAll={(selected) => dispatch({ type: 'toggle_all', selected })}
          emptyMessage="拖入视频文件，或点击上方「添加文件」"
          emptyHint="支持 MP4、MOV、AVI、MKV"
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

export default VideoView;

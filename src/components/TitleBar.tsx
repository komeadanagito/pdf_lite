import type { Window } from '@tauri-apps/api/window';

interface TitleBarProps {
  appWindow: Window;
  fileCount: number;
  selectedCount: number;
}

export default function TitleBar({ appWindow, fileCount, selectedCount }: TitleBarProps) {
  return (
    <div className="titlebar">
      <div
        className="titlebar-drag"
        onMouseDown={() => void appWindow.startDragging()}
        onDoubleClick={() => void appWindow.toggleMaximize()}
      />

      <div className="titlebar-left">
        <div className="traffic-lights">
          <button className="traffic-light close" onClick={() => void appWindow.close()} title="关闭">
            <svg viewBox="0 0 6 6" fill="none" stroke="#4c0000" strokeWidth="1.2">
              <line x1="0.5" y1="0.5" x2="5.5" y2="5.5" />
              <line x1="5.5" y1="0.5" x2="0.5" y2="5.5" />
            </svg>
          </button>
          <button className="traffic-light minimize" onClick={() => void appWindow.minimize()} title="最小化">
            <svg viewBox="0 0 6 6" fill="none" stroke="#995700" strokeWidth="1.2">
              <line x1="0.5" y1="3" x2="5.5" y2="3" />
            </svg>
          </button>
          <button className="traffic-light maximize" onClick={() => void appWindow.toggleMaximize()} title="最大化">
            <svg viewBox="0 0 6 6" fill="none" stroke="#006500" strokeWidth="1.2">
              <polyline points="1,4.5 1,1 4.5,1" />
              <polyline points="5,1.5 5,5 1.5,5" />
            </svg>
          </button>
        </div>
        <span className="titlebar-title">File Lite</span>
      </div>

      <div className="titlebar-right">
        <div className="titlebar-stats">
          <div className="titlebar-stat">
            <div className="titlebar-stat-value">{fileCount}</div>
            <div className="titlebar-stat-label">文件</div>
          </div>
          <div className="titlebar-stat">
            <div className="titlebar-stat-value">{selectedCount}</div>
            <div className="titlebar-stat-label">选中</div>
          </div>
        </div>
      </div>
    </div>
  );
}

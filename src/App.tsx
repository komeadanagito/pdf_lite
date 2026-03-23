import { useCallback, useEffect, useRef, useState, type DragEvent } from 'react';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { getCurrentWindow } from '@tauri-apps/api/window';
import TitleBar from './components/TitleBar';
import TabBar from './components/TabBar';
import DropZone from './components/DropZone';
import { MobileTabBar } from './layouts/MobileLayout';
import PdfView from './views/PdfView';
import ImageView from './views/ImageView';
import VideoView from './views/VideoView';
import type { FileCategory } from './types';
import { getFileCategory } from './types';

export default function App() {
  const [activeTab, setActiveTab] = useState<FileCategory>('pdf');
  const [dragActive, setDragActive] = useState(false);
  const [counts, setCounts] = useState<Record<FileCategory, number>>({ pdf: 0, image: 0, video: 0 });

  const pdfRef = useRef<{ addFiles: (paths: string[]) => void }>(null);
  const imageRef = useRef<{ addFiles: (paths: string[]) => void }>(null);
  const videoRef = useRef<{ addFiles: (paths: string[]) => void }>(null);

  const appWindow = getCurrentWindow();

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const payload = event.payload as { type: string; paths: string[] };
        if (payload.type === 'enter' || payload.type === 'over') setDragActive(true);
        if (payload.type !== 'enter' && payload.type !== 'over') setDragActive(false);
        if (payload.type === 'drop') {
          setDragActive(false);
          const byCategory: Record<FileCategory, string[]> = { pdf: [], image: [], video: [] };
          for (const path of payload.paths) {
            const cat = getFileCategory(path);
            if (cat) byCategory[cat].push(path);
          }
          for (const cat of ['pdf', 'image', 'video'] as FileCategory[]) {
            if (byCategory[cat].length > 0) {
              setActiveTab(cat);
              if (cat === 'pdf') pdfRef.current?.addFiles(byCategory[cat]);
              else if (cat === 'image') imageRef.current?.addFiles(byCategory[cat]);
              else if (cat === 'video') videoRef.current?.addFiles(byCategory[cat]);
            }
          }
        }
      })
      .then((cleanup) => { unlisten = cleanup; });
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  function handleDrop(event: DragEvent<HTMLDivElement>) {
    event.preventDefault();
    setDragActive(false);
  }

  const updateCount = useCallback((cat: FileCategory, n: number) => {
    setCounts((c) => ({ ...c, [cat]: n }));
  }, []);

  const totalFiles = counts.pdf + counts.image + counts.video;

  return (
    <DropZone active={dragActive} onDragOver={() => setDragActive(true)} onDragLeave={() => setDragActive(false)} onDrop={handleDrop}>
      <div className="app-shell">
        <TitleBar appWindow={appWindow} fileCount={totalFiles} selectedCount={0} />
        <TabBar activeTab={activeTab} onChange={setActiveTab} counts={counts} />

        <div className={`view-panel${activeTab === 'pdf' ? ' active' : ''}`}>
          <PdfView ref={pdfRef} onFileCountChange={(n) => updateCount('pdf', n)} />
        </div>
        <div className={`view-panel${activeTab === 'image' ? ' active' : ''}`}>
          <ImageView ref={imageRef} onFileCountChange={(n) => updateCount('image', n)} />
        </div>
        <div className={`view-panel${activeTab === 'video' ? ' active' : ''}`}>
          <VideoView ref={videoRef} onFileCountChange={(n) => updateCount('video', n)} />
        </div>

        <MobileTabBar activeTab={activeTab} onChange={setActiveTab} counts={counts} />
      </div>
    </DropZone>
  );
}

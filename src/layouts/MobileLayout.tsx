import type { FileCategory } from '../types';

interface Tab {
  id: FileCategory;
  label: string;
  icon: JSX.Element;
}

const tabs: Tab[] = [
  {
    id: 'pdf',
    label: 'PDF',
    icon: (
      <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
        <path d="M12 2H5a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V7z" />
        <polyline points="12 2 12 7 17 7" />
      </svg>
    ),
  },
  {
    id: 'image',
    label: '图片',
    icon: (
      <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
        <rect x="2" y="2" width="16" height="16" rx="2" />
        <circle cx="7.5" cy="7.5" r="1.5" />
        <polyline points="18 13 13 8 4 18" />
      </svg>
    ),
  },
  {
    id: 'video',
    label: '视频',
    icon: (
      <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
        <rect x="1" y="4" width="13" height="12" rx="2" />
        <polyline points="14 8 19 5 19 15 14 12" />
      </svg>
    ),
  },
];

interface MobileTabBarProps {
  activeTab: FileCategory;
  onChange: (tab: FileCategory) => void;
  counts: Record<FileCategory, number>;
}

export function MobileTabBar({ activeTab, onChange, counts }: MobileTabBarProps) {
  return (
    <nav className="mobile-tab-bar">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          className={`mobile-tab-item${activeTab === tab.id ? ' active' : ''}`}
          onClick={() => onChange(tab.id)}
        >
          <span className="mobile-tab-icon">{tab.icon}</span>
          <span className="mobile-tab-label">{tab.label}</span>
          {counts[tab.id] > 0 && <span className="mobile-tab-badge">{counts[tab.id]}</span>}
        </button>
      ))}
    </nav>
  );
}

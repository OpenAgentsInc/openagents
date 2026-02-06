import { Link } from '@tanstack/react-router';
import { useCallback, useEffect, useState } from 'react';

const SITE_TITLE = 'OpenAgents';
const STORAGE_KEY = 'autopilot-sidebar-collapsed';
const WIDTH_EXPANDED = '16rem';
const WIDTH_COLLAPSED = '3rem';

function PanelIcon({ className }: { className?: string }) {
  return (
    <svg
      className={className}
      width="20"
      height="20"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <rect x="3" y="3" width="7" height="18" rx="1" />
      <rect x="14" y="3" width="7" height="18" rx="1" />
    </svg>
  );
}

export function AutopilotSidebar() {
  const [collapsed, setCollapsed] = useState(() => {
    if (typeof window === 'undefined') return false;
    return window.localStorage.getItem(STORAGE_KEY) === 'true';
  });

  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEY, String(collapsed));
  }, [collapsed]);

  const toggle = useCallback(() => setCollapsed((c) => !c), []);

  return (
    <aside
      className="hidden md:flex h-full flex-col shrink-0 border-r border-border-dark bg-bg-secondary text-text-primary transition-[width] duration-200 ease-linear"
      style={{ width: collapsed ? WIDTH_COLLAPSED : WIDTH_EXPANDED }}
    >
      <header className="relative flex h-12 shrink-0 flex-row items-center gap-2 border-b border-border-dark px-2">
        <button
          type="button"
          onClick={toggle}
          className="flex size-8 shrink-0 items-center justify-center rounded text-text-muted hover:text-text-primary hover:bg-surface-primary focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
          aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
        >
          <PanelIcon className="size-5" />
        </button>
        <Link
          to="/"
          className={`flex flex-1 items-center justify-center text-sm font-semibold text-accent transition-opacity duration-200 ease-linear ${
            collapsed ? 'w-0 overflow-hidden opacity-0' : 'opacity-100'
          }`}
        >
          {SITE_TITLE}
        </Link>
      </header>
      <div className="flex min-h-0 flex-1 flex-col overflow-auto" data-collapsed={collapsed ? '' : undefined}>
        {!collapsed ? <div className="p-2" /> : null}
      </div>
      <footer className="shrink-0 border-t border-border-dark p-2">
        {!collapsed ? <div className="h-6" /> : null}
      </footer>
    </aside>
  );
}

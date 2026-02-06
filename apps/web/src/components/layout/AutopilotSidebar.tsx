import { useAtomSet, useAtomValue } from '@effect-atom/atom-react';
import { Link, useRouter } from '@tanstack/react-router';
import { useCallback, useEffect, useRef, useState } from 'react';
import { SessionAtom } from '../../effect/atoms/session';
import { clearRootAuthCache } from '../../routes/__root';

const SITE_TITLE = 'OpenAgents';
const STORAGE_KEY = 'autopilot-sidebar-collapsed';
const WIDTH_EXPANDED = '16rem';
const WIDTH_COLLAPSED = '3rem';

function getInitials(user: { firstName?: string | null; lastName?: string | null; email?: string | null }): string {
  const first = user.firstName?.trim().slice(0, 1) ?? '';
  const last = user.lastName?.trim().slice(0, 1) ?? '';
  if (first || last) return (first + last).toUpperCase();
  const email = user.email?.trim() ?? '';
  if (email) return email.slice(0, 2).toUpperCase();
  return '?';
}

function useSignOutAction() {
  const router = useRouter();
  const setSession = useAtomSet(SessionAtom);

  return useCallback(async () => {
    try {
      await fetch('/api/auth/signout', {
        method: 'POST',
        credentials: 'include',
      });
    } catch {
      // Best-effort sign-out; always clear local session state.
    } finally {
      clearRootAuthCache();
      setSession({ userId: null, user: null });
      router.navigate({ href: '/' }).catch(() => {});
    }
  }, [router, setSession]);
}

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

function SidebarUserMenu() {
  const session = useAtomValue(SessionAtom);
  const user = session.user;
  const signOut = useSignOutAction();
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!menuOpen) return;
    const handleClick = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) setMenuOpen(false);
    };
    document.addEventListener('click', handleClick);
    return () => document.removeEventListener('click', handleClick);
  }, [menuOpen]);

  if (!user) return null;

  const displayName =
    [user.firstName, user.lastName].filter(Boolean).join(' ') || user.email || 'Account';
  const initials = getInitials(user);

  return (
    <div className="relative" ref={menuRef}>
      <button
        type="button"
        onClick={() => setMenuOpen((o) => !o)}
        className="flex w-full items-center gap-2 rounded-md px-2 py-2 text-left text-text-primary hover:bg-surface-primary focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
        aria-expanded={menuOpen}
        aria-haspopup="true"
        aria-label="User menu"
      >
        <span
          className="flex size-8 shrink-0 items-center justify-center rounded-full border border-border-dark bg-surface-primary text-xs font-medium text-accent"
          aria-hidden
        >
          {initials}
        </span>
        <span className="min-w-0 flex-1 truncate text-sm text-text-primary">{displayName}</span>
        <svg
          className="size-4 shrink-0 text-text-dim"
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          aria-hidden
        >
          <path d="M6 9l6 6 6-6" />
        </svg>
      </button>
      {menuOpen && (
        <div
          className="absolute bottom-full left-0 right-0 mb-1 rounded border border-border-dark bg-bg-secondary py-1 shadow-lg"
          role="menu"
        >
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              setMenuOpen(false);
              void signOut();
            }}
            className="w-full px-3 py-2 text-left text-sm text-text-primary hover:bg-surface-primary focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
          >
            Log out
          </button>
        </div>
      )}
    </div>
  );
}

function SidebarUserCollapsed() {
  const session = useAtomValue(SessionAtom);
  const user = session.user;
  const signOut = useSignOutAction();
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!menuOpen) return;
    const handleClick = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) setMenuOpen(false);
    };
    document.addEventListener('click', handleClick);
    return () => document.removeEventListener('click', handleClick);
  }, [menuOpen]);

  if (!user) return null;
  const initials = getInitials(user);

  return (
    <div className="relative flex justify-center" ref={menuRef}>
      <button
        type="button"
        onClick={() => setMenuOpen((o) => !o)}
        className="flex size-8 items-center justify-center rounded-full border border-border-dark bg-surface-primary text-xs font-medium text-accent hover:bg-surface-primary/80 focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
        aria-expanded={menuOpen}
        aria-haspopup="true"
        aria-label="User menu"
      >
        {initials}
      </button>
      {menuOpen && (
        <div
          className="absolute bottom-full left-1/2 mb-1 min-w-[120px] -translate-x-1/2 rounded border border-border-dark bg-bg-secondary py-1 shadow-lg"
          role="menu"
        >
          <div className="truncate px-3 py-2 text-xs text-text-dim">{user.email}</div>
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              setMenuOpen(false);
              void signOut();
            }}
            className="w-full px-3 py-2 text-left text-sm text-text-primary hover:bg-surface-primary focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
          >
            Log out
          </button>
        </div>
      )}
    </div>
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
        {collapsed ? <SidebarUserCollapsed /> : <SidebarUserMenu />}
      </footer>
    </aside>
  );
}

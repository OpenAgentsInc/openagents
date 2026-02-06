import { useAtomSet, useAtomValue } from '@effect-atom/atom-react';
import { useRouter, useRouterState } from '@tanstack/react-router';
import { useCallback, useEffect, useMemo, useRef } from 'react';
import { EffuseMount } from '../EffuseMount';
import { SessionAtom } from '../../effect/atoms/session';
import {
  AutopilotSidebarCollapsedAtom,
  AutopilotSidebarUserMenuOpenAtom,
} from '../../effect/atoms/autopilotUi';
import { clearRootAuthCache } from '../../routes/__root';
import { runAutopilotSidebar } from '../../effuse-pages/autopilotSidebar';

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

export function AutopilotSidebar() {
  const session = useAtomValue(SessionAtom);
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const signOut = useSignOutAction();

  const collapsed = useAtomValue(AutopilotSidebarCollapsedAtom);
  const setCollapsed = useAtomSet(AutopilotSidebarCollapsedAtom);
  const userMenuOpen = useAtomValue(AutopilotSidebarUserMenuOpenAtom);
  const setUserMenuOpen = useAtomSet(AutopilotSidebarUserMenuOpenAtom);
  const mountElRef = useRef<Element | null>(null);

  useEffect(() => {
    if (!userMenuOpen) return;
    const onDocClick = (e: MouseEvent) => {
      const root = mountElRef.current;
      if (!root) return;
      if (!(e.target instanceof Node)) return;
      if (!root.contains(e.target)) setUserMenuOpen(false);
    };
    document.addEventListener('click', onDocClick);
    return () => document.removeEventListener('click', onDocClick);
  }, [userMenuOpen]);

  const model = useMemo(
    () => ({
      collapsed,
      pathname,
      user: session.user
        ? {
            email: session.user.email,
            firstName: session.user.firstName,
            lastName: session.user.lastName,
          }
        : null,
      userMenuOpen,
    }),
    [collapsed, pathname, session.user, userMenuOpen],
  );

  const run = useCallback((el: Element) => runAutopilotSidebar(el, model), [model]);

  const onRendered = useCallback((container: Element) => {
    mountElRef.current = container;

    const toggleBtn = container.querySelector('[data-action="toggle-collapse"]');
    toggleBtn?.addEventListener('click', () => {
      setUserMenuOpen(false);
      setCollapsed((c) => !c);
    });

    const menuToggle = container.querySelector('[data-action="toggle-user-menu"]');
    menuToggle?.addEventListener('click', () => setUserMenuOpen((o) => !o));

    const logoutBtn = container.querySelector('[data-action="logout"]');
    logoutBtn?.addEventListener('click', () => {
      setUserMenuOpen(false);
      void signOut();
    });
  }, [signOut]);

  const widthClass = collapsed ? 'w-12' : 'w-64';

  return (
    <EffuseMount
      run={run}
      deps={[collapsed, pathname, session.user?.id ?? null, userMenuOpen]}
      onRendered={onRendered}
      className={`hidden md:flex h-full flex-col shrink-0 border-r border-border-dark bg-bg-secondary text-text-primary transition-[width] duration-200 ease-linear ${widthClass}`}
    />
  );
}

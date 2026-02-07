import { Atom } from '@effect-atom/atom';
import * as Option from 'effect/Option';

const STORAGE_KEY = 'autopilot-sidebar-collapsed';

function readCollapsedFromStorage(): boolean {
  if (typeof window === 'undefined') return false;
  return window.localStorage.getItem(STORAGE_KEY) === 'true';
}

export const AutopilotSidebarCollapsedAtom = Atom.writable(
  (get) => {
    const existing = get.self<boolean>();
    if (Option.isSome(existing)) return existing.value;
    const initial = readCollapsedFromStorage();
    get.setSelf(initial);
    return initial;
  },
  (ctx, value: boolean) => {
    ctx.setSelf(value);
    if (typeof window !== 'undefined') {
      window.localStorage.setItem(STORAGE_KEY, String(value));
    }
  },
).pipe(Atom.keepAlive, Atom.withLabel('@openagents/web/autopilot/sidebarCollapsed'));

export const AutopilotSidebarUserMenuOpenAtom = Atom.writable(
  (get) => {
    const existing = get.self<boolean>();
    if (Option.isSome(existing)) return existing.value;
    get.setSelf(false);
    return false;
  },
  (ctx, value: boolean) => {
    ctx.setSelf(value);
  },
).pipe(
  Atom.keepAlive,
  Atom.withLabel('@openagents/web/autopilot/sidebarUserMenuOpen'),
);


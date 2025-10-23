import AsyncStorage from '@react-native-async-storage/async-storage';

export type LogKind = 'md' | 'reason' | 'text' | 'json' | 'summary' | 'delta' | 'exec' | 'file' | 'search' | 'mcp' | 'todo' | 'cmd' | 'err' | 'turn' | 'thread' | 'item_lifecycle';

export type LogDetail = {
  id: number;
  text: string;
  kind: LogKind;
  deemphasize?: boolean;
  ts?: number;
  // Optional pointer: for summary/preview items, link to the full JSON/detail entry.
  detailId?: number;
};

const store = new Map<number, LogDetail>();
type Listener = () => void;
const listeners = new Set<Listener>();
const KEY = '@openagents/logs-v1';
let hydrated = false;
let loadPromise: Promise<LogDetail[]> | null = null;
let saveTimer: ReturnType<typeof setTimeout> | null = null;

function notify() {
  listeners.forEach((listener) => {
    try { listener(); } catch {}
  });
}

export function subscribe(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function putLog(detail: LogDetail) {
  try {
    const txt = String(detail.text ?? '');
    if (txt.includes('exec_command_end')) {
      return; // hard filter noisy exec end blobs from ever entering the store
    }
  } catch {}
  store.set(detail.id, detail);
  notify();
  scheduleSave();
}

export function getLog(id: number): LogDetail | undefined {
  return store.get(id);
}

export function getAllLogs(): LogDetail[] {
  return Array.from(store.values()).sort((a, b) => (a.id - b.id));
}

function scheduleSave() {
  if (saveTimer) return;
  saveTimer = setTimeout(() => {
    saveTimer = null;
    persistNow().catch(() => {});
  }, 150);
}

async function persistNow() {
  try {
    const arr = getAllLogs();
    await AsyncStorage.setItem(KEY, JSON.stringify(arr));
  } catch {}
}

export function isHydrated(): boolean {
  return hydrated;
}

export async function loadLogs(): Promise<LogDetail[]> {
  if (hydrated && !loadPromise) {
    return getAllLogs();
  }
  if (!loadPromise) {
    loadPromise = (async () => {
      try {
        const raw = await AsyncStorage.getItem(KEY);
        if (!raw) {
          hydrated = true;
          notify();
          return [];
        }
        const arr: LogDetail[] = JSON.parse(raw);
        const sanitized = arr.filter((d) => {
          try { return !String(d.text ?? '').includes('exec_command_end'); } catch { return true; }
        });
        if (sanitized.length !== arr.length) {
          try { await AsyncStorage.setItem(KEY, JSON.stringify(sanitized)); } catch {}
        }
        store.clear();
        for (const d of sanitized) store.set(d.id, d);
        hydrated = true;
        notify();
        return getAllLogs();
      } catch {
        hydrated = true;
        notify();
        return [];
      } finally {
        loadPromise = null;
      }
    })();
  }
  const pending = loadPromise;
  return pending!;
}

export async function saveLogs(): Promise<void> {
  scheduleSave();
}

export async function flushLogs(): Promise<void> {
  if (saveTimer) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
  await persistNow();
}

export async function clearLogs(): Promise<void> {
  store.clear();
  if (saveTimer) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
  try { await AsyncStorage.removeItem(KEY); } catch {}
  hydrated = true;
  notify();
}

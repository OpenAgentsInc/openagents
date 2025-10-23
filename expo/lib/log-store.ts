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
}

export function getLog(id: number): LogDetail | undefined {
  return store.get(id);
}

export function getAllLogs(): LogDetail[] {
  return Array.from(store.values()).sort((a, b) => (a.id - b.id));
}

export async function loadLogs(): Promise<LogDetail[]> {
  try {
    const raw = await AsyncStorage.getItem(KEY);
    if (!raw) return [];
    const arr: LogDetail[] = JSON.parse(raw);
    // Sanitize persisted logs: drop any exec_command_end blobs that may have been
    // saved before we added runtime filtering. This avoids showing them after
    // rehydration without requiring users to clear the log.
    const sanitized = arr.filter((d) => {
      try { return !String(d.text ?? '').includes('exec_command_end'); } catch { return true }
    });
    // Persist the sanitized list so it stays clean across restarts.
    try { if (sanitized.length !== arr.length) await AsyncStorage.setItem(KEY, JSON.stringify(sanitized)); } catch {}
    store.clear();
    for (const d of sanitized) store.set(d.id, d);
    notify();
    return getAllLogs();
  } catch {
    return [];
  }
}

export async function saveLogs(): Promise<void> {
  try {
    const arr = getAllLogs();
    await AsyncStorage.setItem(KEY, JSON.stringify(arr));
  } catch {}
}

export async function clearLogs(): Promise<void> {
  store.clear();
  try { await AsyncStorage.removeItem(KEY); } catch {}
  notify();
}

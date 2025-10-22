import AsyncStorage from '@react-native-async-storage/async-storage';

export type LogKind = 'md' | 'reason' | 'text' | 'json' | 'summary' | 'delta' | 'exec';

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
const KEY = '@openagents/logs-v1';

export function putLog(detail: LogDetail) {
  store.set(detail.id, detail);
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
    store.clear();
    for (const d of arr) store.set(d.id, d);
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
}

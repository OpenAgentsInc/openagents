const STORAGE_KEY = "clawstr-relay-health-v1";
const MAX_READ_RELAYS = 2;

export type RelayHealth = {
  url: string;
  openCount: number;
  errorCount: number;
  closeCount: number;
  avgOpenMs?: number;
  lastOpenAt?: number;
  lastErrorAt?: number;
};

type RelayHealthState = Record<string, RelayHealth>;

function loadState(): RelayHealthState {
  if (typeof window === "undefined") return {};
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as RelayHealthState;
    return parsed ?? {};
  } catch {
    return {};
  }
}

function saveState(state: RelayHealthState) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // ignore persistence failures
  }
}

function getEntry(state: RelayHealthState, url: string): RelayHealth {
  return (
    state[url] ?? {
      url,
      openCount: 0,
      errorCount: 0,
      closeCount: 0,
    }
  );
}

export function recordRelayOpen(url: string, openMs?: number) {
  if (typeof window === "undefined") return;
  const state = loadState();
  const entry = getEntry(state, url);
  entry.openCount += 1;
  entry.lastOpenAt = Date.now();
  if (typeof openMs === "number" && openMs >= 0) {
    const prevAvg = entry.avgOpenMs ?? openMs;
    entry.avgOpenMs = Math.round((prevAvg * (entry.openCount - 1) + openMs) / entry.openCount);
  }
  state[url] = entry;
  saveState(state);
}

export function recordRelayError(url: string) {
  if (typeof window === "undefined") return;
  const state = loadState();
  const entry = getEntry(state, url);
  entry.errorCount += 1;
  entry.lastErrorAt = Date.now();
  state[url] = entry;
  saveState(state);
}

export function recordRelayClose(url: string) {
  if (typeof window === "undefined") return;
  const state = loadState();
  const entry = getEntry(state, url);
  entry.closeCount += 1;
  state[url] = entry;
  saveState(state);
}

function getRelayScore(entry: RelayHealth | undefined, index: number): number {
  if (!entry) return -index;
  const success = entry.openCount * 5;
  const penalty = entry.errorCount * 15 + entry.closeCount * 5;
  const latencyPenalty = entry.avgOpenMs ? Math.min(50, entry.avgOpenMs / 20) : 0;
  const base = 50;
  return base + success - penalty - latencyPenalty - index;
}

export function pickReadRelays(relays: string[]): string[] {
  if (relays.length <= 1) return relays;
  const state = loadState();
  const ranked = [...relays]
    .map((url, index) => ({ url, index, score: getRelayScore(state[url], index) }))
    .sort((a, b) => b.score - a.score)
    .map((entry) => entry.url);
  const limit = Math.min(MAX_READ_RELAYS, ranked.length);
  const picked = ranked.slice(0, limit);
  return picked.length > 0 ? picked : relays;
}

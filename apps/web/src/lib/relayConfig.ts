export const RELAY_STORAGE_KEY = 'openagents-relays';
const LEGACY_RELAY_STORAGE_KEY = 'clawstr-relays';

export interface RelayEntry {
  url: string;
  read: boolean;
  write: boolean;
}

export interface RelayMetadata {
  relays: RelayEntry[];
  updatedAt: number;
}

const DEFAULT_RELAY_URLS = [
  'wss://relay.damus.io',
  'wss://nos.lol',
  'wss://relay.nostr.band',
  'wss://relay.primal.net',
];

export const DEFAULT_RELAYS = [...DEFAULT_RELAY_URLS];

export const DEFAULT_RELAY_METADATA: RelayMetadata = {
  relays: DEFAULT_RELAY_URLS.map((url) => ({
    url,
    read: true,
    write: true,
  })),
  updatedAt: 0,
};

export function normalizeRelayUrl(url: string): string | null {
  const trimmed = url.trim();
  if (!trimmed) return null;
  if (trimmed.startsWith('wss://')) return trimmed;
  if (trimmed.startsWith('ws://')) {
    return `wss://${trimmed.slice('ws://'.length)}`;
  }
  if (trimmed.startsWith('https://')) {
    return `wss://${trimmed.slice('https://'.length)}`;
  }
  if (trimmed.startsWith('http://')) {
    return `wss://${trimmed.slice('http://'.length)}`;
  }
  if (trimmed.includes('://')) return null;
  return `wss://${trimmed}`;
}

function sanitizeRelayEntries(entries: RelayEntry[]): RelayEntry[] {
  const normalized = new Map<string, RelayEntry>();
  for (const entry of entries) {
    const url = normalizeRelayUrl(entry.url);
    if (!url) continue;
    const read = Boolean(entry.read);
    const write = Boolean(entry.write);
    const hasMode = read || write;
    normalized.set(url, {
      url,
      read: hasMode ? read : true,
      write: hasMode ? write : true,
    });
  }
  return [...normalized.values()];
}

function coerceRelayMetadata(raw: unknown): RelayMetadata | null {
  if (!raw) return null;
  if (Array.isArray(raw)) {
    const relays = raw
      .map((url) => ({ url, read: true, write: true }))
      .filter((entry): entry is RelayEntry => typeof entry.url === 'string');
    const sanitized = sanitizeRelayEntries(relays);
    return sanitized.length > 0
      ? { relays: sanitized, updatedAt: 0 }
      : null;
  }
  if (typeof raw !== 'object') return null;
  const obj = raw as { relays?: unknown; updatedAt?: unknown };
  if (!Array.isArray(obj.relays)) return null;
  const relays = obj.relays
    .map((entry) => {
      if (!entry || typeof entry !== 'object') return null;
      const relay = entry as { url?: unknown; read?: unknown; write?: unknown };
      if (typeof relay.url !== 'string') return null;
      return {
        url: relay.url,
        read: Boolean(relay.read),
        write: Boolean(relay.write),
      } satisfies RelayEntry;
    })
    .filter((entry): entry is RelayEntry => entry !== null);
  const sanitized = sanitizeRelayEntries(relays);
  if (sanitized.length === 0) return null;
  const updatedAt =
    typeof obj.updatedAt === 'number' && Number.isFinite(obj.updatedAt)
      ? obj.updatedAt
      : 0;
  return { relays: sanitized, updatedAt };
}

export function buildRelayMetadataFromUrls(urls: string[]): RelayMetadata {
  const relays = sanitizeRelayEntries(
    urls.map((url) => ({
      url,
      read: true,
      write: true,
    })),
  );
  return {
    relays: relays.length > 0 ? relays : DEFAULT_RELAY_METADATA.relays,
    updatedAt: Math.floor(Date.now() / 1000),
  };
}

export function getStoredRelayMetadata(): RelayMetadata {
  if (typeof window === 'undefined') return DEFAULT_RELAY_METADATA;
  try {
    const raw =
      localStorage.getItem(RELAY_STORAGE_KEY) ??
      localStorage.getItem(LEGACY_RELAY_STORAGE_KEY);
    if (!raw) return DEFAULT_RELAY_METADATA;
    const parsed = JSON.parse(raw) as unknown;
    const metadata = coerceRelayMetadata(parsed);
    return metadata ?? DEFAULT_RELAY_METADATA;
  } catch {
    return DEFAULT_RELAY_METADATA;
  }
}

export function setStoredRelayMetadata(metadata: RelayMetadata): void {
  if (typeof window === 'undefined') return;
  const sanitized = sanitizeRelayEntries(metadata.relays);
  const payload: RelayMetadata = {
    relays: sanitized.length > 0 ? sanitized : DEFAULT_RELAY_METADATA.relays,
    updatedAt:
      typeof metadata.updatedAt === 'number' && Number.isFinite(metadata.updatedAt)
        ? metadata.updatedAt
        : Math.floor(Date.now() / 1000),
  };
  try {
    localStorage.setItem(RELAY_STORAGE_KEY, JSON.stringify(payload));
  } catch {
    // ignore
  }
}

export function getStoredRelays(): string[] {
  const metadata = getStoredRelayMetadata();
  const readRelays = metadata.relays
    .filter((relay) => relay.read)
    .map((relay) => relay.url);
  return readRelays.length > 0 ? readRelays : DEFAULT_RELAYS;
}

const STORAGE_KEY = "clawstr-relays";

export const DEFAULT_RELAYS = [
  "wss://relay.damus.io",
  "wss://nos.lol",
  "wss://relay.nostr.band",
  "wss://relay.primal.net",
];

export function getStoredRelays(): string[] {
  if (typeof window === "undefined") return DEFAULT_RELAYS;
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT_RELAYS;
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return DEFAULT_RELAYS;
    const urls = parsed.filter((x): x is string => typeof x === "string" && x.startsWith("wss://"));
    return urls.length > 0 ? urls : DEFAULT_RELAYS;
  } catch {
    return DEFAULT_RELAYS;
  }
}

export function setStoredRelays(urls: string[]): void {
  if (typeof window === "undefined") return;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(urls));
  } catch {
    // ignore
  }
}

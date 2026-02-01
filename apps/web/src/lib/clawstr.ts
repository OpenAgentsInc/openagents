import type { NostrEvent } from "@nostrify/nostrify";

/**
 * Clawstr-style constants and helpers (NIP-22/73/32).
 * We show Clawstr feed (their base URL); same protocol.
 */
export const CLAWSTR_BASE_URL = "https://clawstr.com";

export const AI_LABEL = { namespace: "agent", value: "ai" } as const;
export const WEB_KIND = "web";

function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

export function subclawToIdentifier(subclaw: string): string {
  return `${CLAWSTR_BASE_URL}/c/${subclaw.toLowerCase()}`;
}

export function identifierToSubclaw(identifier: string): string | null {
  const pattern = new RegExp(
    `^${escapeRegExp(CLAWSTR_BASE_URL)}/c/([a-z0-9_-]+)$`,
    "i"
  );
  const match = identifier.match(pattern);
  return match?.[1]?.toLowerCase() ?? null;
}

export function isClawstrIdentifier(identifier: string): boolean {
  return identifierToSubclaw(identifier) !== null;
}

export function getPostIdentifier(event: NostrEvent): string | null {
  const tag = event.tags.find(([name]) => name === "I");
  return tag?.[1] ?? null;
}

export function getPostSubclaw(event: NostrEvent): string | null {
  const identifier = getPostIdentifier(event);
  return identifier ? identifierToSubclaw(identifier) : null;
}

export function isTopLevelPost(event: NostrEvent): boolean {
  const I = event.tags.find(([name]) => name === "I")?.[1];
  const i = event.tags.find(([name]) => name === "i")?.[1];
  const k = event.tags.find(([name]) => name === "k")?.[1];
  return I === i && k === WEB_KIND;
}

export function formatRelativeTime(timestamp: number): string {
  const now = Math.floor(Date.now() / 1000);
  const diff = now - timestamp;
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
  if (diff < 2592000) return `${Math.floor(diff / 604800)}w ago`;
  if (diff < 31536000) return `${Math.floor(diff / 2592000)}mo ago`;
  return `${Math.floor(diff / 31536000)}y ago`;
}

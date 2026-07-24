import type { SarahNostrEventTemplate } from "../nostr-identity/types.ts";
import { assertSarahNostrPublicSafe } from "../nostr-identity/redaction.ts";
import {
  MAX_CLIENT_ID_LENGTH,
  MAX_CONTEXT_ENTRIES,
  MAX_CONTEXT_ID_BYTES,
  MAX_CONTEXT_TIMESTAMP,
  READ_STATE_ALT,
  READ_STATE_D_PREFIX,
  READ_STATE_T_VALUE,
  READ_STATE_VERSION,
  SARAH_READ_STATE_KIND,
  type SarahNostrMemoryCipher,
  type SarahReadContexts,
  type SarahReadStateBlob,
} from "./types.ts";

const utf8 = new TextEncoder();
const ASCII_SLOT = /^[\x20-\x7E]{1,64}$/;
const HEX64 = /^[0-9a-f]{64}$/;

/** Build addressable `d` = `read-state:<slot-id>`. */
export const buildReadStateDTag = (slotId: string): string | null => {
  if (!ASCII_SLOT.test(slotId)) return null;
  return `${READ_STATE_D_PREFIX}${slotId}`;
};

export const parseReadStateSlotId = (d: string): string | null => {
  if (!d.startsWith(READ_STATE_D_PREFIX)) return null;
  const slotId = d.slice(READ_STATE_D_PREFIX.length);
  if (!ASCII_SLOT.test(slotId)) return null;
  return slotId;
};

/**
 * Grow-only max-register merge (NIP-RS CvRDT join).
 * Associative, commutative, idempotent.
 *
 * effective[ctx] = max(timestamp) across all inputs
 */
export const mergeReadContexts = (
  ...maps: readonly SarahReadContexts[]
): SarahReadContexts => {
  const out: Record<string, number> = {};
  for (const map of maps) {
    for (const [ctx, ts] of Object.entries(map)) {
      const prev = out[ctx];
      if (prev === undefined || ts > prev) {
        out[ctx] = ts;
      }
    }
  }
  return out;
};

/** Monotonic advance: only raise frontiers (never lower). */
export const advanceReadContexts = (
  current: SarahReadContexts,
  patch: SarahReadContexts,
): SarahReadContexts => mergeReadContexts(current, patch);

const utf8ByteLength = (s: string): number => utf8.encode(s).length;

/**
 * Validate a decrypted read-state blob per NIP-RS content rules.
 * Returns null when the blob must be discarded.
 */
export const validateReadStateBlob = (
  input: string | unknown,
): SarahReadStateBlob | null => {
  let raw: unknown = input;
  if (typeof input === "string") {
    try {
      raw = JSON.parse(input);
    } catch {
      return null;
    }
  }
  if (raw === null || typeof raw !== "object" || Array.isArray(raw)) {
    return null;
  }
  const obj = raw as Record<string, unknown>;
  const v = obj["v"];
  if (typeof v !== "number" || !Number.isInteger(v)) return null;
  if (v !== READ_STATE_VERSION) return null;

  const clientId = obj["client_id"];
  if (typeof clientId !== "string") return null;
  if (clientId.length < 1 || clientId.length > MAX_CLIENT_ID_LENGTH) return null;

  const contextsRaw = obj["contexts"];
  if (
    contextsRaw === null ||
    typeof contextsRaw !== "object" ||
    Array.isArray(contextsRaw)
  ) {
    return null;
  }

  const entries = Object.entries(contextsRaw as Record<string, unknown>);
  if (entries.length > MAX_CONTEXT_ENTRIES) return null;

  const contexts: Record<string, number> = {};
  for (const [ctxId, ts] of entries) {
    if (utf8ByteLength(ctxId) > MAX_CONTEXT_ID_BYTES) continue;
    if (typeof ts !== "number" || !Number.isInteger(ts)) continue;
    if (ts < 0 || ts > MAX_CONTEXT_TIMESTAMP) continue;
    contexts[ctxId] = ts;
  }

  return {
    v: READ_STATE_VERSION,
    client_id: clientId,
    contexts,
  };
};

export const buildReadStateBlob = (input: {
  readonly clientId: string;
  readonly contexts: SarahReadContexts;
}): SarahReadStateBlob => {
  if (
    input.clientId.length < 1 ||
    input.clientId.length > MAX_CLIENT_ID_LENGTH
  ) {
    throw new Error("sarah_nostr_memory: client_id length 1–64");
  }
  const validated = validateReadStateBlob({
    v: READ_STATE_VERSION,
    client_id: input.clientId,
    contexts: input.contexts,
  });
  if (!validated) {
    throw new Error("sarah_nostr_memory: invalid read-state contexts");
  }
  return validated;
};

export const serializeReadStateBlob = (blob: SarahReadStateBlob): string =>
  JSON.stringify({
    v: blob.v,
    client_id: blob.client_id,
    contexts: blob.contexts,
  });

/**
 * Build an unsigned NIP-RS read-state event (kind 30078).
 * Content is encrypt-to-self via the injected cipher.
 */
export const buildReadStateWriteTemplate = (input: {
  readonly slotId: string;
  readonly blob: SarahReadStateBlob;
  readonly cipher: SarahNostrMemoryCipher;
  readonly createdAt?: number;
  readonly alt?: string;
}): {
  readonly template: SarahNostrEventTemplate;
  readonly d: string;
  readonly plaintext: string;
} => {
  const d = buildReadStateDTag(input.slotId);
  if (d === null) {
    throw new Error("sarah_nostr_memory: invalid read-state slotId");
  }
  const plaintext = serializeReadStateBlob(input.blob);
  const ciphertext = input.cipher.encryptToOwner(plaintext);
  if (!ciphertext || ciphertext.trim() === "") {
    throw new Error("sarah_nostr_memory: cipher returned empty content");
  }
  if (ciphertext.includes('"client_id"') || ciphertext.includes('"contexts"')) {
    throw new Error("sarah_nostr_memory: cipher must not return plaintext blob");
  }

  const template: SarahNostrEventTemplate = {
    kind: SARAH_READ_STATE_KIND,
    created_at: input.createdAt ?? Math.floor(Date.now() / 1000),
    tags: [
      ["d", d],
      ["t", READ_STATE_T_VALUE],
      ["alt", input.alt ?? READ_STATE_ALT],
    ],
    content: ciphertext,
  };
  assertSarahNostrPublicSafe(template);
  return { template, d, plaintext };
};

export const readReadStateBlob = (input: {
  readonly content: string;
  readonly cipher: SarahNostrMemoryCipher;
}): SarahReadStateBlob | null => {
  const plaintext = input.cipher.decryptFromOwner(input.content);
  return validateReadStateBlob(plaintext);
};

/** Well-known thread context key. */
export const threadContextKey = (rootEventId: string): string => {
  if (!HEX64.test(rootEventId)) {
    throw new Error("sarah_nostr_memory: thread root must be 64 hex");
  }
  return `thread:${rootEventId}`;
};

/** Well-known per-message context key. */
export const msgContextKey = (eventId: string): string => {
  if (!HEX64.test(eventId)) {
    throw new Error("sarah_nostr_memory: msg id must be 64 hex");
  }
  return `msg:${eventId}`;
};

/** Sarah conversation read frontier key (owner-scoped conversation tag). */
export const sarahConversationContextKey = (conversation: string): string => {
  if (!/^sarah\.[0-9a-f]{24}$/.test(conversation)) {
    throw new Error(
      "sarah_nostr_memory: conversation must match sarah.<24 hex>",
    );
  }
  return `sarah-conversation:${conversation}`;
};

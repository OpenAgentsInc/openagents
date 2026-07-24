import { randomBytes, bytesToHex } from "@noble/hashes/utils";

import type { SarahNostrEventTemplate } from "../nostr-identity/types.ts";
import { assertSarahNostrPublicSafe } from "../nostr-identity/redaction.ts";
import { assertSarahMemoryValueStorable } from "./redaction.ts";
import {
  REMINDER_ALT,
  SARAH_REMINDER_KIND,
  type SarahNostrMemoryCipher,
  type SarahReminderContent,
  type SarahReminderStatus,
} from "./types.ts";

const MAX_NOT_BEFORE = 9_007_199_254_740_991;

/** Fresh opaque `d` (128 bits hex). */
export const generateReminderId = (): string => bytesToHex(randomBytes(16));

/**
 * Parse `not_before` under NIP-ER rules: ASCII digits only, no leading zero
 * except `"0"`, within [0, MAX_SAFE_INTEGER].
 */
export const parseNotBefore = (value: string | undefined): number | null => {
  if (value === undefined) return null;
  if (!/^(0|[1-9][0-9]*)$/.test(value)) return null;
  const n = Number(value);
  if (!Number.isSafeInteger(n) || n < 0 || n > MAX_NOT_BEFORE) return null;
  return n;
};

export const getReminderNotBefore = (event: {
  readonly tags: readonly (readonly string[])[];
}): number | null => {
  const found = event.tags.filter((t) => t[0] === "not_before");
  if (found.length !== 1) return null;
  return parseNotBefore(found[0]?.[1]);
};

export const getReminderD = (event: {
  readonly tags: readonly (readonly string[])[];
}): string | null => {
  const found = event.tags.filter((t) => t[0] === "d");
  if (found.length !== 1) return null;
  const d = found[0]?.[1];
  return d && d.length > 0 ? d : null;
};

export const buildReminderContent = (input: {
  readonly status: SarahReminderStatus;
  readonly note?: string;
  readonly target?: SarahReminderContent["target"];
}): SarahReminderContent => {
  if (input.note !== undefined) {
    assertSarahMemoryValueStorable(input.note);
  }
  if (input.target?.preview !== undefined) {
    assertSarahMemoryValueStorable(input.target.preview);
  }
  return {
    status: input.status,
    ...(input.note !== undefined ? { note: input.note } : {}),
    ...(input.target !== undefined ? { target: input.target } : {}),
  };
};

export const serializeReminderContent = (
  content: SarahReminderContent,
): string => JSON.stringify(content);

export const parseReminderContent = (
  plaintext: string,
): SarahReminderContent | null => {
  let raw: unknown;
  try {
    raw = JSON.parse(plaintext);
  } catch {
    return null;
  }
  if (raw === null || typeof raw !== "object" || Array.isArray(raw)) {
    return null;
  }
  const obj = raw as Record<string, unknown>;
  const status = obj["status"];
  if (status !== "pending" && status !== "done" && status !== "cancelled") {
    return null;
  }
  const content: SarahReminderContent = { status };
  if (typeof obj["note"] === "string") {
    (content as { note?: string }).note = obj["note"];
  }
  if (
    obj["target"] !== undefined &&
    obj["target"] !== null &&
    typeof obj["target"] === "object" &&
    !Array.isArray(obj["target"])
  ) {
    (content as { target?: SarahReminderContent["target"] }).target = obj[
      "target"
    ] as SarahReminderContent["target"];
  }
  return content;
};

/**
 * Build an unsigned NIP-ER reminder event (kind 30300).
 * Content is NIP-44 encrypt-to-self via the cipher port.
 * `not_before` is public for pending reminders only.
 * `expiration` is NIP-40 cleanup (must be > notBefore when both set).
 */
export const buildReminderWriteTemplate = (input: {
  readonly content: SarahReminderContent;
  readonly cipher: SarahNostrMemoryCipher;
  readonly d?: string;
  readonly notBefore?: number;
  readonly expiration?: number;
  readonly createdAt?: number;
  readonly alt?: string;
}): {
  readonly template: SarahNostrEventTemplate;
  readonly d: string;
  readonly plaintext: string;
} => {
  const d = input.d ?? generateReminderId();
  if (d.length < 1 || d.length > 128) {
    throw new Error("sarah_nostr_memory: reminder d length invalid");
  }

  if (input.content.status === "pending") {
    if (input.notBefore === undefined) {
      throw new Error(
        "sarah_nostr_memory: pending reminder requires notBefore",
      );
    }
    if (
      !Number.isSafeInteger(input.notBefore) ||
      input.notBefore < 0 ||
      input.notBefore > MAX_NOT_BEFORE
    ) {
      throw new Error("sarah_nostr_memory: invalid notBefore");
    }
  }

  if (input.expiration !== undefined) {
    if (
      !Number.isSafeInteger(input.expiration) ||
      input.expiration < 0 ||
      input.expiration > MAX_NOT_BEFORE
    ) {
      throw new Error("sarah_nostr_memory: invalid expiration");
    }
    if (
      input.notBefore !== undefined &&
      input.expiration <= input.notBefore
    ) {
      throw new Error(
        "sarah_nostr_memory: expiration must be greater than notBefore",
      );
    }
  }

  if (input.content.note !== undefined) {
    assertSarahMemoryValueStorable(input.content.note);
  }

  const plaintext = serializeReminderContent(input.content);
  const ciphertext = input.cipher.encryptToOwner(plaintext);
  if (!ciphertext || ciphertext.trim() === "") {
    throw new Error("sarah_nostr_memory: cipher returned empty content");
  }
  if (ciphertext.includes('"status"') || ciphertext.includes('"note"')) {
    throw new Error(
      "sarah_nostr_memory: cipher must not return plaintext reminder",
    );
  }

  const tags: string[][] = [
    ["d", d],
    ["alt", input.alt ?? REMINDER_ALT],
  ];
  if (input.content.status === "pending" && input.notBefore !== undefined) {
    tags.push(["not_before", String(input.notBefore)]);
  }
  if (input.expiration !== undefined) {
    tags.push(["expiration", String(input.expiration)]);
  }

  const template: SarahNostrEventTemplate = {
    kind: SARAH_REMINDER_KIND,
    created_at: input.createdAt ?? Math.floor(Date.now() / 1000),
    tags,
    content: ciphertext,
  };
  assertSarahNostrPublicSafe(template);
  return { template, d, plaintext };
};

export const readReminderContent = (input: {
  readonly content: string;
  readonly cipher: SarahNostrMemoryCipher;
}): SarahReminderContent | null => {
  const plaintext = input.cipher.decryptFromOwner(input.content);
  return parseReminderContent(plaintext);
};

/** Addressable coordinate `30300:<author-pubkey>:<d>`. */
export const reminderAddress = (authorPubkey: string, d: string): string =>
  `${SARAH_REMINDER_KIND}:${authorPubkey}:${d}`;

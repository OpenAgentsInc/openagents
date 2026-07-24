import { hmac } from "@noble/hashes/hmac";
import { sha256 } from "@noble/hashes/sha256";
import { bytesToHex, hexToBytes } from "@noble/hashes/utils";
import { Schema as S } from "effect";

import type { SarahNostrEventTemplate } from "../nostr-identity/types.ts";
import { assertSarahNostrPublicSafe } from "../nostr-identity/redaction.ts";
import { assertSarahMemoryValueStorable } from "./redaction.ts";
import {
  CORE_SLUG,
  ENGRAM_ALT,
  ENGRAM_D_TAG_DOMAIN,
  FORBIDDEN_DURABLE_MEMORY_FIELDS,
  MAX_ENGRAM_PLAINTEXT_BYTES,
  MAX_SLUG_BYTES,
  MEMORY_SLUG_PATTERN,
  SARAH_ENGRAM_KIND,
  SARAH_NIP_AE_COMPANION_SCHEMA,
  SarahNipAeCompanion,
  type SarahEngramBody,
  type SarahEngramConversation,
  type SarahEngramCoreBody,
  type SarahEngramMemoryBody,
  type SarahNostrMemoryCipher,
  type SarahNipAeCompanion as Companion,
} from "./types.ts";

const decodeCompanion = S.decodeUnknownSync(SarahNipAeCompanion);
const utf8 = new TextEncoder();

export const isValidSlug = (slug: string): boolean => {
  if (slug.length === 0 || slug.length > MAX_SLUG_BYTES) return false;
  if (utf8.encode(slug).length > MAX_SLUG_BYTES) return false;
  if (slug === CORE_SLUG) return true;
  return MEMORY_SLUG_PATTERN.test(slug);
};

export const isCoreSlug = (slug: string): boolean => slug === CORE_SLUG;

export const isMemorySlug = (slug: string): boolean =>
  MEMORY_SLUG_PATTERN.test(slug) && slug.length <= MAX_SLUG_BYTES;

/**
 * HMAC-blinded addressable `d` tag (NIP-AE):
 * d = lower_hex(HMAC-SHA256(K_c, utf8("agent-memory/v1/d-tag") || 0x00 || utf8(slug)))
 */
export const deriveEngramDTag = (
  conversationKeyHex: string,
  slug: string,
): string => {
  if (!/^[0-9a-f]{64}$/.test(conversationKeyHex)) {
    throw new Error("sarah_nostr_memory: conversationKeyHex must be 64 lowercase hex");
  }
  if (!isValidSlug(slug)) {
    throw new Error(`sarah_nostr_memory: invalid slug: ${slug}`);
  }
  const key = hexToBytes(conversationKeyHex);
  const domain = utf8.encode(ENGRAM_D_TAG_DOMAIN);
  const slugBytes = utf8.encode(slug);
  const msg = new Uint8Array(domain.length + 1 + slugBytes.length);
  msg.set(domain, 0);
  msg[domain.length] = 0x00;
  msg.set(slugBytes, domain.length + 1);
  return bytesToHex(hmac(sha256, key, msg));
};

/** Content digest of redacted value UTF-8 bytes. */
export const contentDigestOf = (value: string): `sha256:${string}` =>
  `sha256:${bytesToHex(sha256(utf8.encode(value)))}`;

const assertNoForbiddenFields = (value: unknown, path = "$"): void => {
  if (value === null || value === undefined) return;
  if (typeof value !== "object") return;
  if (Array.isArray(value)) {
    value.forEach((item, i) => assertNoForbiddenFields(item, `${path}[${i}]`));
    return;
  }
  for (const [key, child] of Object.entries(value as Record<string, unknown>)) {
    if (FORBIDDEN_DURABLE_MEMORY_FIELDS.includes(key)) {
      throw new Error(
        `sarah_nostr_memory: forbidden durable field ${path}.${key}`,
      );
    }
    assertNoForbiddenFields(child, `${path}.${key}`);
  }
};

export const buildCoreBody = (profile: string): SarahEngramCoreBody => {
  if (typeof profile !== "string" || profile.length === 0) {
    throw new Error("sarah_nostr_memory: core profile must be non-empty string");
  }
  assertSarahMemoryValueStorable(profile);
  return { slug: "core", profile };
};

export const buildMemoryBody = (input: {
  readonly slug: string;
  readonly value: string;
  readonly openagents: Omit<Companion, "schema" | "contentDigest"> & {
    readonly contentDigest?: Companion["contentDigest"];
  };
}): SarahEngramMemoryBody => {
  if (!isMemorySlug(input.slug)) {
    throw new Error(`sarah_nostr_memory: invalid memory slug: ${input.slug}`);
  }
  assertSarahMemoryValueStorable(input.value);
  const digest = input.openagents.contentDigest ?? contentDigestOf(input.value);
  if (digest !== contentDigestOf(input.value)) {
    throw new Error("sarah_nostr_memory: contentDigest must match value");
  }
  const openagents = decodeCompanion({
    schema: SARAH_NIP_AE_COMPANION_SCHEMA,
    admission: input.openagents.admission,
    entityId: input.openagents.entityId,
    contentDigest: digest,
    sourceEventRefs: input.openagents.sourceEventRefs,
    relations: input.openagents.relations,
    derivedFromSlugs: input.openagents.derivedFromSlugs,
  });
  if (
    openagents.admission === "admitted" &&
    openagents.sourceEventRefs.length < 1
  ) {
    throw new Error(
      "sarah_nostr_memory: admitted memory requires at least one sourceEventRef",
    );
  }
  for (const rel of openagents.relations) {
    if (!isValidSlug(rel.targetSlug)) {
      throw new Error(
        `sarah_nostr_memory: invalid relation targetSlug: ${rel.targetSlug}`,
      );
    }
  }
  for (const s of openagents.derivedFromSlugs) {
    if (!isValidSlug(s)) {
      throw new Error(`sarah_nostr_memory: invalid derivedFrom slug: ${s}`);
    }
  }
  const body: SarahEngramMemoryBody = {
    slug: input.slug,
    value: input.value,
    openagents,
  };
  assertNoForbiddenFields(body);
  return body;
};

export const buildTombstoneBody = (slug: string): SarahEngramMemoryBody => {
  if (!isMemorySlug(slug)) {
    throw new Error(`sarah_nostr_memory: invalid memory slug: ${slug}`);
  }
  return { slug, value: null };
};

/**
 * Serialize engram body. Companion fields ride under the unknown-fields rule.
 * Core stays pure NIP-AE `{slug, profile}`.
 */
export const serializeEngramBody = (body: SarahEngramBody): string => {
  assertNoForbiddenFields(body);
  if (body.slug === CORE_SLUG) {
    return JSON.stringify({
      slug: "core",
      profile: (body as SarahEngramCoreBody).profile,
    });
  }
  const mem = body as SarahEngramMemoryBody;
  if (mem.value === null) {
    return JSON.stringify({ slug: mem.slug, value: null });
  }
  if (mem.openagents === undefined) {
    return JSON.stringify({ slug: mem.slug, value: mem.value });
  }
  return JSON.stringify({
    slug: mem.slug,
    value: mem.value,
    openagents: mem.openagents,
  });
};

/**
 * Parse decrypted engram plaintext. Unknown top-level keys beyond
 * slug/value/profile/openagents are ignored for NIP-AE head validity, but
 * forbidden ranking fields still fail closed for Sarah writes.
 */
export const parseEngramBody = (plaintext: string): SarahEngramBody | null => {
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
  try {
    assertNoForbiddenFields(obj);
  } catch {
    return null;
  }
  const slug = obj["slug"];
  if (typeof slug !== "string" || !isValidSlug(slug)) return null;

  if (slug === CORE_SLUG) {
    const profile = obj["profile"];
    if (typeof profile !== "string") return null;
    return { slug: "core", profile };
  }

  if (!("value" in obj)) return null;
  const value = obj["value"];
  if (value !== null && typeof value !== "string") return null;

  if (value === null) {
    return { slug, value: null };
  }

  const oa = obj["openagents"];
  if (oa === undefined) {
    return { slug, value };
  }
  try {
    const openagents = decodeCompanion(oa);
    return { slug, value, openagents };
  } catch {
    return null;
  }
};

export const buildEngramWriteTemplate = (input: {
  readonly conversation: SarahEngramConversation;
  readonly body: SarahEngramBody;
  readonly cipher: SarahNostrMemoryCipher;
  readonly createdAt?: number;
  readonly alt?: string;
}): {
  readonly template: SarahNostrEventTemplate;
  readonly d: string;
  readonly plaintext: string;
  readonly slug: string;
} => {
  if (
    !/^[0-9a-f]{64}$/.test(input.conversation.ownerPubkey) ||
    !/^[0-9a-f]{64}$/.test(input.conversation.sarahPubkey)
  ) {
    throw new Error("sarah_nostr_memory: pubkeys must be 64 lowercase hex");
  }
  const slug = input.body.slug;
  if (!isValidSlug(slug)) {
    throw new Error(`sarah_nostr_memory: invalid slug: ${slug}`);
  }
  if (slug !== CORE_SLUG && (input.body as SarahEngramMemoryBody).value !== null) {
    const mem = input.body as SarahEngramMemoryBody;
    if (typeof mem.value === "string") {
      assertSarahMemoryValueStorable(mem.value);
    }
  }

  const plaintext = serializeEngramBody(input.body);
  if (utf8.encode(plaintext).length > MAX_ENGRAM_PLAINTEXT_BYTES) {
    throw new Error("sarah_nostr_memory: plaintext exceeds 65535 bytes");
  }

  const ciphertext = input.cipher.encryptToOwner(plaintext);
  if (!ciphertext || ciphertext.trim() === "") {
    throw new Error("sarah_nostr_memory: cipher returned empty content");
  }
  // Fail closed if a broken cipher echoed plaintext JSON.
  if (
    ciphertext.includes('"slug":') ||
    ciphertext.includes(SARAH_NIP_AE_COMPANION_SCHEMA)
  ) {
    throw new Error("sarah_nostr_memory: cipher must not return plaintext body");
  }

  const d = deriveEngramDTag(input.conversation.conversationKeyHex, slug);
  const alt = input.alt ?? ENGRAM_ALT;
  const tags: string[][] = [
    ["d", d],
    ["p", input.conversation.ownerPubkey],
    ["alt", alt],
  ];

  const template: SarahNostrEventTemplate = {
    kind: SARAH_ENGRAM_KIND,
    created_at: input.createdAt ?? Math.floor(Date.now() / 1000),
    tags,
    content: ciphertext,
  };
  assertSarahNostrPublicSafe(template);

  return { template, d, plaintext, slug };
};

/** Decrypt wire content and parse the engram body (owner or agent side). */
export const readEngramBody = (input: {
  readonly content: string;
  readonly cipher: SarahNostrMemoryCipher;
}): SarahEngramBody | null => {
  const plaintext = input.cipher.decryptFromOwner(input.content);
  return parseEngramBody(plaintext);
};

/** Addressable coordinate `30174:<agent-pubkey>:<d>`. */
export const engramAddress = (agentPubkey: string, d: string): string =>
  `${SARAH_ENGRAM_KIND}:${agentPubkey}:${d}`;

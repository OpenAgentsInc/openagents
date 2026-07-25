import { sha256 } from "@noble/hashes/sha256";
import { bytesToHex, utf8ToBytes } from "@noble/hashes/utils";
import { Schema as S } from "effect";

import { assertSarahNostrPublicSafe } from "../nostr-identity/redaction.ts";
import { resolveSarahConversationIdentity } from "./mapping.ts";
import {
  SARAH_NOSTR_MIGRATION_MANIFEST_SCHEMA,
  SarahNostrMigrationManifest,
  type SarahNostrMigrationStage,
} from "./types.ts";

const decodeManifest = S.decodeUnknownSync(SarahNostrMigrationManifest);

const EVENT_ID_RE = /^[0-9a-f]{64}$/;

export const computeEventIdDigestChain = (
  eventIds: ReadonlyArray<string>,
): string => {
  for (const id of eventIds) {
    if (!EVENT_ID_RE.test(id)) {
      throw new Error(
        `sarah_nostr_migration: event id must be 64 lowercase hex, got ${id.slice(0, 16)}…`,
      );
    }
  }
  // @noble/hashes rather than node:crypto: this module is reachable from the
  // OpenAgents mobile bundle through the package barrel, and a node: import
  // there fails at runtime on device while passing every Node-hosted test.
  return bytesToHex(sha256(utf8ToBytes(eventIds.join("\n"))));
};

/**
 * Build a public-safe export/rollback manifest for a conversation.
 * Event ids only — never content, ciphertext, prompts, or credentials.
 * Idempotent: same inputs yield the same digestChain.
 */
export const buildSarahNostrMigrationManifest = (input: {
  readonly stage: SarahNostrMigrationStage;
  readonly threadRefOrConversation: string;
  readonly eventIds: ReadonlyArray<string>;
  readonly exportedAt?: string;
  readonly rollbackWindowClosesAt?: string;
  readonly note?: string;
}): SarahNostrMigrationManifest => {
  const identity = resolveSarahConversationIdentity(
    input.threadRefOrConversation,
  );
  const eventIds = [...input.eventIds];
  const digestChain = computeEventIdDigestChain(eventIds);
  const manifest = decodeManifest({
    schema: SARAH_NOSTR_MIGRATION_MANIFEST_SCHEMA,
    stage: input.stage,
    conversation: identity.conversation,
    threadRef: identity.threadRef,
    digest: identity.digest,
    exportedAt: input.exportedAt ?? new Date().toISOString(),
    eventCount: eventIds.length,
    eventIds,
    digestChain,
    ...(input.rollbackWindowClosesAt !== undefined
      ? { rollbackWindowClosesAt: input.rollbackWindowClosesAt }
      : {}),
    ...(input.note !== undefined ? { note: input.note } : {}),
  });
  assertSarahNostrPublicSafe(manifest);
  return manifest;
};

/**
 * Validate a rollback target against a previously exported manifest.
 *
 * Rules:
 * - manifest must decode
 * - digestChain must match recomputed chain
 * - target stage must be a legal rollback from the manifest stage
 *   (retirement→cutover, cutover→shadow)
 * - does not delete Cloud SQL rows; callers keep rows during the window
 */
export const validateSarahNostrMigrationRollback = (input: {
  readonly manifest: unknown;
  readonly targetStage: SarahNostrMigrationStage;
  readonly nowIso?: string;
}): {
  readonly ok: true;
  readonly manifest: SarahNostrMigrationManifest;
  readonly targetStage: SarahNostrMigrationStage;
} | {
  readonly ok: false;
  readonly reason: string;
} => {
  let manifest: SarahNostrMigrationManifest;
  try {
    manifest = decodeManifest(input.manifest);
    assertSarahNostrPublicSafe(manifest);
  } catch (error) {
    return {
      ok: false,
      reason:
        error instanceof Error
          ? `invalid_manifest: ${error.message}`
          : "invalid_manifest",
    };
  }

  const recomputed = computeEventIdDigestChain(manifest.eventIds);
  if (recomputed !== manifest.digestChain) {
    return { ok: false, reason: "digest_chain_mismatch" };
  }

  if (manifest.eventCount !== manifest.eventIds.length) {
    return { ok: false, reason: "event_count_mismatch" };
  }

  const legalRollback: Record<
    SarahNostrMigrationStage,
    SarahNostrMigrationStage | null
  > = {
    shadow: null,
    cutover: "shadow",
    retirement: "cutover",
  };
  const expected = legalRollback[manifest.stage];
  if (expected === null) {
    return {
      ok: false,
      reason: "rollback_not_applicable_at_shadow",
    };
  }
  if (input.targetStage !== expected) {
    return {
      ok: false,
      reason: `illegal_rollback_target: expected ${expected}, got ${input.targetStage}`,
    };
  }

  if (manifest.rollbackWindowClosesAt !== undefined) {
    const now = input.nowIso ?? new Date().toISOString();
    if (now > manifest.rollbackWindowClosesAt) {
      return { ok: false, reason: "rollback_window_closed" };
    }
  }

  return {
    ok: true,
    manifest,
    targetStage: input.targetStage,
  };
};

/**
 * Serialize a manifest to JSON. Asserts public-safe before stringify.
 */
export const serializeSarahNostrMigrationManifest = (
  manifest: SarahNostrMigrationManifest,
): string => {
  assertSarahNostrPublicSafe(manifest);
  return JSON.stringify(manifest);
};

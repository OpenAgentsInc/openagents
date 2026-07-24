/**
 * Sarah Nostr memory runtime adapters — SARAH-NR-07.
 *
 * NIP-AE engrams (kind 30174), NIP-RS read state (kind 30078), NIP-ER
 * reminders (kind 30300). Injectable cipher port; no live nostr-effect pin.
 *
 * @see docs/omega/2026-07-24-sarah-memory-runtime.md
 * @see docs/sarah/2026-07-24-sarah-memory-on-nostr-audit.md
 */

export {
  makeNip44MemoryCipher,
  testSarahNostrMemoryCipher,
} from "./cipher.ts";
export {
  buildCoreBody,
  buildEngramWriteTemplate,
  buildMemoryBody,
  buildTombstoneBody,
  contentDigestOf,
  deriveEngramDTag,
  engramAddress,
  isCoreSlug,
  isMemorySlug,
  isValidSlug,
  parseEngramBody,
  readEngramBody,
  serializeEngramBody,
} from "./engram.ts";
export {
  assertSarahMemoryValueStorable,
  guardSarahMemoryValue,
  type SarahMemoryValueVerdict,
} from "./redaction.ts";
export {
  advanceReadContexts,
  buildReadStateBlob,
  buildReadStateDTag,
  buildReadStateWriteTemplate,
  mergeReadContexts,
  msgContextKey,
  parseReadStateSlotId,
  readReadStateBlob,
  sarahConversationContextKey,
  serializeReadStateBlob,
  threadContextKey,
  validateReadStateBlob,
} from "./read-state.ts";
export {
  buildReminderContent,
  buildReminderWriteTemplate,
  generateReminderId,
  getReminderD,
  getReminderNotBefore,
  parseNotBefore,
  parseReminderContent,
  readReminderContent,
  reminderAddress,
  serializeReminderContent,
} from "./reminder.ts";
export {
  CORE_SLUG,
  ENGRAM_ALT,
  ENGRAM_D_TAG_DOMAIN,
  FORBIDDEN_DURABLE_MEMORY_FIELDS,
  MAX_CLIENT_ID_LENGTH,
  MAX_CONTEXT_ENTRIES,
  MAX_CONTEXT_ID_BYTES,
  MAX_CONTEXT_TIMESTAMP,
  MAX_ENGRAM_PLAINTEXT_BYTES,
  MAX_SLUG_BYTES,
  MEMORY_SLUG_PATTERN,
  READ_STATE_ALT,
  READ_STATE_D_PREFIX,
  READ_STATE_T_VALUE,
  READ_STATE_VERSION,
  REMINDER_ALT,
  SARAH_DELETION_KIND,
  SARAH_ENGRAM_KIND,
  SARAH_NIP_AE_COMPANION_EXTENSION,
  SARAH_NIP_AE_COMPANION_SCHEMA,
  SARAH_READ_STATE_KIND,
  SARAH_REMINDER_KIND,
  SarahMemoryAdmission,
  SarahMemoryRelation,
  SarahMemoryRelationDirection,
  SarahMemorySourceEventRef,
  SarahMemorySourceRole,
  SarahNipAeCompanion,
  SarahReminderStatus,
  type SarahEngramBody,
  type SarahEngramConversation,
  type SarahEngramCoreBody,
  type SarahEngramMemoryBody,
  type SarahNostrMemoryCipher,
  type SarahReadContexts,
  type SarahReadStateBlob,
  type SarahReminderContent,
  type SarahReminderTarget,
  type SarahNipAeCompanion as SarahNipAeCompanionType,
  type SarahReminderStatus as SarahReminderStatusType,
  type SarahMemoryAdmission as SarahMemoryAdmissionType,
} from "./types.ts";

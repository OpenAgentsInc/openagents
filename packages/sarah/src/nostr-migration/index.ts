/**
 * Sarah Nostr migration and cutover — SARAH-NR-08
 *
 * Stage machine (shadow | cutover | retirement), thread ↔ conversation
 * mapping, public-safe drift comparison, export/rollback manifests, and
 * SARAH_NOSTR_RECORD_MODE resolution (with legacy SARAH_NOSTR_SHADOW_PUBLISH).
 *
 * @see docs/omega/2026-07-24-sarah-nostr-cutover.md
 */

export {
  buildSarahNostrMigrationManifest,
  computeEventIdDigestChain,
  serializeSarahNostrMigrationManifest,
  validateSarahNostrMigrationRollback,
} from "./export.ts";
export {
  compareKhalaAndNostrDurableEvents,
  projectNostrDurableEventForDrift,
} from "./drift.ts";
export {
  conversationTagFromThreadRef,
  extractSarahDigest,
  isSarahConversationTag,
  isSarahThreadRef,
  resolveSarahConversationIdentity,
  threadRefFromConversationTag,
} from "./mapping.ts";
export {
  isSarahNostrRecordMode,
  khalaRemainsRecordAuthority,
  nostrIsRecordAuthority,
  resolveImpliedMigrationStage,
  resolveSarahNostrRecordMode,
  shouldPublishSarahNostrFromMode,
} from "./mode.ts";
export {
  SARAH_NOSTR_MIGRATION_STAGES,
  SarahNostrMigrationStageError,
  SarahNostrMigrationStageMachine,
  canTransitionSarahNostrMigrationStage,
  defaultStageForRecordMode,
  isSarahNostrMigrationStage,
} from "./stages.ts";
export {
  SARAH_NOSTR_MIGRATION_MANIFEST_SCHEMA,
  SARAH_NOSTR_RECORD_MODE_ENV,
  SARAH_NOSTR_SHADOW_PUBLISH_ENV,
  SarahNostrMigrationManifest,
  SarahNostrMigrationStage,
  SarahNostrRecordMode,
  type DriftItem,
  type DriftMismatchKind,
  type DriftReport,
  type KhalaShapedEvent,
  type NostrDurableEventProjection,
  type SarahNostrMigrationManifest as SarahNostrMigrationManifestValue,
  type SarahNostrMigrationStage as SarahNostrMigrationStageValue,
  type SarahNostrRecordMode as SarahNostrRecordModeValue,
} from "./types.ts";

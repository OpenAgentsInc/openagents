import { Schema as S } from "effect";

import { FactRef, MemoryTimestamp, OwnerScopeId, PatternRef, ProjectScopeId, Sha256Hex } from "./refs.js";

/**
 * The distilled global-pattern layer — the only genuinely new surface in AFS-10.
 *
 * A pattern names a recurring phenomenon distilled offline from the owner's own
 * consented per-case records. It carries supporting success and failure
 * references and an applicability bound, so recall returns a bounded slice
 * rather than the whole per-case bank (the MemoHarness global layer). A pattern
 * is derived from redacted per-case text only; it must carry no owner-private
 * raw content, and it inherits no access to the private cases that supported it.
 * It stays inside one owner scope; a cross-owner or cross-project pattern needs
 * a separate explicit scope and authority.
 */
export const GLOBAL_PATTERN_SCHEMA_LITERAL = "openagents.experience_pattern.v1" as const;

export const MAX_PATTERN_TEXT_CHARS = 1000;

const patternText = S.String.check(S.isMinLength(1), S.isMaxLength(MAX_PATTERN_TEXT_CHARS));

export const GlobalPattern = S.Struct({
  schema: S.Literal(GLOBAL_PATTERN_SCHEMA_LITERAL),
  patternRef: PatternRef,
  ownerScope: OwnerScopeId,
  projectScope: ProjectScopeId,
  /** The redacted phenomenon description. */
  phenomenon: patternText,
  /** The redacted applicability bound: when the pattern is expected to apply. */
  applicability: patternText,
  /** The redacted expected effect of acting on the pattern. */
  expectedEffect: patternText,
  supportSuccessRefs: S.Array(FactRef),
  supportFailureRefs: S.Array(FactRef),
  confidence: S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1)),
  observedAt: MemoryTimestamp,
  /** The content address of the redacted pattern text. */
  digest: Sha256Hex,
});
export type GlobalPattern = typeof GlobalPattern.Type;

/** Trusted decoder for the distillation path (redaction happens before this). */
export const decodeGlobalPattern = S.decodeUnknownSync(GlobalPattern);

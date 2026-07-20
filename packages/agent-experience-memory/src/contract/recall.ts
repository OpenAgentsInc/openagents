import { Schema as S } from "effect";

import { FactRef, PatternRef, Sha256Hex } from "./refs.js";

/**
 * The result of a single, one-shot pre-turn recall.
 *
 * A recall is bounded and auditable. It names the frozen bank it read
 * (`bankDigest`), the exact adaptation it produced (`effectiveAdaptationDigest`,
 * bound to the turn), which records and patterns it included and dropped, the
 * token budget it used, and the bounded, already-redacted `memoryBlock` a host
 * may prepend to a prompt. An empty result is the default: with memory OFF, or
 * an empty or corrupt bank, `memoryBlock` is the empty string and the host
 * prompt is unchanged.
 */
export const RECALL_RESULT_SCHEMA_LITERAL = "openagents.experience_recall.v1" as const;

export const RecallResult = S.Struct({
  schema: S.Literal(RECALL_RESULT_SCHEMA_LITERAL),
  enabled: S.Boolean,
  bankDigest: Sha256Hex,
  /** SHA-256 over the frozen bank digest and the request features. Bound to the turn. */
  effectiveAdaptationDigest: Sha256Hex,
  includedRecordRefs: S.Array(FactRef),
  droppedRecordRefs: S.Array(FactRef),
  includedPatternRefs: S.Array(PatternRef),
  usedTokens: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  /** The bounded, redacted slice a host may inject. Empty when memory added nothing. */
  memoryBlock: S.String.check(S.isMaxLength(4000)),
});
export type RecallResult = typeof RecallResult.Type;

export const decodeRecallResult = S.decodeUnknownSync(RecallResult);

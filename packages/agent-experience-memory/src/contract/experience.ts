import { Schema as S } from "effect";

import { canonicalStringify } from "../internal/canonical.js";
import { sha256Hex } from "../internal/sha256.js";
import {
  FactRef,
  MemoryConsent,
  MemoryTimestamp,
  OwnerScopeId,
  ProjectScopeId,
  RepoRef,
  Sha256Hex,
  TraceRef,
} from "./refs.js";

/**
 * The per-case experience record — the private per-execution layer.
 *
 * This mirrors the reviewed shape of the unwired Pylon TAS `repo-memory.ts` and
 * `session-memory.ts` records, but adds the schema, owner scope, project scope,
 * consent, redaction digest, and trace reference that those files never had. A
 * record stores only a bounded, already-redacted fact plus references. It never
 * stores a raw prompt, a raw trajectory, a secret, a local path, or wallet
 * material; the trace and its detail stay behind a `traceRef`, never inlined.
 */
export const EXPERIENCE_RECORD_SCHEMA_LITERAL = "openagents.experience_record.v1" as const;

/** The bounded fact kinds. The first four match the reviewed TAS `RepoMemoryKind`. */
export const ExperienceKind = S.Literals(["convention", "layout", "command", "note", "outcome"]);
export type ExperienceKind = typeof ExperienceKind.Type;

/** The maximum length of a stored redacted fact. Facts are short by design. */
export const MAX_EXPERIENCE_TEXT_CHARS = 1000;

/** An optional coarse six-dimension diagnosis, matching the MemoHarness vocabulary. */
export const SixDimensionDiagnosis = S.Struct({
  contextAssembly: S.Number.check(S.isGreaterThanOrEqualTo(-1), S.isLessThanOrEqualTo(1)),
  toolInteraction: S.Number.check(S.isGreaterThanOrEqualTo(-1), S.isLessThanOrEqualTo(1)),
  generationControl: S.Number.check(S.isGreaterThanOrEqualTo(-1), S.isLessThanOrEqualTo(1)),
  orchestration: S.Number.check(S.isGreaterThanOrEqualTo(-1), S.isLessThanOrEqualTo(1)),
  memoryManagement: S.Number.check(S.isGreaterThanOrEqualTo(-1), S.isLessThanOrEqualTo(1)),
  outputProcessing: S.Number.check(S.isGreaterThanOrEqualTo(-1), S.isLessThanOrEqualTo(1)),
});
export type SixDimensionDiagnosis = typeof SixDimensionDiagnosis.Type;

export const ExperienceRecord = S.Struct({
  schema: S.Literal(EXPERIENCE_RECORD_SCHEMA_LITERAL),
  recordRef: FactRef,
  ownerScope: OwnerScopeId,
  projectScope: ProjectScopeId,
  repoRef: RepoRef,
  kind: ExperienceKind,
  /** A bounded, already-redacted fact. The redaction guard produced this text. */
  text: S.String.check(S.isMinLength(1), S.isMaxLength(MAX_EXPERIENCE_TEXT_CHARS)),
  confidence: S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1)),
  consent: MemoryConsent,
  observedAt: MemoryTimestamp,
  /** A reference to a redacted ATIF trace, or null. Never inlined trajectory content. */
  traceRef: S.NullOr(TraceRef),
  /** The content address of the redacted text, so a drift is detectable. */
  digest: Sha256Hex,
  /** An optional local embedding for cosine recall. Never uploaded on the Apple FM path. */
  embedding: S.optionalKey(S.Array(S.Number)),
  diagnosis: S.optionalKey(SixDimensionDiagnosis),
});
export type ExperienceRecord = typeof ExperienceRecord.Type;

/** Trusted decoder for the build path (redaction happens before this). */
export const decodeExperienceRecord = S.decodeUnknownSync(ExperienceRecord);

/** The content address of a record's redacted text. */
export const experienceTextDigest = (text: string): Sha256Hex =>
  S.decodeUnknownSync(Sha256Hex)(sha256Hex(canonicalStringify({ text })));

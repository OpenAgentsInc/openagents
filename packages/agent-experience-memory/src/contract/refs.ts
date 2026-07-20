import { Schema as S } from "effect";

/**
 * Branded references and shared scalars for the owner-local experience memory.
 *
 * These are portable Effect schemas. Every reference is bounded and
 * pattern-checked. An owner scope and a project scope are separate brands so a
 * type error, not a convention, stops a cross-owner or cross-project mix.
 */

const memoryRef = <const Brand extends string>(brand: Brand) =>
  S.String.check(
    S.isMinLength(1),
    S.isMaxLength(256),
    S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:/-]*$/),
  ).pipe(S.brand(brand));

/** ISO-8601 UTC timestamp, matching the frozen turn timestamp shape. */
export const MemoryTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/),
);
export type MemoryTimestamp = typeof MemoryTimestamp.Type;

/** A 64-character lowercase-hex SHA-256 digest. */
export const Sha256Hex = S.String.check(S.isPattern(/^[a-f0-9]{64}$/));
export type Sha256Hex = typeof Sha256Hex.Type;

/** The owner boundary. One owner scope must never read another owner's memory. */
export const OwnerScopeId = memoryRef("MemoryOwnerScopeId");
export type OwnerScopeId = typeof OwnerScopeId.Type;

/** The project boundary. Recall stays inside one project without a separate scope grant. */
export const ProjectScopeId = memoryRef("MemoryProjectScopeId");
export type ProjectScopeId = typeof ProjectScopeId.Type;

export const RepoRef = memoryRef("MemoryRepoRef");
export type RepoRef = typeof RepoRef.Type;

/** The stable identity of one per-case experience fact. */
export const FactRef = memoryRef("MemoryFactRef");
export type FactRef = typeof FactRef.Type;

/** The stable identity of one distilled global pattern (the genuinely new layer). */
export const PatternRef = memoryRef("MemoryPatternRef");
export type PatternRef = typeof PatternRef.Type;

/** A frozen eligible bank identity. */
export const BankId = memoryRef("MemoryBankId");
export type BankId = typeof BankId.Type;

/** A reference to an existing redacted ATIF trace, never raw trajectory content. */
export const TraceRef = memoryRef("MemoryTraceRef");
export type TraceRef = typeof TraceRef.Type;

/** Consent for reuse of a record. It defaults to withheld, matching the trace store. */
export const MemoryConsent = S.Literals(["granted", "withheld"]);
export type MemoryConsent = typeof MemoryConsent.Type;

/** Trusted constructors for scripts, tests, and derivation paths. */
export const ownerScopeId = S.decodeUnknownSync(OwnerScopeId);
export const projectScopeId = S.decodeUnknownSync(ProjectScopeId);
export const repoRef = S.decodeUnknownSync(RepoRef);
export const factRef = S.decodeUnknownSync(FactRef);
export const patternRef = S.decodeUnknownSync(PatternRef);
export const bankId = S.decodeUnknownSync(BankId);
export const traceRef = S.decodeUnknownSync(TraceRef);

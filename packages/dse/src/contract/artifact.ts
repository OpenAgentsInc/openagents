import { Schema as S } from "effect";
import {
  ArtifactRef,
  ReleasedArtifact,
  type ReleasedArtifact as ReleasedArtifactType,
} from "@openagentsinc/agent-runtime-schema";

import { canonicalStringify } from "../internal/canonical.js";
import { sha256Hex } from "../internal/sha256.js";
import {
  CandidateId,
  DatasetRevisionId,
  DseTimestamp,
  DseUsageTruth,
  PromotionId,
  ReceiptId,
  Sha256Hex,
  SignatureId,
} from "./refs.js";
import { PromptIr } from "./signature.js";
import { SearchPlan } from "./budget.js";

/**
 * Compiled programs, immutable candidate artifacts, released pointers, rollback
 * receipts, and predict receipts.
 *
 * A `CandidateArtifact` is content-addressed by a digest over ALL its bytes (the
 * audit correction that a compiled ID must cover the complete artifact, not only
 * a parameter hash). A `ReleasedArtifactPointer` binds a promoted candidate to
 * the frozen `ReleasedArtifact` shape from `agent-runtime-schema`, so a released
 * DSE artifact resolves through the same runtime contract that serves every
 * released turn artifact.
 */

export const COMPILED_PROGRAM_SCHEMA_LITERAL = "openagents.dse.compiled_program.v1" as const;
export const CANDIDATE_ARTIFACT_SCHEMA_LITERAL = "openagents.dse.candidate_artifact.v1" as const;
export const RELEASED_POINTER_SCHEMA_LITERAL =
  "openagents.dse.released_artifact_pointer.v1" as const;
export const ROLLBACK_RECEIPT_SCHEMA_LITERAL = "openagents.dse.rollback_receipt.v1" as const;
export const PREDICT_RECEIPT_SCHEMA_LITERAL = "openagents.dse.predict_receipt.v1" as const;

/** The bounded decode policy carried in a compiled program. */
export const DecodePolicy = S.Struct({
  maxRepairs: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(3)),
  maxOutputChars: S.Number.check(
    S.isInt(),
    S.isGreaterThanOrEqualTo(1),
    S.isLessThanOrEqualTo(20000),
  ),
});
export type DecodePolicy = typeof DecodePolicy.Type;

/** The serialized compiled program: the searched prompt, decode policy, and role. */
export const CompiledProgram = S.Struct({
  schema: S.Literal(COMPILED_PROGRAM_SCHEMA_LITERAL),
  signatureId: SignatureId,
  promptIr: PromptIr,
  decodePolicy: DecodePolicy,
  modelRole: S.String.check(S.isMinLength(1), S.isMaxLength(128)),
});
export type CompiledProgram = typeof CompiledProgram.Type;

/** An immutable compiled candidate, content-addressed by a digest over all bytes. */
export const CandidateArtifact = S.Struct({
  schema: S.Literal(CANDIDATE_ARTIFACT_SCHEMA_LITERAL),
  candidateId: CandidateId,
  signatureId: SignatureId,
  datasetRevisionId: DatasetRevisionId,
  searchPlan: SearchPlan,
  program: CompiledProgram,
  producedAt: DseTimestamp,
  digest: Sha256Hex,
});
export type CandidateArtifact = typeof CandidateArtifact.Type;

/**
 * A released, resolvable pointer. `released` is the frozen `agent-runtime-schema`
 * `ReleasedArtifact` (kind `prompt_program`). Its `digest` equals the candidate
 * digest and its `rollbackOf` names the prior released artifact when this release
 * supersedes one. The pointer requires a `promotionId`, so an unreviewed
 * candidate can never masquerade as released.
 */
export const ReleasedArtifactPointer = S.Struct({
  schema: S.Literal(RELEASED_POINTER_SCHEMA_LITERAL),
  signatureId: SignatureId,
  candidateId: CandidateId,
  promotionId: PromotionId,
  released: ReleasedArtifact,
  evaluationReportDigest: Sha256Hex,
  releasedAt: DseTimestamp,
});
export type ReleasedArtifactPointer = typeof ReleasedArtifactPointer.Type;

/** A rollback receipt records restoring a prior released artifact. */
export const RollbackReceipt = S.Struct({
  schema: S.Literal(ROLLBACK_RECEIPT_SCHEMA_LITERAL),
  signatureId: SignatureId,
  fromCandidateId: CandidateId,
  toCandidateId: CandidateId,
  restoredArtifactRef: ArtifactRef,
  reason: S.String.check(S.isMinLength(1), S.isMaxLength(2000)),
  rolledBackAt: DseTimestamp,
});
export type RollbackReceipt = typeof RollbackReceipt.Type;

/** An append-only predict receipt. It is evidence, never release authority. */
export const PredictReceipt = S.Struct({
  schema: S.Literal(PREDICT_RECEIPT_SCHEMA_LITERAL),
  receiptId: ReceiptId,
  signatureId: SignatureId,
  candidateId: CandidateId,
  promptDigest: Sha256Hex,
  outputDigest: Sha256Hex,
  decodeOutcome: S.Literals(["decoded", "repaired", "failed"]),
  repairCount: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(3)),
  usageTruth: DseUsageTruth,
  outputChars: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  observedAt: DseTimestamp,
});
export type PredictReceipt = typeof PredictReceipt.Type;

const decodeCandidateArtifact = S.decodeUnknownSync(CandidateArtifact);
const encodeProgram = S.encodeUnknownSync(CompiledProgram);
const encodeSearchPlan = S.encodeUnknownSync(SearchPlan);

/**
 * Build an immutable candidate artifact whose digest covers all of its bytes.
 * The digest is computed over the canonical serialization of every field except
 * `candidateId` and `digest`, then the content-addressed identity is derived
 * from that digest. Recomputing the digest over the same fields reproduces the
 * same identity, which is what makes offline verification exact.
 */
export const makeCandidateArtifact = (args: {
  readonly signatureId: typeof SignatureId.Type;
  readonly datasetRevisionId: typeof DatasetRevisionId.Type;
  readonly searchPlan: typeof SearchPlan.Type;
  readonly program: CompiledProgram;
  readonly producedAt: typeof DseTimestamp.Type;
}): CandidateArtifact => {
  const covered = {
    schema: CANDIDATE_ARTIFACT_SCHEMA_LITERAL,
    signatureId: args.signatureId,
    datasetRevisionId: args.datasetRevisionId,
    searchPlan: encodeSearchPlan(args.searchPlan),
    program: encodeProgram(args.program),
    producedAt: args.producedAt,
  };
  const digest = sha256Hex(canonicalStringify(covered));
  return decodeCandidateArtifact({
    ...covered,
    candidateId: `cand:${digest}`,
    digest,
  });
};

/** Recompute the digest that binds a candidate artifact's bytes. */
export const candidateArtifactDigest = (artifact: CandidateArtifact): string => {
  const covered = {
    schema: CANDIDATE_ARTIFACT_SCHEMA_LITERAL,
    signatureId: artifact.signatureId,
    datasetRevisionId: artifact.datasetRevisionId,
    searchPlan: encodeSearchPlan(artifact.searchPlan),
    program: encodeProgram(artifact.program),
    producedAt: artifact.producedAt,
  };
  return sha256Hex(canonicalStringify(covered));
};

/** The frozen released-artifact reference for a candidate digest (slash-free). */
export const releasedArtifactRefFor = (
  signatureId: typeof SignatureId.Type,
  digest: string,
): typeof ArtifactRef.Type => {
  const slug = signatureId.replaceAll("/", ".");
  return S.decodeUnknownSync(ArtifactRef)(`artifact:${slug}:${digest.slice(0, 16)}`);
};

export type { ReleasedArtifactType };

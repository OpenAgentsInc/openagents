import { Schema as S } from "effect";

import { brandedTurnRef, ProviderTurnRef, TurnRequestRef, TurnTimestamp, TurnUsageTruth } from "./turn.js";
import { CandidateRef } from "./provider.js";

/**
 * AFS-00 frozen artifact and receipt contract.
 *
 * This module freezes the released-artifact reference, the evidence reference,
 * and the turn receipt. Runtime code can resolve only a released immutable
 * artifact. A local model, provider, renderer, or turn service can never
 * publish, promote, or replace an artifact.
 *
 * Compatibility rules are the shared AFS-00 rules recorded in `turn.ts`.
 */
export const ARTIFACT_SCHEMA_LITERAL = "openagents.agent_turn_artifact.v1" as const;
export const RECEIPT_SCHEMA_LITERAL = "openagents.agent_turn_receipt.v1" as const;

/**
 * An artifact reference names one immutable released artifact. The reference
 * covers all artifact bytes. It is content-addressed by digest.
 */
export const ArtifactRef = brandedTurnRef("ArtifactRef");
export type ArtifactRef = typeof ArtifactRef.Type;

/** The 64-character lowercase hex digest that binds all artifact bytes. */
export const ArtifactDigest = S.String.check(S.isPattern(/^[a-f0-9]{64}$/));
export type ArtifactDigest = typeof ArtifactDigest.Type;

/**
 * A released, resolvable artifact pointer. Runtime code resolves it read-only.
 * A rollback pointer identifies a previously released artifact.
 */
export const ReleasedArtifact = S.Struct({
  schema: S.Literal(ARTIFACT_SCHEMA_LITERAL),
  artifactRef: ArtifactRef,
  digest: ArtifactDigest,
  kind: S.Literals(["policy_bundle", "prompt_program", "context_packing", "memory_bank"]),
  releasedAt: TurnTimestamp,
  rollbackOf: S.optionalKey(ArtifactRef),
});
export type ReleasedArtifact = typeof ReleasedArtifact.Type;

/** An evidence reference names one dereferenceable evidence record. */
export const EvidenceRef = brandedTurnRef("EvidenceRef");
export type EvidenceRef = typeof EvidenceRef.Type;

export const RouteDecisionRef = brandedTurnRef("RouteDecisionRef");
export type RouteDecisionRef = typeof RouteDecisionRef.Type;

/**
 * The decision recorded on a candidate. `accepted`, `rejected`, and `compared`
 * are candidate dispositions. `cancelled` and `failed` are terminal lifecycle
 * outcomes carried into the receipt.
 */
export const TurnReceiptDecision = S.Literals([
  "accepted",
  "rejected",
  "compared",
  "cancelled",
  "failed",
]);
export type TurnReceiptDecision = typeof TurnReceiptDecision.Type;

/**
 * The turn receipt binds the request, the route decision, the provider turn,
 * the candidate, the decision, the usage truth, and the evidence references. A
 * receipt is evidence. It is not release authority.
 */
export const TurnReceipt = S.Struct({
  schema: S.Literal(RECEIPT_SCHEMA_LITERAL),
  requestRef: TurnRequestRef,
  routeDecisionRef: RouteDecisionRef,
  providerTurnRef: S.optionalKey(ProviderTurnRef),
  candidateRef: S.optionalKey(CandidateRef),
  decision: TurnReceiptDecision,
  usageTruth: TurnUsageTruth,
  evidenceRefs: S.Array(EvidenceRef).check(S.isMaxLength(32)),
});
export type TurnReceipt = typeof TurnReceipt.Type;

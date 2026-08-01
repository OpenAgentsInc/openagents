import { Schema as S } from "effect";

import { forensicSha256Digest } from "./canonical.ts";
import {
  BoundedRefs,
  BoundedShortTexts,
  ClaimAdjudication,
  ClaimKind,
  EvidenceKind,
  EvidenceTier,
  ForensicRef,
  ForensicTimestamp,
  LongText,
  NonNegativeInteger,
  PositiveInteger,
  Sha256Digest,
  ShortText,
} from "./primitives.ts";

export const FORENSIC_FINDING_VERSION = "openagents.forensic_finding.v1" as const;
export const FORENSIC_HYPOTHESIS_VERSION = "openagents.forensic_hypothesis.v1" as const;
export const FORENSIC_CLAIM_RECORD_VERSION = "openagents.forensic_claim_record.v1" as const;
export const FORENSIC_CLAIM_REVISION_VERSION = "openagents.forensic_claim_revision.v1" as const;
export const FORENSIC_EVIDENCE_GRAPH_VERSION = "openagents.forensic_evidence_graph.v1" as const;

export const ClaimEvidenceSchema = S.Struct({
  kind: EvidenceKind,
  evidenceRef: ForensicRef,
  receiptDigest: Sha256Digest,
});
export interface ClaimEvidence extends S.Schema.Type<typeof ClaimEvidenceSchema> {}

export const REQUIRED_EVIDENCE_BY_CLAIM_KIND: Readonly<
  Record<ClaimKind, ReadonlyArray<EvidenceKind>>
> = {
  source_flaw: ["source_reference"],
  artifact_selected: ["preprocessed_source", "symbol_provider", "firmware_digest", "fault_build"],
  state_space_model: ["generator_vector", "assumption_model", "sensitivity_receipt"],
  owned_fixture_recovered: ["owned_fixture_authorization", "generator_trace", "wallet_match"],
  program_fingerprint: [
    "historical_chain_snapshot",
    "positive_control",
    "negative_control",
    "base_rate",
  ],
  entity_cluster: ["evidence_graph", "pooling_edge"],
  unauthorized_movement: ["victim_testimony", "transaction_reference"],
  identity_attribution: ["identity_evidence", "independent_review"],
};

const hasRequiredEvidence = (
  claimKind: ClaimKind,
  evidence: ReadonlyArray<ClaimEvidence>,
): boolean => {
  const observed = new Set(evidence.map((entry) => entry.kind));
  return REQUIRED_EVIDENCE_BY_CLAIM_KIND[claimKind].every((kind) => observed.has(kind));
};

export const ForensicClaimRecordSchema = S.Struct({
  schema: S.Literal(FORENSIC_CLAIM_RECORD_VERSION),
  claimRef: ForensicRef,
  runRef: ForensicRef,
  subjectRef: ForensicRef,
  claimKind: ClaimKind,
  proposition: LongText,
  evidenceTier: EvidenceTier,
  evidence: S.Array(ClaimEvidenceSchema).check(S.isMaxLength(256)),
  assumptions: BoundedShortTexts,
  nonImplications: BoundedShortTexts.check(S.isMinLength(1)),
  confidence: S.Literals(["low", "medium", "high", "decisive"]),
  authorRef: ForensicRef,
  adjudication: ClaimAdjudication,
  adjudicatorRef: S.optionalKey(ForensicRef),
  createdAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (claim) =>
          !["qualified", "verified", "retained"].includes(claim.adjudication) ||
          hasRequiredEvidence(claim.claimKind, claim.evidence),
        { message: "qualified claims require evidence specific to their own claim rung" },
      ),
      S.makeFilter(
        (claim) =>
          !["verified", "retained"].includes(claim.adjudication) ||
          (claim.evidenceTier === "independently_verified" && claim.adjudicatorRef !== undefined),
        {
          message:
            "verified or retained claims require independently verified evidence and an adjudicator",
        },
      ),
    ),
  )
  .annotate({ identifier: "ForensicClaimRecord" });
export interface ForensicClaimRecord extends S.Schema.Type<typeof ForensicClaimRecordSchema> {}

export const ForensicClaimRevisionSchema = S.Struct({
  schema: S.Literal(FORENSIC_CLAIM_REVISION_VERSION),
  revisionRef: ForensicRef,
  claimRef: ForensicRef,
  revision: PositiveInteger,
  priorClaimDigest: Sha256Digest,
  appendedEvidence: S.Array(ClaimEvidenceSchema).check(S.isMaxLength(256)),
  previousEvidenceTier: EvidenceTier,
  nextEvidenceTier: EvidenceTier,
  previousAdjudication: ClaimAdjudication,
  nextAdjudication: ClaimAdjudication,
  reason: LongText,
  reviewerRef: ForensicRef,
  affectedProjectionRefs: BoundedRefs,
  revisedAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (revision) =>
          revision.previousEvidenceTier !== revision.nextEvidenceTier ||
          revision.previousAdjudication !== revision.nextAdjudication ||
          revision.appendedEvidence.length > 0,
        { message: "claim revisions must append evidence or change tier or adjudication" },
      ),
    ),
  )
  .annotate({ identifier: "ForensicClaimRevision" });
export interface ForensicClaimRevision extends S.Schema.Type<typeof ForensicClaimRevisionSchema> {}

export const CausalStepSchema = S.Struct({
  sequence: PositiveInteger,
  proposition: ShortText,
  evidenceRefs: S.Array(ForensicRef).check(S.isMinLength(1), S.isMaxLength(64)),
});
export interface CausalStep extends S.Schema.Type<typeof CausalStepSchema> {}

export const ForensicFindingSchema = S.Struct({
  schema: S.Literal(FORENSIC_FINDING_VERSION),
  findingRef: ForensicRef,
  runRef: ForensicRef,
  claimRef: ForensicRef,
  title: ShortText,
  impact: LongText,
  causalSteps: S.Array(CausalStepSchema).check(S.isMinLength(1), S.isMaxLength(128)),
  sourceRefs: S.Array(ForensicRef).check(S.isMinLength(1), S.isMaxLength(256)),
  assumptions: BoundedShortTexts,
  severity: S.Literals(["low", "medium", "high", "critical"]),
  evidenceTier: EvidenceTier,
  pocRef: S.optionalKey(ForensicRef),
  verifierState: S.Literals(["not_requested", "pending", "confirmed", "dismissed", "inconclusive"]),
  disclosureState: S.Literals([
    "private",
    "awaiting_review",
    "approved_for_contact",
    "reported",
    "embargoed",
  ]),
  submittedAt: ForensicTimestamp,
}).annotate({ identifier: "ForensicFinding" });
export interface ForensicFinding extends S.Schema.Type<typeof ForensicFindingSchema> {}

export const ForensicHypothesisSchema = S.Struct({
  schema: S.Literal(FORENSIC_HYPOTHESIS_VERSION),
  hypothesisRef: ForensicRef,
  runRef: ForensicRef,
  suspectedMechanism: LongText,
  supportingRefs: BoundedRefs,
  missingEvidence: BoundedShortTexts.check(S.isMinLength(1)),
  nextCheck: LongText,
  consequenceIfTrue: LongText,
  state: S.Literals(["unverified", "investigating", "expired", "promoted", "dismissed"]),
  promotedFindingRef: S.optionalKey(ForensicRef),
  expiresAt: S.optionalKey(ForensicTimestamp),
  submittedAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (hypothesis) =>
          hypothesis.state !== "promoted" || hypothesis.promotedFindingRef !== undefined,
        { message: "promoted hypotheses require a finding ref" },
      ),
    ),
  )
  .annotate({ identifier: "ForensicHypothesis" });
export interface ForensicHypothesis extends S.Schema.Type<typeof ForensicHypothesisSchema> {}

export const EvidenceGraphNodeSchema = S.Struct({
  nodeRef: ForensicRef,
  kind: S.Literals([
    "source_claim",
    "transaction",
    "address",
    "collector",
    "vault",
    "component",
    "episode",
    "claim",
  ]),
  sourceRefs: BoundedRefs,
  attributesDigest: Sha256Digest,
});
export interface EvidenceGraphNode extends S.Schema.Type<typeof EvidenceGraphNodeSchema> {}

export const EvidenceGraphEdgeSchema = S.Struct({
  edgeRef: ForensicRef,
  fromNodeRef: ForensicRef,
  toNodeRef: ForensicRef,
  kind: S.Literals([
    "reports",
    "names_transaction",
    "pays_to",
    "funds",
    "spends_onward",
    "pools_with",
    "member_of",
    "occurs_in",
    "supports",
    "refutes",
  ]),
  derivationRuleRef: ForensicRef,
  sourceClaimRef: ForensicRef,
  generatedExplanation: ShortText,
  confidence: S.Literals([
    "pattern_candidate",
    "published_unconfirmed",
    "victim_confirmed",
    "observed",
  ]),
  evidenceRefs: S.Array(ForensicRef).check(S.isMinLength(1), S.isMaxLength(64)),
});
export interface EvidenceGraphEdge extends S.Schema.Type<typeof EvidenceGraphEdgeSchema> {}

export const ForensicEvidenceGraphSchema = S.Struct({
  schema: S.Literal(FORENSIC_EVIDENCE_GRAPH_VERSION),
  graphRef: ForensicRef,
  runRef: ForensicRef,
  rawInputDigest: Sha256Digest,
  derivationRevisionDigest: Sha256Digest,
  traversalMaxDepth: PositiveInteger,
  traversalLimitHit: S.Boolean,
  nodes: S.Array(EvidenceGraphNodeSchema).check(S.isMaxLength(100_000)),
  edges: S.Array(EvidenceGraphEdgeSchema).check(S.isMaxLength(250_000)),
  componentRefs: BoundedRefs,
  episodeRefs: BoundedRefs,
  completeness: S.Literals(["bounded_complete", "known_floor", "unmeasurable", "incomplete"]),
  limitationRefs: BoundedRefs,
  generatedAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (graph) => !graph.traversalLimitHit || graph.completeness !== "bounded_complete",
        { message: "a traversal limit hit cannot produce bounded_complete graph coverage" },
      ),
    ),
  )
  .annotate({ identifier: "ForensicEvidenceGraph" });
export interface ForensicEvidenceGraph extends S.Schema.Type<typeof ForensicEvidenceGraphSchema> {}

export type ClaimGateDecision =
  | Readonly<{ _tag: "Allowed"; claimRef: ForensicRef }>
  | Readonly<{
      _tag: "Refused";
      claimRef: ForensicRef;
      blockerRef: ForensicRef;
      missingEvidenceKinds: ReadonlyArray<EvidenceKind>;
    }>;

export const evaluateClaimGate = (claim: ForensicClaimRecord): ClaimGateDecision => {
  const observed = new Set(claim.evidence.map((entry) => entry.kind));
  const missingEvidenceKinds = REQUIRED_EVIDENCE_BY_CLAIM_KIND[claim.claimKind].filter(
    (kind) => !observed.has(kind),
  );
  if (missingEvidenceKinds.length === 0) {
    return { _tag: "Allowed", claimRef: claim.claimRef };
  }
  return {
    _tag: "Refused",
    claimRef: claim.claimRef,
    blockerRef: `blocker.forensic.claim.${claim.claimKind}.missing_evidence`,
    missingEvidenceKinds,
  };
};

export type ClaimRevisionOriginDecision =
  | Readonly<{ _tag: "Valid"; claimRef: ForensicRef; revisionRef: ForensicRef }>
  | Readonly<{ _tag: "Invalid"; blockerRef: ForensicRef }>;

export const evaluateClaimRevisionOrigin = (
  claim: ForensicClaimRecord,
  revision: ForensicClaimRevision,
): ClaimRevisionOriginDecision => {
  if (revision.claimRef !== claim.claimRef) {
    return { _tag: "Invalid", blockerRef: "blocker.forensic.claim_revision.claim_mismatch" };
  }
  if (revision.priorClaimDigest !== forensicSha256Digest(claim)) {
    return { _tag: "Invalid", blockerRef: "blocker.forensic.claim_revision.prior_digest_mismatch" };
  }
  return { _tag: "Valid", claimRef: claim.claimRef, revisionRef: revision.revisionRef };
};

export const countGraphOrphans = (graph: ForensicEvidenceGraph): NonNegativeInteger => {
  const connected = new Set<string>();
  for (const edge of graph.edges) {
    connected.add(edge.fromNodeRef);
    connected.add(edge.toNodeRef);
  }
  return NonNegativeInteger.make(graph.nodes.filter((node) => !connected.has(node.nodeRef)).length);
};

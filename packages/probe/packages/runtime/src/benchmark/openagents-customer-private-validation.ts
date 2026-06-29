import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import {
  decodeOpenAgentsRepoStudiedKnowledgeGraph,
  type OpenAgentsRepoStudiedKnowledgeGraph,
} from "./openagents-study-graph";
import {
  decodeOpenAgentsRepoStudyPacket,
  type OpenAgentsRepoStudyPacket,
} from "./openagents-study-packet";
import {
  type OpenAgentsStudybenchEvalHarnessReport,
} from "./openagents-studybench-eval-harness";
import {
  type OpenAgentsRepoStudiedKnowledgeVerificationReport,
} from "./openagents-study-verification";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

/**
 * Customer-private validation for repo-study packets.
 *
 * This module is the missing customer-private validation path for
 * autopilot.repo_study_packets.v1: it validates a study packet PRIVATELY for a
 * customer, against a committed private holdout, BEFORE the packet is delivered
 * to the customer or made claimable.
 *
 * Hard rules (enforced by the schema + validator):
 *  - The input private-holdout commitment is refs/digests/counts ONLY. Raw
 *    private task text, gold answers, rubric claims, and evidence excerpts are
 *    NEVER accepted here and NEVER appear in the public projection
 *    (sourceBoundary = "customer_refs_withheld").
 *  - The verdict is INERT: `deliverable` is false unless explicitly armed AND
 *    every privacy/correctness/lift gate passes; `customerPublicClaimAllowed`,
 *    `marketplacePackageAllowed`, and `payoutEligible` are always false here.
 *  - The delivery/claim seam (planCustomerPrivateValidationDelivery) ALWAYS
 *    returns effectsApplied:false: this module plans, it never delivers a real
 *    packet, marks a real packet claimable, sends, settles, or spends.
 *
 * Green for the broader promise still requires real customer data privacy
 * review, an armed delivery with a dereferenceable closeout receipt, marketplace
 * metering, pricing, payout eligibility, settlement, and owner sign-off per
 * proof.claim_upgrade_receipts.v1 — all owner-gated and out of scope for the
 * pure validation path built here.
 */
export const OPENAGENTS_CUSTOMER_PRIVATE_VALIDATION_SCHEMA_REF =
  "openagents.customer_private_repo_study_validation.v0" as const;

const WITHHELD_DIGEST_REF =
  "checksum.customer_private_holdout.withheld" as const;

export const OpenAgentsCustomerPrivateValidationGateState = S.Literals([
  // No flag, default posture: validation ran, delivery is held inert.
  "inert_disabled",
  // Flag armed but a required gate (privacy/correctness/lift/owner) is unmet.
  "armed_blocked",
  // Flag armed and every gate passed; delivery is permitted by policy but this
  // module STILL applies no real effect (effectsApplied is always false).
  "armed_ready",
]);
export type OpenAgentsCustomerPrivateValidationGateState =
  typeof OpenAgentsCustomerPrivateValidationGateState.Type;

export const OpenAgentsCustomerPrivateValidationVerdictState = S.Literals([
  "validated_held",
  "blocked",
]);
export type OpenAgentsCustomerPrivateValidationVerdictState =
  typeof OpenAgentsCustomerPrivateValidationVerdictState.Type;

/**
 * The customer's committed private holdout, expressed as refs/digests/counts
 * only. NO raw private content is admitted.
 */
export interface CustomerPrivateHoldoutCommitment {
  /** Stable split ref, e.g. split.customer.<id>.private_holdout.v0 */
  readonly splitRef: string;
  /** Stable dataset ref for the private holdout split. */
  readonly datasetRef: string;
  /**
   * Digest ref over the private holdout manifest (row ids, row hashes, source
   * commit refs, split policy refs). Either a sha256 ref or the explicit
   * withheld sentinel. Raw row content must NOT be embedded.
   */
  readonly checksumRef: string;
  /** Number of committed private holdout rows (>= 1). */
  readonly rowCount: number;
  /**
   * Measured private-holdout lift of the studied substrate over the baseline,
   * in basis points. Refs-only summary statistic; carries no row content.
   */
  readonly holdoutPassRateLiftBps: number;
  /** Number of private holdout rows the studied substrate passed. */
  readonly holdoutPassCount: number;
}

export const OpenAgentsCustomerPrivateValidationGate = S.Struct({
  blockedReasonRefs: S.Array(S.String),
  effectsApplied: S.Literal(false),
  flagName: S.Literal("CUSTOMER_PRIVATE_REPO_STUDY_DELIVERY_ENABLED"),
  ownerSignoffPresent: S.Boolean,
  state: OpenAgentsCustomerPrivateValidationGateState,
});
export type OpenAgentsCustomerPrivateValidationGate =
  typeof OpenAgentsCustomerPrivateValidationGate.Type;

export const OpenAgentsCustomerPrivateValidationVerdict = S.Struct({
  blockerRefs: S.Array(S.String),
  correctnessGatePassed: S.Boolean,
  customerPublicClaimAllowed: S.Literal(false),
  deliverable: S.Literal(false),
  deliveryGate: OpenAgentsCustomerPrivateValidationGate,
  evalReportHash: S.String,
  evalReportRef: S.String,
  evidenceRefs: S.Array(S.String),
  generatedAt: S.String,
  graphHash: S.String,
  graphRef: S.String,
  holdoutChecksumRef: S.String,
  holdoutDatasetRef: S.String,
  holdoutLiftMet: S.Boolean,
  holdoutPassCount: S.Number,
  holdoutPassRateLiftBps: S.Number,
  holdoutRowCount: S.Number,
  holdoutSplitRef: S.String,
  marketplacePackageAllowed: S.Literal(false),
  packetHash: S.String,
  packetRef: S.String,
  payoutEligible: S.Literal(false),
  privacyDisciplinePassed: S.Boolean,
  repo: S.String,
  safeCopy: S.String,
  schemaRef: S.Literal(OPENAGENTS_CUSTOMER_PRIVATE_VALIDATION_SCHEMA_REF),
  sourceBoundary: S.Literal("customer_refs_withheld"),
  state: OpenAgentsCustomerPrivateValidationVerdictState,
  studiedBeatsBaseline: S.Boolean,
  unsafeCopyRefs: S.Array(S.String),
  validatorReviewRequired: S.Boolean,
  verdictHash: S.String,
  verdictRef: S.String,
  verificationHash: S.String,
  verificationRef: S.String,
});
export type OpenAgentsCustomerPrivateValidationVerdict =
  typeof OpenAgentsCustomerPrivateValidationVerdict.Type;

export interface BuildOpenAgentsCustomerPrivateValidationInput {
  readonly evalReport: OpenAgentsStudybenchEvalHarnessReport;
  readonly generatedAt?: string;
  readonly graph: OpenAgentsRepoStudiedKnowledgeGraph;
  readonly holdout: CustomerPrivateHoldoutCommitment;
  /**
   * Whether the customer-private delivery flag is armed. Default false. Even
   * when armed, this module applies no real effect.
   */
  readonly deliveryFlagArmed?: boolean;
  /**
   * Whether an owner sign-off (per proof.claim_upgrade_receipts.v1) is recorded
   * for an armed delivery. Default false.
   */
  readonly ownerSignoffPresent?: boolean;
  readonly packet: OpenAgentsRepoStudyPacket;
  readonly repo: string;
  readonly verification: OpenAgentsRepoStudiedKnowledgeVerificationReport;
}

const MIN_HOLDOUT_LIFT_BPS = 1;

export function buildOpenAgentsCustomerPrivateValidation(
  input: BuildOpenAgentsCustomerPrivateValidationInput,
): Effect.Effect<
  OpenAgentsCustomerPrivateValidationVerdict,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(input.repo, "customerPrivateValidation.repo");

    // Decode the inputs through their existing public-projection-safe contracts.
    const packet = yield* decodeOpenAgentsRepoStudyPacket(input.packet);
    const graph = yield* decodeOpenAgentsRepoStudiedKnowledgeGraph(input.graph);

    // The packet, graph, verification, and eval report must all describe the
    // same studied artifact, or the verdict is meaningless.
    if (graph.packetHash !== packet.packetHash) {
      return yield* validationError(
        "customerPrivateValidation.graph.packetHash",
        "graph must be built from the validated packet",
      );
    }
    if (input.verification.packetHash !== packet.packetHash) {
      return yield* validationError(
        "customerPrivateValidation.verification.packetHash",
        "verification report must cover the validated packet",
      );
    }
    if (input.evalReport.packetHash !== packet.packetHash) {
      return yield* validationError(
        "customerPrivateValidation.evalReport.packetHash",
        "eval report must cover the validated packet",
      );
    }

    yield* requireRefsWithheldHoldout(input.holdout);

    const correctnessGatePassed = input.verification.correctnessGatePassed;
    const studiedBeatsBaseline = input.evalReport.comparison.studiedBeatsBaseline;
    const holdoutLiftMet =
      input.holdout.holdoutPassRateLiftBps >= MIN_HOLDOUT_LIFT_BPS &&
      input.holdout.holdoutPassCount > 0 &&
      input.holdout.holdoutPassCount <= input.holdout.rowCount;
    const privacyDisciplinePassed = holdoutPrivacyDisciplinePassed(input.holdout);

    const validatorReviewRequired = input.verification.validatorReviewRequired;

    const verdictPassed =
      correctnessGatePassed &&
      studiedBeatsBaseline &&
      holdoutLiftMet &&
      privacyDisciplinePassed;

    const blockerRefs = buildVerdictBlockerRefs({
      correctnessGatePassed,
      holdoutLiftMet,
      privacyDisciplinePassed,
      studiedBeatsBaseline,
      validatorReviewRequired,
    });

    const deliveryGate = buildDeliveryGate({
      deliveryFlagArmed: input.deliveryFlagArmed ?? false,
      ownerSignoffPresent: input.ownerSignoffPresent ?? false,
      verdictPassed,
      validatorReviewRequired,
    });

    const generatedAt =
      input.generatedAt ??
      "generated_at.withheld_for_stable_customer_private_validation_hash";

    const evidenceRefs = [
      input.packet.packetRef,
      graph.graphRef,
      input.verification.verificationRef,
      input.evalReport.reportRef,
      input.holdout.splitRef,
      input.holdout.datasetRef,
      input.holdout.checksumRef,
      "docs/research/machine-studying/openagents-studybench/private-boundary.md",
      "docs/promises/2026-06-17-repo-studying-product-promise-gate-review.md",
    ];

    const base: OpenAgentsCustomerPrivateValidationVerdict = {
      blockerRefs,
      correctnessGatePassed,
      customerPublicClaimAllowed: false,
      deliverable: false,
      deliveryGate,
      evalReportHash: input.evalReport.reportHash,
      evalReportRef: input.evalReport.reportRef,
      evidenceRefs,
      generatedAt,
      graphHash: graph.graphHash,
      graphRef: graph.graphRef,
      holdoutChecksumRef: input.holdout.checksumRef,
      holdoutDatasetRef: input.holdout.datasetRef,
      holdoutLiftMet,
      holdoutPassCount: input.holdout.holdoutPassCount,
      holdoutPassRateLiftBps: input.holdout.holdoutPassRateLiftBps,
      holdoutRowCount: input.holdout.rowCount,
      holdoutSplitRef: input.holdout.splitRef,
      marketplacePackageAllowed: false,
      packetHash: input.packet.packetHash,
      packetRef: input.packet.packetRef,
      payoutEligible: false,
      privacyDisciplinePassed,
      repo: input.repo,
      safeCopy:
        "Customer-private validation ran against a committed private holdout (refs and digests only). The verdict is held inert; the packet is not delivered or claimable, and no customer, marketplace, payout, or settlement claim is made.",
      schemaRef: OPENAGENTS_CUSTOMER_PRIVATE_VALIDATION_SCHEMA_REF,
      sourceBoundary: "customer_refs_withheld",
      state: verdictPassed ? "validated_held" : "blocked",
      studiedBeatsBaseline,
      unsafeCopyRefs: [
        "blocked_claim.customer_repo_studying_live",
        "blocked_claim.study_packet_delivered_to_customer",
        "blocked_claim.study_packet_claimable",
        "blocked_claim.study_packet_marketplace_package",
        "blocked_claim.machine_studying_payout_eligible",
      ],
      validatorReviewRequired,
      verdictHash: "sha256:pending",
      verdictRef: "customer_private_repo_study_validation.pending",
      verificationHash: input.verification.verificationHash,
      verificationRef: input.verification.verificationRef,
    };

    const verdictHash = openAgentsCustomerPrivateValidationVerdictHash(base);

    return yield* decodeOpenAgentsCustomerPrivateValidationVerdict({
      ...base,
      verdictHash,
      verdictRef: `customer_private_repo_study_validation.${slugRepo(input.repo)}.${shortHash(verdictHash)}`,
    });
  });
}

/**
 * FLAG-GATED INERT delivery/claim seam. It plans whether a validated packet
 * WOULD be deliverable/claimable for a customer, but it NEVER applies a real
 * effect: effectsApplied is always false.
 */
export interface OpenAgentsCustomerPrivateValidationDeliveryPlan {
  readonly customerRef: string;
  readonly deliverable: false;
  readonly effectsApplied: false;
  readonly gateState: OpenAgentsCustomerPrivateValidationGateState;
  readonly reasonRefs: ReadonlyArray<string>;
  readonly verdictRef: string;
  readonly wouldDeliverWhenArmed: boolean;
}

export function planCustomerPrivateValidationDelivery(input: {
  readonly customerRef: string;
  readonly verdict: OpenAgentsCustomerPrivateValidationVerdict;
}): OpenAgentsCustomerPrivateValidationDeliveryPlan {
  const gateState = input.verdict.deliveryGate.state;
  const wouldDeliverWhenArmed =
    input.verdict.state === "validated_held" && gateState === "armed_ready";
  const reasonRefs =
    gateState === "armed_ready"
      ? ["delivery.inert.no_real_effect_applied_by_this_module"]
      : input.verdict.deliveryGate.blockedReasonRefs.length > 0
        ? input.verdict.deliveryGate.blockedReasonRefs
        : ["delivery.inert.flag_disabled"];
  return {
    customerRef: input.customerRef,
    deliverable: false,
    effectsApplied: false,
    gateState,
    reasonRefs,
    verdictRef: input.verdict.verdictRef,
    wouldDeliverWhenArmed,
  };
}

export function decodeOpenAgentsCustomerPrivateValidationVerdict(
  value: unknown,
): Effect.Effect<
  OpenAgentsCustomerPrivateValidationVerdict,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "customerPrivateValidationVerdict");
    const verdict = yield* S.decodeUnknownEffect(OpenAgentsCustomerPrivateValidationVerdict)(value).pipe(
      Effect.mapError((error) =>
        new ProbeBenchmarkContractError({
          path: "customerPrivateValidationVerdict",
          reason: String(error),
        }),
      ),
    );
    yield* validateCustomerPrivateValidationVerdict(verdict);
    return verdict;
  });
}

export function openAgentsCustomerPrivateValidationVerdictHash(
  verdict: OpenAgentsCustomerPrivateValidationVerdict,
): string {
  const {
    generatedAt: _generatedAt,
    verdictHash: _verdictHash,
    verdictRef: _verdictRef,
    ...stable
  } = verdict;
  return sha256Ref(stableJson(stable));
}

function buildDeliveryGate(input: {
  readonly deliveryFlagArmed: boolean;
  readonly ownerSignoffPresent: boolean;
  readonly validatorReviewRequired: boolean;
  readonly verdictPassed: boolean;
}): OpenAgentsCustomerPrivateValidationGate {
  if (!input.deliveryFlagArmed) {
    return {
      blockedReasonRefs: [],
      effectsApplied: false,
      flagName: "CUSTOMER_PRIVATE_REPO_STUDY_DELIVERY_ENABLED",
      ownerSignoffPresent: input.ownerSignoffPresent,
      state: "inert_disabled",
    };
  }

  const blockedReasonRefs: string[] = [];
  if (!input.verdictPassed) {
    blockedReasonRefs.push("delivery.blocked.validation_verdict_not_passed");
  }
  if (input.validatorReviewRequired) {
    blockedReasonRefs.push("delivery.blocked.validator_review_remainder_open");
  }
  if (!input.ownerSignoffPresent) {
    blockedReasonRefs.push("delivery.blocked.owner_signoff_missing");
  }

  return {
    blockedReasonRefs,
    effectsApplied: false,
    flagName: "CUSTOMER_PRIVATE_REPO_STUDY_DELIVERY_ENABLED",
    ownerSignoffPresent: input.ownerSignoffPresent,
    state: blockedReasonRefs.length === 0 ? "armed_ready" : "armed_blocked",
  };
}

function buildVerdictBlockerRefs(input: {
  readonly correctnessGatePassed: boolean;
  readonly holdoutLiftMet: boolean;
  readonly privacyDisciplinePassed: boolean;
  readonly studiedBeatsBaseline: boolean;
  readonly validatorReviewRequired: boolean;
}): ReadonlyArray<string> {
  const blockers: string[] = [];
  if (!input.correctnessGatePassed) {
    blockers.push("blocker.customer_private_validation.correctness_gate_failed");
  }
  if (!input.studiedBeatsBaseline) {
    blockers.push("blocker.customer_private_validation.studied_substrate_lift_missing");
  }
  if (!input.holdoutLiftMet) {
    blockers.push("blocker.customer_private_validation.private_holdout_lift_missing");
  }
  if (!input.privacyDisciplinePassed) {
    blockers.push("blocker.customer_private_validation.privacy_discipline_failed");
  }
  if (input.validatorReviewRequired) {
    blockers.push("blocker.customer_private_validation.validator_review_remainder_open");
  }
  return blockers;
}

function holdoutPrivacyDisciplinePassed(
  holdout: CustomerPrivateHoldoutCommitment,
): boolean {
  // The commitment must reference a populated split (or the explicit withheld
  // sentinel), name a stable split + dataset ref, and carry no row content.
  if (holdout.rowCount < 1) {
    return false;
  }
  if (holdout.splitRef.trim().length === 0 || holdout.datasetRef.trim().length === 0) {
    return false;
  }
  return isHoldoutDigestRef(holdout.checksumRef);
}

function isHoldoutDigestRef(checksumRef: string): boolean {
  return checksumRef === WITHHELD_DIGEST_REF || checksumRef.startsWith("sha256:");
}

function requireRefsWithheldHoldout(
  holdout: CustomerPrivateHoldoutCommitment,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(holdout.splitRef, "customerPrivateValidation.holdout.splitRef");
    yield* requireNonEmpty(holdout.datasetRef, "customerPrivateValidation.holdout.datasetRef");
    yield* requireNonEmpty(holdout.checksumRef, "customerPrivateValidation.holdout.checksumRef");

    if (!isHoldoutDigestRef(holdout.checksumRef)) {
      return yield* validationError(
        "customerPrivateValidation.holdout.checksumRef",
        "private holdout checksum must be a sha256 digest ref or the explicit withheld sentinel, never raw content",
      );
    }
    if (!Number.isInteger(holdout.rowCount) || holdout.rowCount < 1) {
      return yield* validationError(
        "customerPrivateValidation.holdout.rowCount",
        "private holdout commitment must carry at least one row",
      );
    }
    if (
      !Number.isInteger(holdout.holdoutPassCount) ||
      holdout.holdoutPassCount < 0 ||
      holdout.holdoutPassCount > holdout.rowCount
    ) {
      return yield* validationError(
        "customerPrivateValidation.holdout.holdoutPassCount",
        "private holdout pass count must be a bounded non-negative integer within the row count",
      );
    }
    if (
      !Number.isFinite(holdout.holdoutPassRateLiftBps) ||
      holdout.holdoutPassRateLiftBps < -10_000 ||
      holdout.holdoutPassRateLiftBps > 10_000
    ) {
      return yield* validationError(
        "customerPrivateValidation.holdout.holdoutPassRateLiftBps",
        "private holdout lift must be a bounded basis-point measurement",
      );
    }
  });
}

function validateCustomerPrivateValidationVerdict(
  verdict: OpenAgentsCustomerPrivateValidationVerdict,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(verdict.repo, "customerPrivateValidationVerdict.repo");
    yield* requireNonEmpty(verdict.verdictRef, "customerPrivateValidationVerdict.verdictRef");
    yield* requireSha256(verdict.verdictHash, "customerPrivateValidationVerdict.verdictHash");
    yield* requireSha256(verdict.packetHash, "customerPrivateValidationVerdict.packetHash");
    yield* requireSha256(verdict.graphHash, "customerPrivateValidationVerdict.graphHash");
    yield* requireSha256(verdict.verificationHash, "customerPrivateValidationVerdict.verificationHash");
    yield* requireSha256(verdict.evalReportHash, "customerPrivateValidationVerdict.evalReportHash");

    if (
      verdict.customerPublicClaimAllowed !== false ||
      verdict.marketplacePackageAllowed !== false ||
      verdict.payoutEligible !== false ||
      verdict.deliverable !== false
    ) {
      return yield* validationError(
        "customerPrivateValidationVerdict.claimGates",
        "customer-private validation must not grant customer, delivery, marketplace, payout, or settlement claims",
      );
    }

    if (verdict.deliveryGate.effectsApplied !== false) {
      return yield* validationError(
        "customerPrivateValidationVerdict.deliveryGate.effectsApplied",
        "validation delivery gate must never apply a real effect",
      );
    }

    if (verdict.state === "validated_held") {
      if (
        !verdict.correctnessGatePassed ||
        !verdict.studiedBeatsBaseline ||
        !verdict.holdoutLiftMet ||
        !verdict.privacyDisciplinePassed
      ) {
        return yield* validationError(
          "customerPrivateValidationVerdict.state",
          "validated_held requires the correctness gate, studied lift, private-holdout lift, and privacy discipline to all pass",
        );
      }
    }

    if (verdict.holdoutPassCount > verdict.holdoutRowCount) {
      return yield* validationError(
        "customerPrivateValidationVerdict.holdoutPassCount",
        "private holdout pass count cannot exceed the committed row count",
      );
    }

    if (verdict.verdictHash !== openAgentsCustomerPrivateValidationVerdictHash(verdict)) {
      return yield* validationError(
        "customerPrivateValidationVerdict.verdictHash",
        "must match the deterministic verdict hash",
      );
    }
  });
}

function requireNonEmpty(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.trim().length === 0 ? validationError(path, "must be non-empty") : Effect.void;
}

function requireSha256(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.startsWith("sha256:") ? Effect.void : validationError(path, "must be a sha256 ref");
}

function slugRepo(repo: string): string {
  return repo
    .replace(/[^a-zA-Z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .toLowerCase()
    .slice(0, 80);
}

function validationError(path: string, reason: string): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}

import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

/**
 * Customer-data privacy review preflight for the external-repo-studying pilot.
 *
 * This module is the smallest genuine piece of the missing privacy-policy
 * control surface for autopilot.external_repo_studying_pilot.v1
 * (blocker.product_promises.external_repo_studying_privacy_policy_missing): it
 * decides whether a customer-data privacy review for an external (non-OpenAgents)
 * repo study WOULD clear, based on REFS and COUNTS ONLY, and — only when every
 * gate passes — derives the deterministic `privacyReviewRef` that the sibling
 * self-serve upload preflight already consumes as a presence check.
 *
 * It performs NO real legal/privacy review and processes NO customer data. It
 * computes an inert preflight VERDICT over a review REQUEST that is itself
 * expressed only as references (a data-processing-agreement ref, a retention
 * policy ref, a customer-authorization ref, a reviewer ref) plus bounded
 * declarations (a retention window in days, a closed set of declared PII
 * categories). No raw customer data, repository content, file paths beyond the
 * external repo slug, PII values, or reviewer notes ever cross this boundary.
 *
 * Hard rules (enforced by the schema + validator):
 *  - Refs/counts/enums ONLY. sourceBoundary = "customer_refs_withheld".
 *  - INERT by construction: `reviewCleared` is ALWAYS false. The module computes
 *    WHETHER a review WOULD clear when the review flag is armed and every gate
 *    passes (wouldClearWhenArmed), but it never clears a real customer review,
 *    authorizes ingestion, delivers a packet, marks anything claimable, sends,
 *    settles, or spends. effectsApplied is always false.
 *  - Flag-gated default-OFF: the review flag
 *    (EXTERNAL_REPO_STUDY_PRIVACY_REVIEW_ENABLED) defaults disabled. With no
 *    flag the clearance gate resolves inert_disabled.
 *  - No claim widening: customerPublicClaimAllowed, marketplacePackageAllowed,
 *    and payoutEligible are always false here.
 *
 * Green for the broader promise still requires a real, human/legal customer-data
 * privacy review backing an armed clearance, real durable-storage and access
 * controls, an armed ingestion against a real customer repo with a
 * dereferenceable closeout receipt, marketplace metering, pricing, payout
 * eligibility, settlement, and owner sign-off per proof.claim_upgrade_receipts.v1
 * — all owner-gated and out of scope for the pure preflight built here.
 */
export const OPENAGENTS_EXTERNAL_REPO_STUDY_PRIVACY_REVIEW_PREFLIGHT_SCHEMA_REF =
  "openagents.external_repo_study_privacy_review_preflight.v0" as const;

export const ExternalRepoStudyPrivacyReviewFlagName =
  "EXTERNAL_REPO_STUDY_PRIVACY_REVIEW_ENABLED" as const;

/** Conservative refs-only retention cap. Bounds the DECLARED retention window;
 *  no customer data is read. A real review would re-verify against the DPA. */
export const PRIVACY_REVIEW_MAX_RETENTION_DAYS = 365;

/** The closed set of PII categories a pilot study may DECLARE it touches. Any
 *  declared category outside this set blocks the review. "none" means the study
 *  declares it touches no customer PII. */
export const PrivacyReviewAllowedPiiCategory = S.Literals([
  "none",
  "contributor_handle",
  "commit_author_email",
  "code_comment_text",
]);
export type PrivacyReviewAllowedPiiCategory =
  typeof PrivacyReviewAllowedPiiCategory.Type;

const ALLOWED_PII_CATEGORIES: ReadonlySet<string> = new Set([
  "none",
  "contributor_handle",
  "commit_author_email",
  "code_comment_text",
]);

export const OpenAgentsExternalRepoStudyPrivacyReviewGateState = S.Literals([
  // No flag, default posture: preflight evaluated, held inert.
  "inert_disabled",
  // Flag armed but a required gate (preflight/reviewer-signoff) is unmet.
  "armed_blocked",
  // Flag armed and every gate passed; clearance permitted by policy, but this
  // module STILL applies no real effect (effectsApplied is always false).
  "armed_ready",
]);
export type OpenAgentsExternalRepoStudyPrivacyReviewGateState =
  typeof OpenAgentsExternalRepoStudyPrivacyReviewGateState.Type;

export const OpenAgentsExternalRepoStudyPrivacyReviewState = S.Literals([
  "review_ready_held",
  "blocked",
]);
export type OpenAgentsExternalRepoStudyPrivacyReviewState =
  typeof OpenAgentsExternalRepoStudyPrivacyReviewState.Type;

/**
 * A privacy-review request, expressed as refs/counts/enums only. No raw customer
 * data, repository content, PII values, or reviewer notes.
 */
export interface ExternalRepoStudyPrivacyReviewRequest {
  /** Stable customer ref the review is for, e.g. customer.<id>.v0 */
  readonly customerRef: string;
  /** External (non-OpenAgents) repo slug, e.g. ExampleCorp/widget-service */
  readonly repo: string;
  /** Ref to the signed data-processing agreement (DPA) covering this study. */
  readonly dataProcessingAgreementRef?: string;
  /** Ref to the recorded data-retention policy for this study's artifacts. */
  readonly retentionPolicyRef?: string;
  /** Declared retention window in days (bounded; no data is read). */
  readonly retentionDays: number;
  /** Ref proving the customer authorized this study of their repo data. */
  readonly customerAuthorizationRef?: string;
  /** Declared PII categories the study touches (closed set). */
  readonly declaredPiiCategories: ReadonlyArray<string>;
  /** Stable ref of the privacy reviewer, e.g. reviewer.privacy.<id>.v0 */
  readonly reviewerRef?: string;
}

export const OpenAgentsExternalRepoStudyPrivacyReviewGate = S.Struct({
  blockedReasonRefs: S.Array(S.String),
  effectsApplied: S.Literal(false),
  flagName: S.Literal(ExternalRepoStudyPrivacyReviewFlagName),
  reviewerSignoffPresent: S.Boolean,
  state: OpenAgentsExternalRepoStudyPrivacyReviewGateState,
});
export type OpenAgentsExternalRepoStudyPrivacyReviewGate =
  typeof OpenAgentsExternalRepoStudyPrivacyReviewGate.Type;

export const OpenAgentsExternalRepoStudyPrivacyReviewPreflight = S.Struct({
  blockerRefs: S.Array(S.String),
  clearanceGate: OpenAgentsExternalRepoStudyPrivacyReviewGate,
  customerAuthorizationPresent: S.Boolean,
  customerPublicClaimAllowed: S.Literal(false),
  customerRef: S.String,
  dataProcessingAgreementPresent: S.Boolean,
  declaredPiiCategories: S.Array(S.String),
  declaredPiiCategoriesWithinPolicy: S.Boolean,
  effectsApplied: S.Literal(false),
  evidenceRefs: S.Array(S.String),
  generatedAt: S.String,
  marketplacePackageAllowed: S.Literal(false),
  payoutEligible: S.Literal(false),
  preflightHash: S.String,
  preflightRef: S.String,
  // Deterministic ref the self-serve upload preflight consumes as its
  // privacyReviewRef once (and only once) this review would clear.
  privacyReviewRef: S.NullOr(S.String),
  repo: S.String,
  retentionDays: S.Number,
  retentionPolicyPresent: S.Boolean,
  retentionWithinCap: S.Boolean,
  reviewCleared: S.Literal(false),
  safeCopy: S.String,
  schemaRef: S.Literal(
    OPENAGENTS_EXTERNAL_REPO_STUDY_PRIVACY_REVIEW_PREFLIGHT_SCHEMA_REF,
  ),
  sourceBoundary: S.Literal("customer_refs_withheld"),
  state: OpenAgentsExternalRepoStudyPrivacyReviewState,
  unsafeCopyRefs: S.Array(S.String),
  wouldClearWhenArmed: S.Boolean,
});
export type OpenAgentsExternalRepoStudyPrivacyReviewPreflight =
  typeof OpenAgentsExternalRepoStudyPrivacyReviewPreflight.Type;

export interface BuildOpenAgentsExternalRepoStudyPrivacyReviewPreflightInput {
  readonly generatedAt?: string;
  readonly request: ExternalRepoStudyPrivacyReviewRequest;
  /**
   * Whether the review flag is armed. Default false. Even when armed, this
   * module applies no real effect (reviewCleared/effectsApplied stay false).
   */
  readonly reviewFlagArmed?: boolean;
  /**
   * Whether a privacy reviewer sign-off is recorded for an armed review.
   * Default false.
   */
  readonly reviewerSignoffPresent?: boolean;
}

export function buildOpenAgentsExternalRepoStudyPrivacyReviewPreflight(
  input: BuildOpenAgentsExternalRepoStudyPrivacyReviewPreflightInput,
): Effect.Effect<
  OpenAgentsExternalRepoStudyPrivacyReviewPreflight,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const request = input.request;
    const repo = request.repo;
    yield* requireNonEmpty(repo, "externalRepoStudyPrivacyReview.repo");
    yield* requireNonEmpty(
      request.customerRef,
      "externalRepoStudyPrivacyReview.customerRef",
    );

    if (repo === "OpenAgentsInc/openagents") {
      return yield* privacyError(
        "externalRepoStudyPrivacyReview.repo",
        "privacy review target must be an external (non-OpenAgents) pilot repo",
      );
    }

    const dataProcessingAgreementPresent =
      (request.dataProcessingAgreementRef ?? "").trim().length > 0;
    const retentionPolicyPresent =
      (request.retentionPolicyRef ?? "").trim().length > 0;
    const customerAuthorizationPresent =
      (request.customerAuthorizationRef ?? "").trim().length > 0;

    const retentionDays = request.retentionDays;
    const retentionWithinCap =
      Number.isInteger(retentionDays) &&
      retentionDays > 0 &&
      retentionDays <= PRIVACY_REVIEW_MAX_RETENTION_DAYS;

    const declaredPiiCategories = [...request.declaredPiiCategories];
    const declaredPiiCategoriesWithinPolicy =
      declaredPiiCategories.length > 0 &&
      declaredPiiCategories.every((category) =>
        ALLOWED_PII_CATEGORIES.has(category),
      );

    const preflightPassed =
      dataProcessingAgreementPresent &&
      retentionPolicyPresent &&
      customerAuthorizationPresent &&
      retentionWithinCap &&
      declaredPiiCategoriesWithinPolicy;

    const blockerRefs = buildPrivacyBlockerRefs({
      customerAuthorizationPresent,
      dataProcessingAgreementPresent,
      declaredPiiCategoriesWithinPolicy,
      retentionPolicyPresent,
      retentionWithinCap,
    });

    const clearanceGate = buildPrivacyGate({
      preflightPassed,
      reviewerSignoffPresent: input.reviewerSignoffPresent ?? false,
      reviewFlagArmed: input.reviewFlagArmed ?? false,
    });

    const wouldClearWhenArmed =
      preflightPassed && clearanceGate.state === "armed_ready";

    const generatedAt =
      input.generatedAt ??
      "generated_at.withheld_for_stable_external_repo_study_privacy_review_preflight_hash";

    const evidenceRefs = [
      request.customerRef,
      ...(dataProcessingAgreementPresent && request.dataProcessingAgreementRef
        ? [request.dataProcessingAgreementRef]
        : []),
      ...(retentionPolicyPresent && request.retentionPolicyRef
        ? [request.retentionPolicyRef]
        : []),
      ...(customerAuthorizationPresent && request.customerAuthorizationRef
        ? [request.customerAuthorizationRef]
        : []),
      ...(request.reviewerRef && request.reviewerRef.trim().length > 0
        ? [request.reviewerRef]
        : []),
      "docs/research/machine-studying/openagents-studybench/private-boundary.md",
      "docs/launch/vertex-fleet/autopilot.external_repo_studying_pilot.v1.md",
    ];

    // The privacyReviewRef is the deterministic artifact the self-serve upload
    // preflight consumes. It is derived ONLY when the review would clear; while
    // blocked it is null so a blocked review cannot satisfy the upload's
    // privacy-review presence check.
    const base: OpenAgentsExternalRepoStudyPrivacyReviewPreflight = {
      blockerRefs,
      clearanceGate,
      customerAuthorizationPresent,
      customerPublicClaimAllowed: false,
      customerRef: request.customerRef,
      dataProcessingAgreementPresent,
      declaredPiiCategories,
      declaredPiiCategoriesWithinPolicy,
      effectsApplied: false,
      evidenceRefs,
      generatedAt,
      marketplacePackageAllowed: false,
      payoutEligible: false,
      preflightHash: "sha256:pending",
      preflightRef: "external_repo_study_privacy_review_preflight.pending",
      privacyReviewRef: null,
      repo,
      retentionDays,
      retentionPolicyPresent,
      retentionWithinCap,
      reviewCleared: false,
      safeCopy:
        "Privacy review preflight evaluated an external-repo study privacy review from refs, counts, and a closed PII-category set only (DPA ref, retention policy ref, customer-authorization ref, retention window, declared PII categories). The preflight is held inert; no customer data is processed, no review is cleared, no ingestion is authorized, and no customer, marketplace, payout, or settlement claim is made.",
      schemaRef:
        OPENAGENTS_EXTERNAL_REPO_STUDY_PRIVACY_REVIEW_PREFLIGHT_SCHEMA_REF,
      sourceBoundary: "customer_refs_withheld",
      state: preflightPassed ? "review_ready_held" : "blocked",
      unsafeCopyRefs: [
        "blocked_claim.customer_data_privacy_cleared_live",
        "blocked_claim.customer_repo_data_processing_authorized",
        "blocked_claim.privacy_review_grants_ingestion",
        "blocked_claim.study_packet_marketplace_package",
        "blocked_claim.machine_studying_payout_eligible",
      ],
      wouldClearWhenArmed,
    };

    const withRef: OpenAgentsExternalRepoStudyPrivacyReviewPreflight = {
      ...base,
      privacyReviewRef: preflightPassed
        ? `privacy_review.${slugRepo(repo)}.v0`
        : null,
    };

    const preflightHash =
      openAgentsExternalRepoStudyPrivacyReviewPreflightHash(withRef);

    return yield* decodeOpenAgentsExternalRepoStudyPrivacyReviewPreflight({
      ...withRef,
      preflightHash,
      preflightRef: `external_repo_study_privacy_review_preflight.${slugRepo(repo)}.${shortHash(preflightHash)}`,
    });
  });
}

export function decodeOpenAgentsExternalRepoStudyPrivacyReviewPreflight(
  value: unknown,
): Effect.Effect<
  OpenAgentsExternalRepoStudyPrivacyReviewPreflight,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(
      value,
      "externalRepoStudyPrivacyReview",
    );
    const preflight = yield* S.decodeUnknownEffect(
      OpenAgentsExternalRepoStudyPrivacyReviewPreflight,
    )(value).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBenchmarkContractError({
            path: "externalRepoStudyPrivacyReview",
            reason: String(error),
          }),
      ),
    );
    yield* validateExternalRepoStudyPrivacyReviewPreflight(preflight);
    return preflight;
  });
}

export function openAgentsExternalRepoStudyPrivacyReviewPreflightHash(
  preflight: OpenAgentsExternalRepoStudyPrivacyReviewPreflight,
): string {
  const {
    preflightHash: _preflightHash,
    preflightRef: _preflightRef,
    generatedAt: _generatedAt,
    ...stable
  } = preflight;
  return sha256Ref(stableJson(stable));
}

function buildPrivacyGate(input: {
  readonly preflightPassed: boolean;
  readonly reviewerSignoffPresent: boolean;
  readonly reviewFlagArmed: boolean;
}): OpenAgentsExternalRepoStudyPrivacyReviewGate {
  if (!input.reviewFlagArmed) {
    return {
      blockedReasonRefs: [],
      effectsApplied: false,
      flagName: ExternalRepoStudyPrivacyReviewFlagName,
      reviewerSignoffPresent: input.reviewerSignoffPresent,
      state: "inert_disabled",
    };
  }

  const blockedReasonRefs: string[] = [];
  if (!input.preflightPassed) {
    blockedReasonRefs.push("privacy_review.blocked.preflight_not_passed");
  }
  if (!input.reviewerSignoffPresent) {
    blockedReasonRefs.push("privacy_review.blocked.reviewer_signoff_missing");
  }

  return {
    blockedReasonRefs,
    effectsApplied: false,
    flagName: ExternalRepoStudyPrivacyReviewFlagName,
    reviewerSignoffPresent: input.reviewerSignoffPresent,
    state: blockedReasonRefs.length === 0 ? "armed_ready" : "armed_blocked",
  };
}

function buildPrivacyBlockerRefs(input: {
  readonly customerAuthorizationPresent: boolean;
  readonly dataProcessingAgreementPresent: boolean;
  readonly declaredPiiCategoriesWithinPolicy: boolean;
  readonly retentionPolicyPresent: boolean;
  readonly retentionWithinCap: boolean;
}): ReadonlyArray<string> {
  const blockers: string[] = [];
  if (!input.dataProcessingAgreementPresent) {
    blockers.push(
      "blocker.external_repo_study_privacy_review.data_processing_agreement_missing",
    );
  }
  if (!input.retentionPolicyPresent) {
    blockers.push(
      "blocker.external_repo_study_privacy_review.retention_policy_missing",
    );
  }
  if (!input.retentionWithinCap) {
    blockers.push(
      "blocker.external_repo_study_privacy_review.retention_window_out_of_bounds",
    );
  }
  if (!input.customerAuthorizationPresent) {
    blockers.push(
      "blocker.external_repo_study_privacy_review.customer_authorization_missing",
    );
  }
  if (!input.declaredPiiCategoriesWithinPolicy) {
    blockers.push(
      "blocker.external_repo_study_privacy_review.declared_pii_categories_out_of_policy",
    );
  }
  return blockers;
}

function validateExternalRepoStudyPrivacyReviewPreflight(
  preflight: OpenAgentsExternalRepoStudyPrivacyReviewPreflight,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(
      preflight.repo,
      "externalRepoStudyPrivacyReview.repo",
    );
    yield* requireNonEmpty(
      preflight.customerRef,
      "externalRepoStudyPrivacyReview.customerRef",
    );
    yield* requireNonEmpty(
      preflight.preflightRef,
      "externalRepoStudyPrivacyReview.preflightRef",
    );
    yield* requireSha256(
      preflight.preflightHash,
      "externalRepoStudyPrivacyReview.preflightHash",
    );

    if (preflight.repo === "OpenAgentsInc/openagents") {
      return yield* privacyError(
        "externalRepoStudyPrivacyReview.repo",
        "privacy review target must be an external (non-OpenAgents) pilot repo",
      );
    }

    if (
      preflight.customerPublicClaimAllowed !== false ||
      preflight.marketplacePackageAllowed !== false ||
      preflight.payoutEligible !== false ||
      preflight.reviewCleared !== false ||
      preflight.effectsApplied !== false
    ) {
      return yield* privacyError(
        "externalRepoStudyPrivacyReview.claimGates",
        "privacy review preflight must not clear a review or grant customer, marketplace, payout, or settlement claims",
      );
    }

    if (preflight.clearanceGate.effectsApplied !== false) {
      return yield* privacyError(
        "externalRepoStudyPrivacyReview.clearanceGate.effectsApplied",
        "clearance gate must never apply a real effect",
      );
    }

    if (preflight.state === "review_ready_held") {
      if (
        !preflight.dataProcessingAgreementPresent ||
        !preflight.retentionPolicyPresent ||
        !preflight.customerAuthorizationPresent ||
        !preflight.retentionWithinCap ||
        !preflight.declaredPiiCategoriesWithinPolicy
      ) {
        return yield* privacyError(
          "externalRepoStudyPrivacyReview.state",
          "review_ready_held requires every preflight check (DPA, retention policy, customer authorization, retention window, declared PII categories) to pass",
        );
      }
      if (preflight.privacyReviewRef === null) {
        return yield* privacyError(
          "externalRepoStudyPrivacyReview.privacyReviewRef",
          "review_ready_held must derive a privacyReviewRef",
        );
      }
    }

    if (preflight.state === "blocked" && preflight.privacyReviewRef !== null) {
      return yield* privacyError(
        "externalRepoStudyPrivacyReview.privacyReviewRef",
        "a blocked review must not derive a privacyReviewRef",
      );
    }

    if (
      preflight.wouldClearWhenArmed &&
      preflight.clearanceGate.state !== "armed_ready"
    ) {
      return yield* privacyError(
        "externalRepoStudyPrivacyReview.wouldClearWhenArmed",
        "a review can only be marked would-clear-when-armed once the armed gate is ready",
      );
    }

    if (
      preflight.preflightHash !==
      openAgentsExternalRepoStudyPrivacyReviewPreflightHash(preflight)
    ) {
      return yield* privacyError(
        "externalRepoStudyPrivacyReview.preflightHash",
        "must match the deterministic preflight hash",
      );
    }
  });
}

function requireNonEmpty(
  value: string,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.trim().length === 0
    ? privacyError(path, "must be non-empty")
    : Effect.void;
}

function requireSha256(
  value: string,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.startsWith("sha256:")
    ? Effect.void
    : privacyError(path, "must be a sha256 ref");
}

function slugRepo(repo: string): string {
  return repo
    .replace(/[^a-zA-Z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .toLowerCase()
    .slice(0, 80);
}

function privacyError(
  path: string,
  reason: string,
): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}

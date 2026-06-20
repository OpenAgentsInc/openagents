import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import {
  isPublishedExternalRepoStudyPrivacyPolicyRef,
  type OpenAgentsExternalRepoStudyPrivacyPolicyRegistry,
} from "./external-repo-studying-privacy-policy-registry";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

/**
 * Data-subject request (DSR) preflight for the external-repo-studying pilot.
 *
 * This module is the smallest genuine piece of the missing privacy-policy
 * control surface for autopilot.external_repo_studying_pilot.v1
 * (blocker.product_promises.external_repo_studying_privacy_policy_missing).
 *
 * The published privacy policy (docs/legal/external-repo-studying-privacy-policy.v0.md,
 * Section 6 "Data subject rights") PROMISES that a customer may request access to,
 * rectification of, or deletion of artifacts derived from their authorized study,
 * and may withdraw authorization. Until now that promise was published text with
 * NO operational control surface: nothing decided whether such a request would be
 * honoured, and nothing tied a request to a KNOWN published policy. A privacy
 * policy that grants data-subject rights but cannot intake or evaluate a request
 * is incomplete.
 *
 * This preflight closes that gap. It evaluates a data-subject REQUEST expressed
 * as REFS and ENUMS ONLY (a request ref, an opaque data-subject ref, the request
 * type, the customer-authorization ref, and the governing policy ref), checks the
 * policy ref against a KNOWN published version in the supplied registry (reusing
 * isPublishedExternalRepoStudyPrivacyPolicyRef — the same forgeable-string seam
 * the review<->policy binding closed elsewhere), and decides whether the request
 * WOULD be admitted for fulfilment. It performs NO real fulfilment: it exports no
 * data, erases no artifact, and withdraws no authorization.
 *
 * Hard rules (enforced by the schema + validator):
 *  - Refs/enums ONLY. sourceBoundary = "customer_refs_withheld". No PII value,
 *    subject identity, repository content, or handler notes ever cross here — only
 *    an opaque subject REF.
 *  - INERT by construction: requestHonored, dataExported, dataErased,
 *    authorizationWithdrawn, and effectsApplied are ALWAYS false. The module
 *    computes WHETHER a request WOULD be fulfilled when the flag is armed and every
 *    gate passes (wouldFulfillWhenArmed), but never fulfils a real request.
 *  - Flag-gated default-OFF: the fulfilment flag
 *    (EXTERNAL_REPO_STUDY_DATA_SUBJECT_REQUEST_ENABLED) defaults disabled. With no
 *    flag the fulfilment gate resolves inert_disabled.
 *  - No claim widening: customerPublicClaimAllowed, marketplacePackageAllowed,
 *    and payoutEligible are always false here.
 *
 * Green for the broader promise still requires a real, human/legal DSR fulfilment
 * process backed by durable, access-controlled storage that can actually export
 * or erase derived artifacts, and an owner-signed armed run with a dereferenceable
 * closeout receipt per proof.claim_upgrade_receipts.v1 — all owner-gated and out
 * of scope for the pure preflight built here.
 */
export const OPENAGENTS_EXTERNAL_REPO_STUDY_DATA_SUBJECT_REQUEST_PREFLIGHT_SCHEMA_REF =
  "openagents.external_repo_study_data_subject_request_preflight.v0" as const;

export const ExternalRepoStudyDataSubjectRequestFlagName =
  "EXTERNAL_REPO_STUDY_DATA_SUBJECT_REQUEST_ENABLED" as const;

/**
 * The closed set of data-subject request types the policy supports (Section 6 of
 * the published privacy policy). Any type outside this set blocks the request.
 */
export const ExternalRepoStudyDataSubjectRequestType = S.Literals([
  "access",
  "rectification",
  "erasure",
  "authorization_withdrawal",
]);
export type ExternalRepoStudyDataSubjectRequestType =
  typeof ExternalRepoStudyDataSubjectRequestType.Type;

const ALLOWED_REQUEST_TYPES: ReadonlySet<string> = new Set([
  "access",
  "rectification",
  "erasure",
  "authorization_withdrawal",
]);

export const OpenAgentsExternalRepoStudyDataSubjectRequestGateState = S.Literals([
  // No flag, default posture: preflight evaluated, held inert.
  "inert_disabled",
  // Flag armed but a required gate (preflight/handler-signoff) is unmet.
  "armed_blocked",
  // Flag armed and every gate passed; fulfilment permitted by policy, but this
  // module STILL applies no real effect (effectsApplied is always false).
  "armed_ready",
]);
export type OpenAgentsExternalRepoStudyDataSubjectRequestGateState =
  typeof OpenAgentsExternalRepoStudyDataSubjectRequestGateState.Type;

export const OpenAgentsExternalRepoStudyDataSubjectRequestState = S.Literals([
  "request_ready_held",
  "blocked",
]);
export type OpenAgentsExternalRepoStudyDataSubjectRequestState =
  typeof OpenAgentsExternalRepoStudyDataSubjectRequestState.Type;

/**
 * A data-subject request, expressed as refs and enums only. No PII value, subject
 * identity, repository content, or handler notes.
 */
export interface ExternalRepoStudyDataSubjectRequest {
  /** Stable customer ref the study belongs to, e.g. customer.<id>.v0 */
  readonly customerRef: string;
  /** External (non-OpenAgents) repo slug, e.g. ExampleCorp/widget-service */
  readonly repo: string;
  /** Stable ref of the request ticket itself, e.g. dsr.<id>.v0 */
  readonly requestRef?: string;
  /** Opaque ref of the data subject (NOT a PII value), e.g. subject.<id>.v0 */
  readonly subjectRef?: string;
  /** The requested action (closed set). */
  readonly requestType: ExternalRepoStudyDataSubjectRequestType;
  /** Ref proving the customer authorized this study of their repo data. */
  readonly customerAuthorizationRef?: string;
}

export const OpenAgentsExternalRepoStudyDataSubjectRequestGate = S.Struct({
  blockedReasonRefs: S.Array(S.String),
  effectsApplied: S.Literal(false),
  flagName: S.Literal(ExternalRepoStudyDataSubjectRequestFlagName),
  handlerSignoffPresent: S.Boolean,
  state: OpenAgentsExternalRepoStudyDataSubjectRequestGateState,
});
export type OpenAgentsExternalRepoStudyDataSubjectRequestGate =
  typeof OpenAgentsExternalRepoStudyDataSubjectRequestGate.Type;

export const OpenAgentsExternalRepoStudyDataSubjectRequestPreflight = S.Struct({
  // Deterministic ref a real fulfilment would later attach a closeout receipt to.
  // Derived ONLY when the request would be admitted; null while blocked.
  acknowledgementRef: S.NullOr(S.String),
  authorizationWithdrawn: S.Literal(false),
  blockerRefs: S.Array(S.String),
  customerAuthorizationPresent: S.Boolean,
  customerPublicClaimAllowed: S.Literal(false),
  customerRef: S.String,
  dataErased: S.Literal(false),
  dataExported: S.Literal(false),
  effectsApplied: S.Literal(false),
  evidenceRefs: S.Array(S.String),
  fulfilmentGate: OpenAgentsExternalRepoStudyDataSubjectRequestGate,
  generatedAt: S.String,
  marketplacePackageAllowed: S.Literal(false),
  payoutEligible: S.Literal(false),
  policyPublished: S.Boolean,
  policyRef: S.String,
  policyRegistryRef: S.String,
  preflightHash: S.String,
  preflightRef: S.String,
  repo: S.String,
  requestHonored: S.Literal(false),
  requestRefPresent: S.Boolean,
  requestType: ExternalRepoStudyDataSubjectRequestType,
  requestTypeSupported: S.Boolean,
  safeCopy: S.String,
  schemaRef: S.Literal(
    OPENAGENTS_EXTERNAL_REPO_STUDY_DATA_SUBJECT_REQUEST_PREFLIGHT_SCHEMA_REF,
  ),
  sourceBoundary: S.Literal("customer_refs_withheld"),
  state: OpenAgentsExternalRepoStudyDataSubjectRequestState,
  subjectRefPresent: S.Boolean,
  unsafeCopyRefs: S.Array(S.String),
  wouldFulfillWhenArmed: S.Boolean,
});
export type OpenAgentsExternalRepoStudyDataSubjectRequestPreflight =
  typeof OpenAgentsExternalRepoStudyDataSubjectRequestPreflight.Type;

export interface BuildOpenAgentsExternalRepoStudyDataSubjectRequestPreflightInput {
  readonly generatedAt?: string;
  /**
   * Whether a request handler sign-off is recorded for an armed request.
   * Default false.
   */
  readonly handlerSignoffPresent?: boolean;
  /** The published policy registry the policy ref is checked against. */
  readonly policyRegistry: OpenAgentsExternalRepoStudyPrivacyPolicyRegistry;
  /**
   * The policy ref the study claims governs it. It must match a KNOWN published
   * version in the registry; otherwise the request is blocked.
   */
  readonly policyRef: string;
  /** The data-subject request, expressed as refs/enums only. */
  readonly request: ExternalRepoStudyDataSubjectRequest;
  /**
   * Whether the fulfilment flag is armed. Default false. Even when armed, this
   * module applies no real effect.
   */
  readonly requestFlagArmed?: boolean;
}

export function buildOpenAgentsExternalRepoStudyDataSubjectRequestPreflight(
  input: BuildOpenAgentsExternalRepoStudyDataSubjectRequestPreflightInput,
): Effect.Effect<
  OpenAgentsExternalRepoStudyDataSubjectRequestPreflight,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const registry = input.policyRegistry;
    const request = input.request;
    const repo = request.repo;
    const policyRef = input.policyRef;

    yield* requireNonEmpty(repo, "externalRepoStudyDataSubjectRequest.repo");
    yield* requireNonEmpty(
      request.customerRef,
      "externalRepoStudyDataSubjectRequest.customerRef",
    );

    if (repo === "OpenAgentsInc/openagents") {
      return yield* dsrError(
        "externalRepoStudyDataSubjectRequest.repo",
        "data-subject request target must be an external (non-OpenAgents) pilot repo",
      );
    }

    const policyPublished = isPublishedExternalRepoStudyPrivacyPolicyRef(
      registry,
      policyRef,
    );
    const requestRefPresent = (request.requestRef ?? "").trim().length > 0;
    const subjectRefPresent = (request.subjectRef ?? "").trim().length > 0;
    const customerAuthorizationPresent =
      (request.customerAuthorizationRef ?? "").trim().length > 0;
    const requestTypeSupported = ALLOWED_REQUEST_TYPES.has(request.requestType);

    const preflightPassed =
      policyPublished &&
      requestRefPresent &&
      subjectRefPresent &&
      customerAuthorizationPresent &&
      requestTypeSupported;

    const blockerRefs = buildDsrBlockerRefs({
      customerAuthorizationPresent,
      policyPublished,
      requestRefPresent,
      requestTypeSupported,
      subjectRefPresent,
    });

    const fulfilmentGate = buildDsrGate({
      handlerSignoffPresent: input.handlerSignoffPresent ?? false,
      preflightPassed,
      requestFlagArmed: input.requestFlagArmed ?? false,
    });

    const wouldFulfillWhenArmed =
      preflightPassed && fulfilmentGate.state === "armed_ready";

    const generatedAt =
      input.generatedAt ??
      "generated_at.withheld_for_stable_external_repo_study_data_subject_request_preflight_hash";

    const evidenceRefs = [
      request.customerRef,
      registry.registryRef,
      ...(policyPublished ? [policyRef] : []),
      ...(requestRefPresent && request.requestRef ? [request.requestRef] : []),
      ...(subjectRefPresent && request.subjectRef ? [request.subjectRef] : []),
      ...(customerAuthorizationPresent && request.customerAuthorizationRef
        ? [request.customerAuthorizationRef]
        : []),
      "docs/legal/external-repo-studying-privacy-policy.v0.md",
      "docs/research/machine-studying/openagents-studybench/private-boundary.md",
      "docs/launch/vertex-fleet/autopilot.external_repo_studying_pilot.v1.md",
    ];

    // The acknowledgementRef is the deterministic artifact a real fulfilment
    // would later attach a closeout receipt to. It is derived ONLY when the
    // request would be admitted; while blocked it is null so a blocked request
    // cannot be mistaken for an admitted one.
    const base: OpenAgentsExternalRepoStudyDataSubjectRequestPreflight = {
      acknowledgementRef: null,
      authorizationWithdrawn: false,
      blockerRefs,
      customerAuthorizationPresent,
      customerPublicClaimAllowed: false,
      customerRef: request.customerRef,
      dataErased: false,
      dataExported: false,
      effectsApplied: false,
      evidenceRefs,
      fulfilmentGate,
      generatedAt,
      marketplacePackageAllowed: false,
      payoutEligible: false,
      policyPublished,
      policyRef,
      policyRegistryRef: registry.registryRef,
      preflightHash: "sha256:pending",
      preflightRef: "external_repo_study_data_subject_request_preflight.pending",
      repo,
      requestHonored: false,
      requestRefPresent,
      requestType: request.requestType,
      requestTypeSupported,
      safeCopy:
        "Data-subject request preflight evaluated an external-repo study privacy request from refs and enums only (request ref, opaque subject ref, request type, customer-authorization ref, governing policy ref) against the published policy registry. The preflight is held inert; no data is exported, no artifact is erased, no authorization is withdrawn, and no customer, marketplace, payout, or settlement claim is made.",
      schemaRef:
        OPENAGENTS_EXTERNAL_REPO_STUDY_DATA_SUBJECT_REQUEST_PREFLIGHT_SCHEMA_REF,
      sourceBoundary: "customer_refs_withheld",
      state: preflightPassed ? "request_ready_held" : "blocked",
      subjectRefPresent,
      unsafeCopyRefs: [
        "blocked_claim.data_subject_request_fulfilled_live",
        "blocked_claim.customer_repo_data_processing_authorized",
        "blocked_claim.privacy_review_grants_ingestion",
        "blocked_claim.study_packet_marketplace_package",
        "blocked_claim.machine_studying_payout_eligible",
      ],
      wouldFulfillWhenArmed,
    };

    const withRef: OpenAgentsExternalRepoStudyDataSubjectRequestPreflight = {
      ...base,
      acknowledgementRef: preflightPassed
        ? `data_subject_request_ack.${slugRepo(repo)}.${request.requestType}.v0`
        : null,
    };

    const preflightHash =
      openAgentsExternalRepoStudyDataSubjectRequestPreflightHash(withRef);

    return yield* decodeOpenAgentsExternalRepoStudyDataSubjectRequestPreflight({
      ...withRef,
      preflightHash,
      preflightRef: `external_repo_study_data_subject_request_preflight.${slugRepo(repo)}.${shortHash(preflightHash)}`,
    });
  });
}

export function decodeOpenAgentsExternalRepoStudyDataSubjectRequestPreflight(
  value: unknown,
): Effect.Effect<
  OpenAgentsExternalRepoStudyDataSubjectRequestPreflight,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(
      value,
      "externalRepoStudyDataSubjectRequest",
    );
    const preflight = yield* S.decodeUnknownEffect(
      OpenAgentsExternalRepoStudyDataSubjectRequestPreflight,
    )(value).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBenchmarkContractError({
            path: "externalRepoStudyDataSubjectRequest",
            reason: String(error),
          }),
      ),
    );
    yield* validateExternalRepoStudyDataSubjectRequestPreflight(preflight);
    return preflight;
  });
}

export function openAgentsExternalRepoStudyDataSubjectRequestPreflightHash(
  preflight: OpenAgentsExternalRepoStudyDataSubjectRequestPreflight,
): string {
  const {
    preflightHash: _preflightHash,
    preflightRef: _preflightRef,
    generatedAt: _generatedAt,
    ...stable
  } = preflight;
  return sha256Ref(stableJson(stable));
}

function buildDsrGate(input: {
  readonly handlerSignoffPresent: boolean;
  readonly preflightPassed: boolean;
  readonly requestFlagArmed: boolean;
}): OpenAgentsExternalRepoStudyDataSubjectRequestGate {
  if (!input.requestFlagArmed) {
    return {
      blockedReasonRefs: [],
      effectsApplied: false,
      flagName: ExternalRepoStudyDataSubjectRequestFlagName,
      handlerSignoffPresent: input.handlerSignoffPresent,
      state: "inert_disabled",
    };
  }

  const blockedReasonRefs: string[] = [];
  if (!input.preflightPassed) {
    blockedReasonRefs.push(
      "data_subject_request.blocked.preflight_not_passed",
    );
  }
  if (!input.handlerSignoffPresent) {
    blockedReasonRefs.push(
      "data_subject_request.blocked.handler_signoff_missing",
    );
  }

  return {
    blockedReasonRefs,
    effectsApplied: false,
    flagName: ExternalRepoStudyDataSubjectRequestFlagName,
    handlerSignoffPresent: input.handlerSignoffPresent,
    state: blockedReasonRefs.length === 0 ? "armed_ready" : "armed_blocked",
  };
}

function buildDsrBlockerRefs(input: {
  readonly customerAuthorizationPresent: boolean;
  readonly policyPublished: boolean;
  readonly requestRefPresent: boolean;
  readonly requestTypeSupported: boolean;
  readonly subjectRefPresent: boolean;
}): ReadonlyArray<string> {
  const blockers: string[] = [];
  if (!input.policyPublished) {
    blockers.push(
      "blocker.external_repo_study_data_subject_request.policy_ref_not_published",
    );
  }
  if (!input.requestRefPresent) {
    blockers.push(
      "blocker.external_repo_study_data_subject_request.request_ref_missing",
    );
  }
  if (!input.subjectRefPresent) {
    blockers.push(
      "blocker.external_repo_study_data_subject_request.subject_ref_missing",
    );
  }
  if (!input.customerAuthorizationPresent) {
    blockers.push(
      "blocker.external_repo_study_data_subject_request.customer_authorization_missing",
    );
  }
  if (!input.requestTypeSupported) {
    blockers.push(
      "blocker.external_repo_study_data_subject_request.request_type_unsupported",
    );
  }
  return blockers;
}

function validateExternalRepoStudyDataSubjectRequestPreflight(
  preflight: OpenAgentsExternalRepoStudyDataSubjectRequestPreflight,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(
      preflight.repo,
      "externalRepoStudyDataSubjectRequest.repo",
    );
    yield* requireNonEmpty(
      preflight.customerRef,
      "externalRepoStudyDataSubjectRequest.customerRef",
    );
    yield* requireNonEmpty(
      preflight.preflightRef,
      "externalRepoStudyDataSubjectRequest.preflightRef",
    );
    yield* requireSha256(
      preflight.preflightHash,
      "externalRepoStudyDataSubjectRequest.preflightHash",
    );

    if (preflight.repo === "OpenAgentsInc/openagents") {
      return yield* dsrError(
        "externalRepoStudyDataSubjectRequest.repo",
        "data-subject request target must be an external (non-OpenAgents) pilot repo",
      );
    }

    if (
      preflight.customerPublicClaimAllowed !== false ||
      preflight.marketplacePackageAllowed !== false ||
      preflight.payoutEligible !== false ||
      preflight.requestHonored !== false ||
      preflight.dataExported !== false ||
      preflight.dataErased !== false ||
      preflight.authorizationWithdrawn !== false ||
      preflight.effectsApplied !== false
    ) {
      return yield* dsrError(
        "externalRepoStudyDataSubjectRequest.claimGates",
        "data-subject request preflight must not fulfil a request or grant customer, marketplace, payout, or settlement claims",
      );
    }

    if (preflight.fulfilmentGate.effectsApplied !== false) {
      return yield* dsrError(
        "externalRepoStudyDataSubjectRequest.fulfilmentGate.effectsApplied",
        "fulfilment gate must never apply a real effect",
      );
    }

    if (preflight.state === "request_ready_held") {
      if (
        !preflight.policyPublished ||
        !preflight.requestRefPresent ||
        !preflight.subjectRefPresent ||
        !preflight.customerAuthorizationPresent ||
        !preflight.requestTypeSupported
      ) {
        return yield* dsrError(
          "externalRepoStudyDataSubjectRequest.state",
          "request_ready_held requires a published policy ref, a request ref, a subject ref, a customer-authorization ref, and a supported request type",
        );
      }
      if (preflight.acknowledgementRef === null) {
        return yield* dsrError(
          "externalRepoStudyDataSubjectRequest.acknowledgementRef",
          "request_ready_held must derive an acknowledgementRef",
        );
      }
    }

    if (
      preflight.state === "blocked" &&
      preflight.acknowledgementRef !== null
    ) {
      return yield* dsrError(
        "externalRepoStudyDataSubjectRequest.acknowledgementRef",
        "a blocked request must not derive an acknowledgementRef",
      );
    }

    if (
      preflight.wouldFulfillWhenArmed &&
      preflight.fulfilmentGate.state !== "armed_ready"
    ) {
      return yield* dsrError(
        "externalRepoStudyDataSubjectRequest.wouldFulfillWhenArmed",
        "a request can only be marked would-fulfil-when-armed once the armed gate is ready",
      );
    }

    if (
      preflight.preflightHash !==
      openAgentsExternalRepoStudyDataSubjectRequestPreflightHash(preflight)
    ) {
      return yield* dsrError(
        "externalRepoStudyDataSubjectRequest.preflightHash",
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
    ? dsrError(path, "must be non-empty")
    : Effect.void;
}

function requireSha256(
  value: string,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.startsWith("sha256:")
    ? Effect.void
    : dsrError(path, "must be a sha256 ref");
}

function slugRepo(repo: string): string {
  return repo
    .replace(/[^a-zA-Z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .toLowerCase()
    .slice(0, 80);
}

function dsrError(
  path: string,
  reason: string,
): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}

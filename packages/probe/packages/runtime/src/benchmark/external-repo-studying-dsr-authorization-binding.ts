import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import {
  isActiveCustomerAuthorizationRef,
  type OpenAgentsExternalRepoStudyCustomerAuthorizationRegistry,
} from "./external-repo-studying-customer-authorization-registry";
import {
  buildOpenAgentsExternalRepoStudyDataSubjectRequestPreflight,
  OpenAgentsExternalRepoStudyDataSubjectRequestPreflight,
  type ExternalRepoStudyDataSubjectRequest,
} from "./external-repo-studying-data-subject-request";
import {
  type OpenAgentsExternalRepoStudyPrivacyPolicyRegistry,
} from "./external-repo-studying-privacy-policy-registry";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

/**
 * Data-subject-request (DSR) <-> customer-AUTHORIZATION BINDING for the
 * external-repo-studying pilot.
 *
 * This module is the smallest genuine piece of the missing privacy POLICY control
 * surface for autopilot.external_repo_studying_pilot.v1
 * (blocker.product_promises.external_repo_studying_privacy_policy_missing): it
 * closes the LAST forgeable-string seam in the data-subject-request preflight —
 * the lawful-basis `customerAuthorizationRef` — and is the exact next step the
 * customer-authorization registry update named ("a binding that DERIVES the
 * privacy-review / upload / DSR preflights' customerAuthorizationRef from a
 * registry-known ACTIVE authorization"). The review preflight's seam was closed
 * by the review<->authorization binding; this closes the DSR preflight's.
 *
 * Section 3 ("Lawful basis and authorization") of the published privacy policy
 * (docs/legal/external-repo-studying-privacy-policy.v0.md) states OpenAgents
 * processes a customer's external repo ONLY with the customer's recorded
 * authorization, and Section 6 ("Data subject rights") states a customer may
 * WITHDRAW it. The customer-authorization registry already exposes
 * `isActiveCustomerAuthorizationRef(...)` to verify a ref matches a KNOWN, ACTIVE
 * authorization for an exact (customerRef, repo). But nothing yet forced the DSR
 * preflight to consume only such a ref: its `customerAuthorizationPresent` gate
 * was still a plain string-presence check, so a forged, stale, or WITHDRAWN ref
 * passed.
 *
 * This composer removes that forgeability, mirroring the review<->authorization
 * binding. It derives the DSR request's `customerAuthorizationRef` FROM a
 * candidate ref that `isActiveCustomerAuthorizationRef` verifies as a
 * registry-known ACTIVE authorization covering the SAME customer + repo, then
 * builds the DSR preflight from it. An unknown / withdrawn / mismatched / empty
 * ref binds NO ref, so the DSR preflight blocks on
 * `blocker.external_repo_study_data_subject_request.customer_authorization_missing`
 * instead of trusting a string. The caller cannot inject its own authorization
 * ref (it is Omitted from the request type).
 *
 * Hard rules (enforced by the schema + validator):
 *  - Refs/enums ONLY. sourceBoundary = "customer_refs_withheld". No PII value,
 *    subject identity, repository content, or handler notes ever cross here.
 *  - INERT by construction: `requestHonored`, `dataExported`, `dataErased`,
 *    `authorizationWithdrawn`, and `effectsApplied` are ALWAYS false (inherited
 *    from the nested DSR preflight and asserted here). The binding decides only
 *    WHETHER the DSR's authorization gate is genuinely backed by an active
 *    recorded authorization; it never fulfils a request, exports or erases data,
 *    withdraws an authorization, ingests, stores bytes, sends, settles, or spends.
 *  - No claim widening: customerPublicClaimAllowed, marketplacePackageAllowed,
 *    and payoutEligible are always false here.
 *
 * Green for the broader promise still requires a real, human/legal DSR fulfilment
 * process backed by durable, access-controlled storage that can actually export
 * or erase derived artifacts, real revocation enforcement, and an owner-signed
 * armed run with a dereferenceable closeout receipt per
 * proof.claim_upgrade_receipts.v1 — all owner-gated and out of scope for the pure
 * binding built here.
 */
export const OPENAGENTS_EXTERNAL_REPO_STUDY_DSR_AUTHORIZATION_BINDING_SCHEMA_REF =
  "openagents.external_repo_study_dsr_authorization_binding.v0" as const;

export const OpenAgentsExternalRepoStudyDsrAuthorizationBindingState = S.Literals(
  [
    // The DSR's authorization ref is derived from a known active authorization.
    "bound_held",
    // The candidate ref is unknown / withdrawn / mismatched / empty; none bound.
    "unbound",
  ],
);
export type OpenAgentsExternalRepoStudyDsrAuthorizationBindingState =
  typeof OpenAgentsExternalRepoStudyDsrAuthorizationBindingState.Type;

export const OpenAgentsExternalRepoStudyDsrAuthorizationBinding = S.Struct({
  authorizationActive: S.Boolean,
  authorizationCandidateRef: S.String,
  authorizationRef: S.NullOr(S.String),
  authorizationRegistryRef: S.String,
  authorizationWithdrawn: S.Literal(false),
  bindingHash: S.String,
  bindingRef: S.String,
  bound: S.Boolean,
  customerPublicClaimAllowed: S.Literal(false),
  customerRef: S.String,
  dataErased: S.Literal(false),
  dataExported: S.Literal(false),
  // The full DSR preflight built from the (only-if-bound) derived ref.
  dsrPreflight: OpenAgentsExternalRepoStudyDataSubjectRequestPreflight,
  effectsApplied: S.Literal(false),
  evidenceRefs: S.Array(S.String),
  generatedAt: S.String,
  marketplacePackageAllowed: S.Literal(false),
  mismatchRefs: S.Array(S.String),
  payoutEligible: S.Literal(false),
  repo: S.String,
  requestHonored: S.Literal(false),
  safeCopy: S.String,
  schemaRef: S.Literal(
    OPENAGENTS_EXTERNAL_REPO_STUDY_DSR_AUTHORIZATION_BINDING_SCHEMA_REF,
  ),
  sourceBoundary: S.Literal("customer_refs_withheld"),
  state: OpenAgentsExternalRepoStudyDsrAuthorizationBindingState,
  unsafeCopyRefs: S.Array(S.String),
  wouldFulfillWhenArmed: S.Boolean,
});
export type OpenAgentsExternalRepoStudyDsrAuthorizationBinding =
  typeof OpenAgentsExternalRepoStudyDsrAuthorizationBinding.Type;

export interface BuildOpenAgentsExternalRepoStudyDsrAuthorizationBindingInput {
  /** The customer-authorization registry the candidate ref is checked against. */
  readonly authorizationRegistry: OpenAgentsExternalRepoStudyCustomerAuthorizationRegistry;
  /**
   * The authorization ref the request claims is its lawful basis. It must match a
   * KNOWN, ACTIVE authorization in the registry for the request's customer+repo;
   * otherwise no authorization ref is derived.
   */
  readonly authorizationCandidateRef: string;
  readonly generatedAt?: string;
  /**
   * Whether a request handler sign-off is recorded for an armed request.
   * Forwarded to the nested DSR preflight. Default false.
   */
  readonly handlerSignoffPresent?: boolean;
  /** The published policy registry the policy ref is checked against. */
  readonly policyRegistry: OpenAgentsExternalRepoStudyPrivacyPolicyRegistry;
  /**
   * The policy ref the request claims governs it. Forwarded to the nested DSR
   * preflight, which checks it against a KNOWN published version.
   */
  readonly policyRef: string;
  /**
   * The data-subject request, MINUS its customerAuthorizationRef. The binding
   * derives that from the registry; callers cannot inject their own.
   */
  readonly request: Omit<
    ExternalRepoStudyDataSubjectRequest,
    "customerAuthorizationRef"
  >;
  /**
   * Whether the fulfilment flag is armed. Forwarded to the nested DSR preflight.
   * Even when armed, no real effect is applied. Default false.
   */
  readonly requestFlagArmed?: boolean;
}

export function buildOpenAgentsExternalRepoStudyDsrAuthorizationBinding(
  input: BuildOpenAgentsExternalRepoStudyDsrAuthorizationBindingInput,
): Effect.Effect<
  OpenAgentsExternalRepoStudyDsrAuthorizationBinding,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const registry = input.authorizationRegistry;
    const req = input.request;
    const candidateRef = input.authorizationCandidateRef;

    yield* requireNonEmpty(
      req.repo,
      "externalRepoStudyDsrAuthorizationBinding.repo",
    );
    yield* requireNonEmpty(
      req.customerRef,
      "externalRepoStudyDsrAuthorizationBinding.customerRef",
    );

    if (req.repo === "OpenAgentsInc/openagents") {
      return yield* bindingError(
        "externalRepoStudyDsrAuthorizationBinding.repo",
        "DSR authorization binding target must be an external (non-OpenAgents) pilot repo",
      );
    }

    const authorizationActive = isActiveCustomerAuthorizationRef(
      registry,
      candidateRef,
      { customerRef: req.customerRef, repo: req.repo },
    );

    const matched = authorizationActive
      ? (registry.recordedAuthorizations.find(
          (authorization) => authorization.authorizationRef === candidateRef,
        ) ?? null)
      : null;

    const bound = authorizationActive && matched !== null;

    // The authorization ref is derived ONLY from a known active authorization.
    // The caller never supplies it directly, so the DSR's authorization gate can
    // only be satisfied by a registry-verified active authorization.
    const authorizationRef = bound ? matched!.authorizationRef : null;

    const dsrPreflight =
      yield* buildOpenAgentsExternalRepoStudyDataSubjectRequestPreflight({
        policyRegistry: input.policyRegistry,
        policyRef: input.policyRef,
        request: {
          ...req,
          customerAuthorizationRef: authorizationRef ?? undefined,
        },
        handlerSignoffPresent: input.handlerSignoffPresent ?? false,
        requestFlagArmed: input.requestFlagArmed ?? false,
      });

    const mismatchRefs = authorizationActive
      ? []
      : [
          "blocker.external_repo_study_dsr_authorization_binding.authorization_not_active",
        ];

    const generatedAt =
      input.generatedAt ??
      "generated_at.withheld_for_stable_external_repo_study_dsr_authorization_binding_hash";

    const evidenceRefs = [
      req.customerRef,
      registry.registryRef,
      dsrPreflight.preflightRef,
      ...(bound
        ? [matched!.authorizationRef, matched!.authorizationDigest]
        : []),
      "docs/legal/external-repo-studying-privacy-policy.v0.md",
      "docs/research/machine-studying/openagents-studybench/private-boundary.md",
      "docs/launch/vertex-fleet/autopilot.external_repo_studying_pilot.v1.md",
    ];

    const base: OpenAgentsExternalRepoStudyDsrAuthorizationBinding = {
      authorizationActive,
      authorizationCandidateRef: candidateRef,
      authorizationRef,
      authorizationRegistryRef: registry.registryRef,
      authorizationWithdrawn: false,
      bindingHash: "sha256:pending",
      bindingRef: "external_repo_study_dsr_authorization_binding.pending",
      bound,
      customerPublicClaimAllowed: false,
      customerRef: req.customerRef,
      dataErased: false,
      dataExported: false,
      dsrPreflight,
      effectsApplied: false,
      evidenceRefs,
      generatedAt,
      marketplacePackageAllowed: false,
      mismatchRefs,
      payoutEligible: false,
      repo: req.repo,
      requestHonored: false,
      safeCopy:
        "Data-subject-request authorization binding cross-checked a privacy request against the customer-authorization registry using refs and enums only. It derives the request's customer-authorization ref from a known active authorization for the same customer and repo; an unknown, withdrawn, mismatched, or empty ref binds nothing, so the request blocks on a missing customer authorization. The binding is held inert; no customer data is processed, no request is fulfilled, no data is exported or erased, no authorization is withdrawn, and no customer, marketplace, payout, or settlement claim is made.",
      schemaRef:
        OPENAGENTS_EXTERNAL_REPO_STUDY_DSR_AUTHORIZATION_BINDING_SCHEMA_REF,
      sourceBoundary: "customer_refs_withheld",
      state: bound ? "bound_held" : "unbound",
      unsafeCopyRefs: [
        "blocked_claim.dsr_authorization_forgeable_string",
        "blocked_claim.data_subject_request_fulfilled_live",
        "blocked_claim.customer_repo_data_processing_authorized",
        "blocked_claim.study_packet_marketplace_package",
        "blocked_claim.machine_studying_payout_eligible",
      ],
      wouldFulfillWhenArmed: dsrPreflight.wouldFulfillWhenArmed,
    };

    const bindingHash =
      openAgentsExternalRepoStudyDsrAuthorizationBindingHash(base);

    return yield* decodeOpenAgentsExternalRepoStudyDsrAuthorizationBinding({
      ...base,
      bindingHash,
      bindingRef: `external_repo_study_dsr_authorization_binding.${slugRepo(req.repo)}.${shortHash(bindingHash)}`,
    });
  });
}

export function decodeOpenAgentsExternalRepoStudyDsrAuthorizationBinding(
  value: unknown,
): Effect.Effect<
  OpenAgentsExternalRepoStudyDsrAuthorizationBinding,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(
      value,
      "externalRepoStudyDsrAuthorizationBinding",
    );
    const binding = yield* S.decodeUnknownEffect(
      OpenAgentsExternalRepoStudyDsrAuthorizationBinding,
    )(value).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBenchmarkContractError({
            path: "externalRepoStudyDsrAuthorizationBinding",
            reason: String(error),
          }),
      ),
    );
    yield* validateExternalRepoStudyDsrAuthorizationBinding(binding);
    return binding;
  });
}

export function openAgentsExternalRepoStudyDsrAuthorizationBindingHash(
  binding: OpenAgentsExternalRepoStudyDsrAuthorizationBinding,
): string {
  const {
    bindingHash: _bindingHash,
    bindingRef: _bindingRef,
    generatedAt: _generatedAt,
    ...stable
  } = binding;
  return sha256Ref(stableJson(stable));
}

function validateExternalRepoStudyDsrAuthorizationBinding(
  binding: OpenAgentsExternalRepoStudyDsrAuthorizationBinding,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(
      binding.repo,
      "externalRepoStudyDsrAuthorizationBinding.repo",
    );
    yield* requireNonEmpty(
      binding.customerRef,
      "externalRepoStudyDsrAuthorizationBinding.customerRef",
    );
    yield* requireNonEmpty(
      binding.bindingRef,
      "externalRepoStudyDsrAuthorizationBinding.bindingRef",
    );
    yield* requireSha256(
      binding.bindingHash,
      "externalRepoStudyDsrAuthorizationBinding.bindingHash",
    );

    if (binding.repo === "OpenAgentsInc/openagents") {
      return yield* bindingError(
        "externalRepoStudyDsrAuthorizationBinding.repo",
        "DSR authorization binding target must be an external (non-OpenAgents) pilot repo",
      );
    }

    if (
      binding.customerPublicClaimAllowed !== false ||
      binding.marketplacePackageAllowed !== false ||
      binding.payoutEligible !== false ||
      binding.requestHonored !== false ||
      binding.dataExported !== false ||
      binding.dataErased !== false ||
      binding.authorizationWithdrawn !== false ||
      binding.effectsApplied !== false
    ) {
      return yield* bindingError(
        "externalRepoStudyDsrAuthorizationBinding.claimGates",
        "DSR authorization binding must not fulfil a request or grant customer, marketplace, payout, or settlement claims",
      );
    }

    // The nested DSR preflight is the source of truth for inertness; assert it
    // never escalates a claim through the binding.
    if (
      binding.dsrPreflight.requestHonored !== false ||
      binding.dsrPreflight.dataExported !== false ||
      binding.dsrPreflight.dataErased !== false ||
      binding.dsrPreflight.authorizationWithdrawn !== false ||
      binding.dsrPreflight.effectsApplied !== false
    ) {
      return yield* bindingError(
        "externalRepoStudyDsrAuthorizationBinding.dsrPreflight",
        "nested DSR preflight must remain inert",
      );
    }

    if (binding.bound !== (binding.state === "bound_held")) {
      return yield* bindingError(
        "externalRepoStudyDsrAuthorizationBinding.bound",
        "bound flag must agree with the binding state",
      );
    }

    if (binding.state === "bound_held") {
      if (
        !binding.authorizationActive ||
        binding.authorizationRef === null ||
        !binding.dsrPreflight.customerAuthorizationPresent
      ) {
        return yield* bindingError(
          "externalRepoStudyDsrAuthorizationBinding.state",
          "bound_held requires an active authorization, a derived authorization ref, and the DSR preflight to record it present",
        );
      }
      // The derived ref must be exactly the candidate ref that verified.
      if (binding.authorizationRef !== binding.authorizationCandidateRef) {
        return yield* bindingError(
          "externalRepoStudyDsrAuthorizationBinding.derivedRef",
          "derived authorization ref must reference the verified candidate authorization",
        );
      }
    }

    if (binding.state === "unbound") {
      if (binding.authorizationRef !== null) {
        return yield* bindingError(
          "externalRepoStudyDsrAuthorizationBinding.derivedRef",
          "an unbound binding must not derive an authorization ref",
        );
      }
      // No forged string can have slipped through, because the binding controls
      // the authorization ref: an unbound DSR cannot present it.
      if (binding.dsrPreflight.customerAuthorizationPresent) {
        return yield* bindingError(
          "externalRepoStudyDsrAuthorizationBinding.dsrPreflight",
          "an unbound binding must not leave the DSR authorization gate satisfied",
        );
      }
    }

    if (
      binding.wouldFulfillWhenArmed &&
      binding.dsrPreflight.fulfilmentGate.state !== "armed_ready"
    ) {
      return yield* bindingError(
        "externalRepoStudyDsrAuthorizationBinding.wouldFulfillWhenArmed",
        "would-fulfil-when-armed requires the nested DSR's armed gate to be ready",
      );
    }

    if (
      binding.bindingHash !==
      openAgentsExternalRepoStudyDsrAuthorizationBindingHash(binding)
    ) {
      return yield* bindingError(
        "externalRepoStudyDsrAuthorizationBinding.bindingHash",
        "must match the deterministic binding hash",
      );
    }
  });
}

function requireNonEmpty(
  value: string,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.trim().length === 0
    ? bindingError(path, "must be non-empty")
    : Effect.void;
}

function requireSha256(
  value: string,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.startsWith("sha256:")
    ? Effect.void
    : bindingError(path, "must be a sha256 ref");
}

function slugRepo(repo: string): string {
  return repo
    .replace(/[^a-zA-Z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .toLowerCase()
    .slice(0, 80);
}

function bindingError(
  path: string,
  reason: string,
): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}

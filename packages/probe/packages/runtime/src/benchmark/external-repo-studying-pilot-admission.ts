import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import {
  buildOpenAgentsCustomerPrivateValidation,
  type CustomerPrivateHoldoutCommitment,
  type OpenAgentsCustomerPrivateValidationGateState,
  type OpenAgentsCustomerPrivateValidationVerdict,
} from "./openagents-customer-private-validation";
import {
  type BuildOpenAgentsExternalRepoStudyPilotResult,
} from "./external-repo-studying-product";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

/**
 * Customer-private admission for the external-repo-studying pilot.
 *
 * This module is the missing customer-private admission path for
 * autopilot.external_repo_studying_pilot.v1
 * (blocker.product_promises.external_repo_studying_customer_private_admission_missing):
 * it decides whether an EXTERNAL contributor's study of a non-OpenAgents repo
 * may be ADMITTED into the pilot FOR A CUSTOMER, after the study has been
 * validated PRIVATELY against the customer's committed private holdout.
 *
 * It does NOT rebuild private validation. It REUSES the sibling module
 * (buildOpenAgentsCustomerPrivateValidation / planCustomerPrivateValidationDelivery)
 * as the private validation engine, and layers an admission decision on top.
 *
 * Hard rules (enforced by the schema + validator):
 *  - Refs/digests/counts ONLY. The customer's private holdout is admitted as a
 *    refs-withheld commitment; raw private task text, gold answers, rubric
 *    claims, and evidence excerpts are NEVER accepted and NEVER appear in the
 *    public projection (sourceBoundary = "customer_refs_withheld").
 *  - INERT by construction: `admitted` is ALWAYS false from this module. It
 *    computes WHETHER a study WOULD be admittable when the admission flag is
 *    armed and every gate passes, but it never admits a real external
 *    contributor's study into a real customer's pilot, delivers a packet, marks
 *    anything claimable, sends, settles, or spends. effectsApplied is always
 *    false.
 *  - Flag-gated default-OFF: the admission flag
 *    (EXTERNAL_REPO_STUDY_PILOT_ADMISSION_ENABLED) defaults disabled. With no
 *    flag the admission gate resolves inert_disabled.
 *  - No claim widening: customerPublicClaimAllowed, marketplacePackageAllowed,
 *    and payoutEligible are always false here.
 *
 * Green for the broader promise still requires real customer-data privacy
 * review, an armed admission with a dereferenceable closeout receipt, self-serve
 * upload controls, marketplace metering, pricing, payout eligibility,
 * settlement, and owner sign-off per proof.claim_upgrade_receipts.v1 — all
 * owner-gated and out of scope for the pure admission path built here.
 */
export const OPENAGENTS_EXTERNAL_REPO_STUDY_PILOT_ADMISSION_SCHEMA_REF =
  "openagents.external_repo_study_pilot_admission.v0" as const;

export const ExternalRepoStudyPilotAdmissionFlagName =
  "EXTERNAL_REPO_STUDY_PILOT_ADMISSION_ENABLED" as const;

export const OpenAgentsExternalRepoStudyPilotAdmissionGateState = S.Literals([
  // No flag, default posture: admission evaluated, held inert.
  "inert_disabled",
  // Flag armed but a required gate (validation/contributor/owner) is unmet.
  "armed_blocked",
  // Flag armed and every gate passed; admission permitted by policy, but this
  // module STILL applies no real effect (effectsApplied is always false).
  "armed_ready",
]);
export type OpenAgentsExternalRepoStudyPilotAdmissionGateState =
  typeof OpenAgentsExternalRepoStudyPilotAdmissionGateState.Type;

export const OpenAgentsExternalRepoStudyPilotAdmissionState = S.Literals([
  "admittable_held",
  "blocked",
]);
export type OpenAgentsExternalRepoStudyPilotAdmissionState =
  typeof OpenAgentsExternalRepoStudyPilotAdmissionState.Type;

/**
 * The external contributor proposing a study for customer admission, expressed
 * as refs only. No contributor PII, contact, key material, or private content.
 */
export interface ExternalRepoStudyContributorRef {
  /** Stable contributor ref, e.g. contributor.pylon.<id>.v0 */
  readonly contributorRef: string;
  /**
   * Whether the contributor accepted the pilot terms (refs-only, no-leak,
   * inert) as a recorded boolean. Default false.
   */
  readonly pilotTermsAccepted?: boolean;
}

/**
 * The customer the study is being admitted FOR, expressed as a ref only.
 */
export interface ExternalRepoStudyCustomerRef {
  /** Stable customer ref, e.g. customer.<id>.v0 */
  readonly customerRef: string;
}

export const OpenAgentsExternalRepoStudyPilotAdmissionGate = S.Struct({
  blockedReasonRefs: S.Array(S.String),
  effectsApplied: S.Literal(false),
  flagName: S.Literal(ExternalRepoStudyPilotAdmissionFlagName),
  ownerSignoffPresent: S.Boolean,
  state: OpenAgentsExternalRepoStudyPilotAdmissionGateState,
});
export type OpenAgentsExternalRepoStudyPilotAdmissionGate =
  typeof OpenAgentsExternalRepoStudyPilotAdmissionGate.Type;

export const OpenAgentsExternalRepoStudyPilotAdmission = S.Struct({
  admissionGate: OpenAgentsExternalRepoStudyPilotAdmissionGate,
  admissionHash: S.String,
  admissionRef: S.String,
  admitted: S.Literal(false),
  blockerRefs: S.Array(S.String),
  contributorRef: S.String,
  contributorTermsAccepted: S.Boolean,
  customerPublicClaimAllowed: S.Literal(false),
  customerRef: S.String,
  effectsApplied: S.Literal(false),
  evidenceRefs: S.Array(S.String),
  generatedAt: S.String,
  marketplacePackageAllowed: S.Literal(false),
  payoutEligible: S.Literal(false),
  privateValidationPassed: S.Boolean,
  productSurfaceHash: S.String,
  productSurfaceRef: S.String,
  repo: S.String,
  safeCopy: S.String,
  schemaRef: S.Literal(OPENAGENTS_EXTERNAL_REPO_STUDY_PILOT_ADMISSION_SCHEMA_REF),
  sourceBoundary: S.Literal("customer_refs_withheld"),
  state: OpenAgentsExternalRepoStudyPilotAdmissionState,
  unsafeCopyRefs: S.Array(S.String),
  validationVerdictHash: S.String,
  validationVerdictRef: S.String,
  wouldAdmitWhenArmed: S.Boolean,
});
export type OpenAgentsExternalRepoStudyPilotAdmission =
  typeof OpenAgentsExternalRepoStudyPilotAdmission.Type;

export interface BuildOpenAgentsExternalRepoStudyPilotAdmissionInput {
  /**
   * Whether the admission flag is armed. Default false. Even when armed, this
   * module applies no real effect (admitted/effectsApplied stay false).
   */
  readonly admissionFlagArmed?: boolean;
  readonly contributor: ExternalRepoStudyContributorRef;
  readonly customer: ExternalRepoStudyCustomerRef;
  readonly generatedAt?: string;
  /** The customer's committed private holdout, refs/digests/counts only. */
  readonly holdout: CustomerPrivateHoldoutCommitment;
  /**
   * Whether an owner sign-off (per proof.claim_upgrade_receipts.v1) is recorded
   * for an armed admission. Default false.
   */
  readonly ownerSignoffPresent?: boolean;
  /** The external-repo-studying pilot result for the contributor's study. */
  readonly pilot: BuildOpenAgentsExternalRepoStudyPilotResult;
}

export interface BuildOpenAgentsExternalRepoStudyPilotAdmissionResult {
  readonly admission: OpenAgentsExternalRepoStudyPilotAdmission;
  readonly validationVerdict: OpenAgentsCustomerPrivateValidationVerdict;
}

export function buildOpenAgentsExternalRepoStudyPilotAdmission(
  input: BuildOpenAgentsExternalRepoStudyPilotAdmissionInput,
): Effect.Effect<
  BuildOpenAgentsExternalRepoStudyPilotAdmissionResult,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const repo = input.pilot.productSurface.repo;
    yield* requireNonEmpty(repo, "externalRepoStudyPilotAdmission.repo");
    yield* requireNonEmpty(
      input.contributor.contributorRef,
      "externalRepoStudyPilotAdmission.contributorRef",
    );
    yield* requireNonEmpty(
      input.customer.customerRef,
      "externalRepoStudyPilotAdmission.customerRef",
    );

    if (repo === "OpenAgentsInc/openagents") {
      return yield* admissionError(
        "externalRepoStudyPilotAdmission.repo",
        "admission target must be an external (non-OpenAgents) pilot repo",
      );
    }

    // REUSE the sibling customer-private validation engine as the private
    // validation step. We never arm the validation delivery flag here: this
    // module's admission gate is the only arming surface, and it is inert.
    const validationVerdict = yield* buildOpenAgentsCustomerPrivateValidation({
      evalReport: input.pilot.evalReport,
      generatedAt: input.generatedAt,
      graph: input.pilot.graph,
      holdout: input.holdout,
      packet: input.pilot.packet,
      repo,
      verification: input.pilot.verification,
    });

    const privateValidationPassed = validationVerdict.state === "validated_held";
    const contributorTermsAccepted = input.contributor.pilotTermsAccepted ?? false;
    const pilotSurfacePilotReady = input.pilot.productSurface.state === "pilot_ready";

    const admissionPassed =
      privateValidationPassed &&
      contributorTermsAccepted &&
      pilotSurfacePilotReady;

    const blockerRefs = buildAdmissionBlockerRefs({
      contributorTermsAccepted,
      pilotSurfacePilotReady,
      privateValidationPassed,
    });

    const admissionGate = buildAdmissionGate({
      admissionFlagArmed: input.admissionFlagArmed ?? false,
      admissionPassed,
      ownerSignoffPresent: input.ownerSignoffPresent ?? false,
    });

    const wouldAdmitWhenArmed =
      admissionPassed && admissionGate.state === "armed_ready";

    const generatedAt =
      input.generatedAt ??
      "generated_at.withheld_for_stable_external_repo_study_pilot_admission_hash";

    const evidenceRefs = [
      input.pilot.productSurface.productSurfaceRef,
      input.pilot.packet.packetRef,
      input.pilot.graph.graphRef,
      input.pilot.verification.verificationRef,
      input.pilot.evalReport.reportRef,
      input.pilot.coderContext.contextPackRef,
      validationVerdict.verdictRef,
      input.holdout.splitRef,
      input.holdout.datasetRef,
      input.holdout.checksumRef,
      input.contributor.contributorRef,
      "docs/research/machine-studying/openagents-studybench/private-boundary.md",
      "docs/research/machine-studying/2026-06-17-tassadar-openagents-repo-studying-roadmap.md#phase-6",
    ];

    const base: OpenAgentsExternalRepoStudyPilotAdmission = {
      admissionGate,
      admissionHash: "sha256:pending",
      admissionRef: "external_repo_study_pilot_admission.pending",
      admitted: false,
      blockerRefs,
      contributorRef: input.contributor.contributorRef,
      contributorTermsAccepted,
      customerPublicClaimAllowed: false,
      customerRef: input.customer.customerRef,
      effectsApplied: false,
      evidenceRefs,
      generatedAt,
      marketplacePackageAllowed: false,
      payoutEligible: false,
      privateValidationPassed,
      productSurfaceHash: input.pilot.productSurface.productSurfaceHash,
      productSurfaceRef: input.pilot.productSurface.productSurfaceRef,
      repo,
      safeCopy:
        "External-repo studying admission evaluated an external contributor's study privately for a customer, against a committed private holdout (refs and digests only). The admission is held inert; no study is admitted into a real customer pilot, no packet is delivered or claimable, and no customer, marketplace, payout, or settlement claim is made.",
      schemaRef: OPENAGENTS_EXTERNAL_REPO_STUDY_PILOT_ADMISSION_SCHEMA_REF,
      sourceBoundary: "customer_refs_withheld",
      state: admissionPassed ? "admittable_held" : "blocked",
      unsafeCopyRefs: [
        "blocked_claim.customer_repo_studying_live",
        "blocked_claim.external_study_admitted_to_customer_pilot",
        "blocked_claim.study_packet_delivered_to_customer",
        "blocked_claim.study_packet_marketplace_package",
        "blocked_claim.machine_studying_payout_eligible",
      ],
      validationVerdictHash: validationVerdict.verdictHash,
      validationVerdictRef: validationVerdict.verdictRef,
      wouldAdmitWhenArmed,
    };

    const admissionHash = openAgentsExternalRepoStudyPilotAdmissionHash(base);

    const admission = yield* decodeOpenAgentsExternalRepoStudyPilotAdmission({
      ...base,
      admissionHash,
      admissionRef: `external_repo_study_pilot_admission.${slugRepo(repo)}.${shortHash(admissionHash)}`,
    });

    return { admission, validationVerdict };
  });
}

export function decodeOpenAgentsExternalRepoStudyPilotAdmission(
  value: unknown,
): Effect.Effect<
  OpenAgentsExternalRepoStudyPilotAdmission,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "externalRepoStudyPilotAdmission");
    const admission = yield* S.decodeUnknownEffect(OpenAgentsExternalRepoStudyPilotAdmission)(value).pipe(
      Effect.mapError((error) =>
        new ProbeBenchmarkContractError({
          path: "externalRepoStudyPilotAdmission",
          reason: String(error),
        }),
      ),
    );
    yield* validateExternalRepoStudyPilotAdmission(admission);
    return admission;
  });
}

export function openAgentsExternalRepoStudyPilotAdmissionHash(
  admission: OpenAgentsExternalRepoStudyPilotAdmission,
): string {
  const {
    admissionHash: _admissionHash,
    admissionRef: _admissionRef,
    generatedAt: _generatedAt,
    ...stable
  } = admission;
  return sha256Ref(stableJson(stable));
}

function buildAdmissionGate(input: {
  readonly admissionFlagArmed: boolean;
  readonly admissionPassed: boolean;
  readonly ownerSignoffPresent: boolean;
}): OpenAgentsExternalRepoStudyPilotAdmissionGate {
  if (!input.admissionFlagArmed) {
    return {
      blockedReasonRefs: [],
      effectsApplied: false,
      flagName: ExternalRepoStudyPilotAdmissionFlagName,
      ownerSignoffPresent: input.ownerSignoffPresent,
      state: "inert_disabled",
    };
  }

  const blockedReasonRefs: string[] = [];
  if (!input.admissionPassed) {
    blockedReasonRefs.push("admission.blocked.private_admission_gate_not_passed");
  }
  if (!input.ownerSignoffPresent) {
    blockedReasonRefs.push("admission.blocked.owner_signoff_missing");
  }

  return {
    blockedReasonRefs,
    effectsApplied: false,
    flagName: ExternalRepoStudyPilotAdmissionFlagName,
    ownerSignoffPresent: input.ownerSignoffPresent,
    state: blockedReasonRefs.length === 0 ? "armed_ready" : "armed_blocked",
  };
}

function buildAdmissionBlockerRefs(input: {
  readonly contributorTermsAccepted: boolean;
  readonly pilotSurfacePilotReady: boolean;
  readonly privateValidationPassed: boolean;
}): ReadonlyArray<string> {
  const blockers: string[] = [];
  if (!input.privateValidationPassed) {
    blockers.push("blocker.external_repo_study_pilot_admission.private_validation_not_passed");
  }
  if (!input.contributorTermsAccepted) {
    blockers.push("blocker.external_repo_study_pilot_admission.contributor_terms_not_accepted");
  }
  if (!input.pilotSurfacePilotReady) {
    blockers.push("blocker.external_repo_study_pilot_admission.pilot_surface_not_ready");
  }
  return blockers;
}

function validateExternalRepoStudyPilotAdmission(
  admission: OpenAgentsExternalRepoStudyPilotAdmission,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(admission.repo, "externalRepoStudyPilotAdmission.repo");
    yield* requireNonEmpty(admission.contributorRef, "externalRepoStudyPilotAdmission.contributorRef");
    yield* requireNonEmpty(admission.customerRef, "externalRepoStudyPilotAdmission.customerRef");
    yield* requireNonEmpty(admission.admissionRef, "externalRepoStudyPilotAdmission.admissionRef");
    yield* requireSha256(admission.admissionHash, "externalRepoStudyPilotAdmission.admissionHash");
    yield* requireSha256(admission.productSurfaceHash, "externalRepoStudyPilotAdmission.productSurfaceHash");
    yield* requireSha256(admission.validationVerdictHash, "externalRepoStudyPilotAdmission.validationVerdictHash");

    if (admission.repo === "OpenAgentsInc/openagents") {
      return yield* admissionError(
        "externalRepoStudyPilotAdmission.repo",
        "admission target must be an external (non-OpenAgents) pilot repo",
      );
    }

    if (
      admission.customerPublicClaimAllowed !== false ||
      admission.marketplacePackageAllowed !== false ||
      admission.payoutEligible !== false ||
      admission.admitted !== false ||
      admission.effectsApplied !== false
    ) {
      return yield* admissionError(
        "externalRepoStudyPilotAdmission.claimGates",
        "external-repo studying admission must not grant admission, customer, marketplace, payout, or settlement claims",
      );
    }

    if (admission.admissionGate.effectsApplied !== false) {
      return yield* admissionError(
        "externalRepoStudyPilotAdmission.admissionGate.effectsApplied",
        "admission gate must never apply a real effect",
      );
    }

    if (admission.state === "admittable_held") {
      if (
        !admission.privateValidationPassed ||
        !admission.contributorTermsAccepted
      ) {
        return yield* admissionError(
          "externalRepoStudyPilotAdmission.state",
          "admittable_held requires private validation to pass and the contributor to have accepted pilot terms",
        );
      }
    }

    if (admission.wouldAdmitWhenArmed && admission.admissionGate.state !== "armed_ready") {
      return yield* admissionError(
        "externalRepoStudyPilotAdmission.wouldAdmitWhenArmed",
        "an admission can only be marked would-admit-when-armed once the armed gate is ready",
      );
    }

    if (admission.admissionHash !== openAgentsExternalRepoStudyPilotAdmissionHash(admission)) {
      return yield* admissionError(
        "externalRepoStudyPilotAdmission.admissionHash",
        "must match the deterministic admission hash",
      );
    }
  });
}

function requireNonEmpty(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.trim().length === 0 ? admissionError(path, "must be non-empty") : Effect.void;
}

function requireSha256(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.startsWith("sha256:") ? Effect.void : admissionError(path, "must be a sha256 ref");
}

function slugRepo(repo: string): string {
  return repo
    .replace(/[^a-zA-Z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .toLowerCase()
    .slice(0, 80);
}

function admissionError(path: string, reason: string): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}

/** Re-export for callers that want to reason about the admission gate state. */
export type { OpenAgentsCustomerPrivateValidationGateState };

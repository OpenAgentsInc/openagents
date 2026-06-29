import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

/**
 * Refs-only economic gate for repo-studying marketplace claims.
 *
 * This is source-level policy machinery only. It can prove that package
 * metering, pricing, payout, and settlement-policy refs are present and
 * mutually bound before a future armed marketplace listing. It never lists,
 * bills, settles, pays, or marks a study packet claimable.
 */
export const OPENAGENTS_EXTERNAL_REPO_STUDY_MARKETPLACE_GATES_SCHEMA_REF =
  "openagents.external_repo_study_marketplace_gates.v0" as const;

export const ExternalRepoStudyMarketplaceGateFlagName =
  "EXTERNAL_REPO_STUDY_MARKETPLACE_GATES_ENABLED" as const;

export const OpenAgentsExternalRepoStudyMarketplaceGateState = S.Literals([
  "inert_disabled",
  "armed_blocked",
  "armed_ready",
]);
export type OpenAgentsExternalRepoStudyMarketplaceGateState =
  typeof OpenAgentsExternalRepoStudyMarketplaceGateState.Type;

export const OpenAgentsExternalRepoStudyMarketplaceDecisionState = S.Literals([
  "economic_gates_ready_held",
  "blocked",
]);
export type OpenAgentsExternalRepoStudyMarketplaceDecisionState =
  typeof OpenAgentsExternalRepoStudyMarketplaceDecisionState.Type;

export interface ExternalRepoStudyMarketplaceGateRequest {
  readonly contributorRef: string;
  readonly customerRef: string;
  readonly meteringPolicyRef?: string;
  readonly packagePolicyRef?: string;
  readonly packetRef: string;
  readonly payoutPolicyRef?: string;
  readonly pricingPolicyRef?: string;
  readonly repo: string;
  readonly settlementPolicyRef?: string;
  readonly termsAccepted?: boolean;
  readonly usageUnitRef?: string;
}

export const OpenAgentsExternalRepoStudyMarketplaceGate = S.Struct({
  blockedReasonRefs: S.Array(S.String),
  effectsApplied: S.Literal(false),
  flagName: S.Literal(ExternalRepoStudyMarketplaceGateFlagName),
  ownerSignoffPresent: S.Boolean,
  state: OpenAgentsExternalRepoStudyMarketplaceGateState,
});
export type OpenAgentsExternalRepoStudyMarketplaceGate =
  typeof OpenAgentsExternalRepoStudyMarketplaceGate.Type;

export const OpenAgentsExternalRepoStudyMarketplaceGates = S.Struct({
  blockerRefs: S.Array(S.String),
  contributorRef: S.String,
  customerPublicClaimAllowed: S.Literal(false),
  customerRef: S.String,
  effectsApplied: S.Literal(false),
  evidenceRefs: S.Array(S.String),
  generatedAt: S.String,
  gate: OpenAgentsExternalRepoStudyMarketplaceGate,
  marketplaceGatesHash: S.String,
  marketplaceGatesRef: S.String,
  marketplacePackageAllowed: S.Literal(false),
  meteringPolicyPresent: S.Boolean,
  packageListed: S.Literal(false),
  packagePolicyPresent: S.Boolean,
  packetRef: S.String,
  payoutEligible: S.Literal(false),
  payoutPolicyPresent: S.Boolean,
  pricingPolicyPresent: S.Boolean,
  repo: S.String,
  safeCopy: S.String,
  schemaRef: S.Literal(
    OPENAGENTS_EXTERNAL_REPO_STUDY_MARKETPLACE_GATES_SCHEMA_REF,
  ),
  settlementPolicyPresent: S.Boolean,
  sourceBoundary: S.Literal("customer_refs_withheld"),
  state: OpenAgentsExternalRepoStudyMarketplaceDecisionState,
  termsAccepted: S.Boolean,
  unsafeCopyRefs: S.Array(S.String),
  usageUnitPresent: S.Boolean,
  wouldAllowMarketplaceWhenArmed: S.Boolean,
});
export type OpenAgentsExternalRepoStudyMarketplaceGates =
  typeof OpenAgentsExternalRepoStudyMarketplaceGates.Type;

export interface BuildOpenAgentsExternalRepoStudyMarketplaceGatesInput {
  readonly generatedAt?: string;
  readonly marketplaceFlagArmed?: boolean;
  readonly ownerSignoffPresent?: boolean;
  readonly request: ExternalRepoStudyMarketplaceGateRequest;
}

export function buildOpenAgentsExternalRepoStudyMarketplaceGates(
  input: BuildOpenAgentsExternalRepoStudyMarketplaceGatesInput,
): Effect.Effect<
  OpenAgentsExternalRepoStudyMarketplaceGates,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const request = input.request;
    yield* requireNonEmpty(request.repo, "externalRepoStudyMarketplace.repo");
    yield* requireNonEmpty(
      request.customerRef,
      "externalRepoStudyMarketplace.customerRef",
    );
    yield* requireNonEmpty(
      request.contributorRef,
      "externalRepoStudyMarketplace.contributorRef",
    );
    yield* requireNonEmpty(
      request.packetRef,
      "externalRepoStudyMarketplace.packetRef",
    );

    const meteringPolicyPresent = hasRef(request.meteringPolicyRef);
    const packagePolicyPresent = hasRef(request.packagePolicyRef);
    const payoutPolicyPresent = hasRef(request.payoutPolicyRef);
    const pricingPolicyPresent = hasRef(request.pricingPolicyRef);
    const settlementPolicyPresent = hasRef(request.settlementPolicyRef);
    const usageUnitPresent = hasRef(request.usageUnitRef);
    const termsAccepted = request.termsAccepted ?? false;

    const economicGatePassed =
      meteringPolicyPresent &&
      packagePolicyPresent &&
      payoutPolicyPresent &&
      pricingPolicyPresent &&
      settlementPolicyPresent &&
      usageUnitPresent &&
      termsAccepted;

    const blockerRefs = buildMarketplaceBlockerRefs({
      meteringPolicyPresent,
      packagePolicyPresent,
      payoutPolicyPresent,
      pricingPolicyPresent,
      settlementPolicyPresent,
      termsAccepted,
      usageUnitPresent,
    });

    const gate = buildMarketplaceGate({
      economicGatePassed,
      marketplaceFlagArmed: input.marketplaceFlagArmed ?? false,
      ownerSignoffPresent: input.ownerSignoffPresent ?? false,
    });

    const generatedAt =
      input.generatedAt ??
      "generated_at.withheld_for_stable_external_repo_study_marketplace_gates_hash";

    const evidenceRefs = [
      request.customerRef,
      request.contributorRef,
      request.packetRef,
      ...(request.meteringPolicyRef ? [request.meteringPolicyRef] : []),
      ...(request.packagePolicyRef ? [request.packagePolicyRef] : []),
      ...(request.pricingPolicyRef ? [request.pricingPolicyRef] : []),
      ...(request.payoutPolicyRef ? [request.payoutPolicyRef] : []),
      ...(request.settlementPolicyRef ? [request.settlementPolicyRef] : []),
      ...(request.usageUnitRef ? [request.usageUnitRef] : []),
      "docs/launch/vertex-fleet/autopilot.external_repo_studying_pilot.v1.md",
    ];

    const base: OpenAgentsExternalRepoStudyMarketplaceGates = {
      blockerRefs,
      contributorRef: request.contributorRef,
      customerPublicClaimAllowed: false,
      customerRef: request.customerRef,
      effectsApplied: false,
      evidenceRefs,
      generatedAt,
      gate,
      marketplaceGatesHash: "sha256:pending",
      marketplaceGatesRef: "external_repo_study_marketplace_gates.pending",
      marketplacePackageAllowed: false,
      meteringPolicyPresent,
      packageListed: false,
      packagePolicyPresent,
      packetRef: request.packetRef,
      payoutEligible: false,
      payoutPolicyPresent,
      pricingPolicyPresent,
      repo: request.repo,
      safeCopy:
        "Repo-studying marketplace gates checked package policy, metering unit, pricing, payout, and settlement refs without listing a package, billing, paying, settling, or allowing a public customer claim.",
      schemaRef: OPENAGENTS_EXTERNAL_REPO_STUDY_MARKETPLACE_GATES_SCHEMA_REF,
      settlementPolicyPresent,
      sourceBoundary: "customer_refs_withheld",
      state: economicGatePassed ? "economic_gates_ready_held" : "blocked",
      termsAccepted,
      unsafeCopyRefs: [
        "blocked_claim.repo_study_marketplace_package_live",
        "blocked_claim.repo_study_usage_billing_live",
        "blocked_claim.repo_study_payout_eligible",
        "blocked_claim.repo_study_settlement_live",
      ],
      usageUnitPresent,
      wouldAllowMarketplaceWhenArmed:
        economicGatePassed && gate.state === "armed_ready",
    };

    const marketplaceGatesHash =
      openAgentsExternalRepoStudyMarketplaceGatesHash(base);

    return yield* decodeOpenAgentsExternalRepoStudyMarketplaceGates({
      ...base,
      marketplaceGatesHash,
      marketplaceGatesRef: `external_repo_study_marketplace_gates.${slugRepo(request.repo)}.${shortHash(marketplaceGatesHash)}`,
    });
  });
}

export function decodeOpenAgentsExternalRepoStudyMarketplaceGates(
  value: unknown,
): Effect.Effect<
  OpenAgentsExternalRepoStudyMarketplaceGates,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(
      value,
      "externalRepoStudyMarketplace",
    );
    const gates = yield* S.decodeUnknownEffect(
      OpenAgentsExternalRepoStudyMarketplaceGates,
    )(value).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBenchmarkContractError({
            path: "externalRepoStudyMarketplace",
            reason: String(error),
          }),
      ),
    );
    yield* validateExternalRepoStudyMarketplaceGates(gates);
    return gates;
  });
}

export function openAgentsExternalRepoStudyMarketplaceGatesHash(
  gates: OpenAgentsExternalRepoStudyMarketplaceGates,
): string {
  const {
    generatedAt: _generatedAt,
    marketplaceGatesHash: _marketplaceGatesHash,
    marketplaceGatesRef: _marketplaceGatesRef,
    ...stable
  } = gates;
  return sha256Ref(stableJson(stable));
}

function buildMarketplaceGate(input: {
  readonly economicGatePassed: boolean;
  readonly marketplaceFlagArmed: boolean;
  readonly ownerSignoffPresent: boolean;
}): OpenAgentsExternalRepoStudyMarketplaceGate {
  if (!input.marketplaceFlagArmed) {
    return {
      blockedReasonRefs: [],
      effectsApplied: false,
      flagName: ExternalRepoStudyMarketplaceGateFlagName,
      ownerSignoffPresent: input.ownerSignoffPresent,
      state: "inert_disabled",
    };
  }

  const blockedReasonRefs: string[] = [];
  if (!input.economicGatePassed) {
    blockedReasonRefs.push("marketplace.blocked.economic_gate_not_passed");
  }
  if (!input.ownerSignoffPresent) {
    blockedReasonRefs.push("marketplace.blocked.owner_signoff_missing");
  }

  return {
    blockedReasonRefs,
    effectsApplied: false,
    flagName: ExternalRepoStudyMarketplaceGateFlagName,
    ownerSignoffPresent: input.ownerSignoffPresent,
    state: blockedReasonRefs.length === 0 ? "armed_ready" : "armed_blocked",
  };
}

function buildMarketplaceBlockerRefs(input: {
  readonly meteringPolicyPresent: boolean;
  readonly packagePolicyPresent: boolean;
  readonly payoutPolicyPresent: boolean;
  readonly pricingPolicyPresent: boolean;
  readonly settlementPolicyPresent: boolean;
  readonly termsAccepted: boolean;
  readonly usageUnitPresent: boolean;
}): ReadonlyArray<string> {
  const blockers: string[] = [];
  if (!input.packagePolicyPresent) {
    blockers.push("blocker.external_repo_study_marketplace.package_policy_missing");
  }
  if (!input.meteringPolicyPresent || !input.usageUnitPresent) {
    blockers.push("blocker.external_repo_study_marketplace.metering_missing");
  }
  if (!input.pricingPolicyPresent) {
    blockers.push("blocker.external_repo_study_marketplace.pricing_missing");
  }
  if (!input.payoutPolicyPresent) {
    blockers.push("blocker.external_repo_study_marketplace.payout_policy_missing");
  }
  if (!input.settlementPolicyPresent) {
    blockers.push("blocker.external_repo_study_marketplace.settlement_policy_missing");
  }
  if (!input.termsAccepted) {
    blockers.push("blocker.external_repo_study_marketplace.terms_not_accepted");
  }
  return blockers;
}

function validateExternalRepoStudyMarketplaceGates(
  gates: OpenAgentsExternalRepoStudyMarketplaceGates,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(gates.repo, "externalRepoStudyMarketplace.repo");
    yield* requireNonEmpty(
      gates.customerRef,
      "externalRepoStudyMarketplace.customerRef",
    );
    yield* requireNonEmpty(
      gates.contributorRef,
      "externalRepoStudyMarketplace.contributorRef",
    );
    yield* requireNonEmpty(
      gates.packetRef,
      "externalRepoStudyMarketplace.packetRef",
    );
    yield* requireSha256(
      gates.marketplaceGatesHash,
      "externalRepoStudyMarketplace.marketplaceGatesHash",
    );

    if (
      gates.customerPublicClaimAllowed !== false ||
      gates.marketplacePackageAllowed !== false ||
      gates.payoutEligible !== false ||
      gates.packageListed !== false ||
      gates.effectsApplied !== false ||
      gates.gate.effectsApplied !== false
    ) {
      return yield* marketplaceError(
        "externalRepoStudyMarketplace.claimGates",
        "marketplace gate must not grant package, billing, payout, settlement, or customer claims",
      );
    }

    if (gates.state === "economic_gates_ready_held") {
      if (
        !gates.packagePolicyPresent ||
        !gates.meteringPolicyPresent ||
        !gates.usageUnitPresent ||
        !gates.pricingPolicyPresent ||
        !gates.payoutPolicyPresent ||
        !gates.settlementPolicyPresent ||
        !gates.termsAccepted
      ) {
        return yield* marketplaceError(
          "externalRepoStudyMarketplace.state",
          "economic_gates_ready_held requires package, metering, usage, pricing, payout, settlement, and terms refs",
        );
      }
    }

    if (
      gates.wouldAllowMarketplaceWhenArmed &&
      gates.gate.state !== "armed_ready"
    ) {
      return yield* marketplaceError(
        "externalRepoStudyMarketplace.wouldAllowMarketplaceWhenArmed",
        "marketplace can only be marked would-allow once the armed gate is ready",
      );
    }

    if (
      gates.marketplaceGatesHash !==
      openAgentsExternalRepoStudyMarketplaceGatesHash(gates)
    ) {
      return yield* marketplaceError(
        "externalRepoStudyMarketplace.marketplaceGatesHash",
        "must match the deterministic marketplace gates hash",
      );
    }
  });
}

function hasRef(value: string | undefined): boolean {
  return (value ?? "").trim().length > 0;
}

function requireNonEmpty(
  value: string,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.trim().length === 0
    ? marketplaceError(path, "must be non-empty")
    : Effect.void;
}

function requireSha256(
  value: string,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.startsWith("sha256:")
    ? Effect.void
    : marketplaceError(path, "must be a sha256 ref");
}

function slugRepo(repo: string): string {
  return repo
    .replace(/[^a-zA-Z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .toLowerCase()
    .slice(0, 80);
}

function marketplaceError(
  path: string,
  reason: string,
): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}

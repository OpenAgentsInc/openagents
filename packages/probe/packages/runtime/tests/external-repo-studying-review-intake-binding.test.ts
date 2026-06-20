import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsExternalRepoStudyCustomerAuthorizationRegistry,
  buildOpenAgentsExternalRepoStudyPrivacyPolicyRegistry,
  buildOpenAgentsExternalRepoStudyReviewIntakeBinding,
  openAgentsExternalRepoStudyReviewIntakeBindingHash,
  EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
  type ExternalRepoStudyCustomerAuthorizationInput,
  type ExternalRepoStudyPrivacyReviewRequest,
  type OpenAgentsExternalRepoStudyCustomerAuthorizationRegistry,
} from "../src";

const generatedAt = "2026-06-20T00:00:00.000Z" as const;

const SECRET_STRINGS = [
  "gold_answer",
  "rubric_claim",
  "evidence_excerpt",
  "raw customer data",
];

type ReviewRequestSansBoundRefs = Omit<
  ExternalRepoStudyPrivacyReviewRequest,
  "dataProcessingAgreementRef" | "retentionPolicyRef" | "customerAuthorizationRef"
>;

const baseReviewRequest: ReviewRequestSansBoundRefs = {
  customerRef: "customer.acme.v0",
  repo: "ExampleCorp/widget-service",
  retentionDays: 90,
  declaredPiiCategories: ["none"],
  reviewerRef: "reviewer.privacy.casey.v0",
};

const activeAuthorization: ExternalRepoStudyCustomerAuthorizationInput = {
  customerRef: "customer.acme.v0",
  effectiveDate: "2026-06-20",
  grantRef: "grant.acme.widget.v0",
  repo: "ExampleCorp/widget-service",
  scope: "external_repo_study",
  status: "active",
};

const policyRegistry = () =>
  buildOpenAgentsExternalRepoStudyPrivacyPolicyRegistry({ generatedAt });

const authRegistryWith = (
  authorizations: ReadonlyArray<ExternalRepoStudyCustomerAuthorizationInput>,
) =>
  buildOpenAgentsExternalRepoStudyCustomerAuthorizationRegistry({
    authorizations,
    generatedAt,
  });

const activeRefOf = (
  registry: OpenAgentsExternalRepoStudyCustomerAuthorizationRegistry,
): string => registry.recordedAuthorizations[0]!.authorizationRef;

describe("external repo studying review<->intake binding (all three gates)", () => {
  test("binds DPA, retention, AND authorization at once, held inert", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const pol = yield* policyRegistry();
        const auth = yield* authRegistryWith([activeAuthorization]);
        return yield* buildOpenAgentsExternalRepoStudyReviewIntakeBinding({
          authorizationRegistry: auth,
          authorizationCandidateRef: activeRefOf(auth),
          generatedAt,
          policyRegistry: pol,
          policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
          reviewRequest: baseReviewRequest,
        });
      }),
    );

    expect(binding.schemaRef).toBe(
      "openagents.external_repo_study_review_intake_binding.v0",
    );
    expect(binding.state).toBe("bound_held");
    expect(binding.bound).toBe(true);
    expect(binding.policyPublished).toBe(true);
    expect(binding.authorizationActive).toBe(true);
    expect(binding.dataProcessingAgreementRef).toBe(
      EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
    );
    expect(binding.retentionPolicyRef).toBe(
      EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
    );
    expect(binding.customerAuthorizationRef).toBe(
      binding.authorizationCandidateRef,
    );

    // The nested review preflight sees all three derived refs as present.
    expect(binding.reviewPreflight.dataProcessingAgreementPresent).toBe(true);
    expect(binding.reviewPreflight.retentionPolicyPresent).toBe(true);
    expect(binding.reviewPreflight.customerAuthorizationPresent).toBe(true);
    expect(binding.reviewPreflight.state).toBe("review_ready_held");

    // Inert by construction.
    expect(binding.reviewCleared).toBe(false);
    expect(binding.effectsApplied).toBe(false);
    expect(binding.customerPublicClaimAllowed).toBe(false);
    expect(binding.marketplacePackageAllowed).toBe(false);
    expect(binding.payoutEligible).toBe(false);
    expect(binding.reviewPreflight.reviewCleared).toBe(false);
    expect(binding.bindingHash).toBe(
      openAgentsExternalRepoStudyReviewIntakeBindingHash(binding),
    );
  });

  test("a forged policy ref leaves DPA/retention unbound (authorization still derived)", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const pol = yield* policyRegistry();
        const auth = yield* authRegistryWith([activeAuthorization]);
        return yield* buildOpenAgentsExternalRepoStudyReviewIntakeBinding({
          authorizationRegistry: auth,
          authorizationCandidateRef: activeRefOf(auth),
          generatedAt,
          policyRegistry: pol,
          policyRef: "external_repo_study_privacy_policy.forged.v0",
          reviewRequest: baseReviewRequest,
        });
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.bound).toBe(false);
    expect(binding.policyPublished).toBe(false);
    expect(binding.dataProcessingAgreementRef).toBeNull();
    expect(binding.retentionPolicyRef).toBeNull();
    expect(binding.policyTermsDigest).toBeNull();
    expect(binding.mismatchRefs).toContain(
      "blocker.external_repo_study_review_intake_binding.policy_ref_not_published",
    );

    // The authorization gate is independently backed and still derived.
    expect(binding.authorizationActive).toBe(true);
    expect(binding.customerAuthorizationRef).toBe(
      binding.authorizationCandidateRef,
    );

    // The review is blocked because DPA + retention are missing.
    expect(binding.reviewPreflight.dataProcessingAgreementPresent).toBe(false);
    expect(binding.reviewPreflight.retentionPolicyPresent).toBe(false);
    expect(binding.reviewPreflight.customerAuthorizationPresent).toBe(true);
    expect(binding.reviewPreflight.state).toBe("blocked");
    expect(binding.reviewPreflight.privacyReviewRef).toBeNull();
  });

  test("a forged authorization ref leaves the lawful-basis gate unbound (policy still derived)", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const pol = yield* policyRegistry();
        const auth = yield* authRegistryWith([activeAuthorization]);
        return yield* buildOpenAgentsExternalRepoStudyReviewIntakeBinding({
          authorizationRegistry: auth,
          authorizationCandidateRef: "customer_authorization.forged.v0",
          generatedAt,
          policyRegistry: pol,
          policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
          reviewRequest: baseReviewRequest,
        });
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.bound).toBe(false);
    expect(binding.authorizationActive).toBe(false);
    expect(binding.customerAuthorizationRef).toBeNull();
    expect(binding.mismatchRefs).toContain(
      "blocker.external_repo_study_review_intake_binding.authorization_not_active",
    );

    // The policy gate is independently backed and still derived.
    expect(binding.policyPublished).toBe(true);
    expect(binding.dataProcessingAgreementRef).toBe(
      EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
    );

    // The review is blocked because the lawful-basis authorization is missing.
    expect(binding.reviewPreflight.customerAuthorizationPresent).toBe(false);
    expect(binding.reviewPreflight.state).toBe("blocked");
    expect(binding.reviewPreflight.blockerRefs).toContain(
      "blocker.external_repo_study_privacy_review.customer_authorization_missing",
    );
  });

  test("a withdrawn authorization never binds the lawful-basis gate", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const pol = yield* policyRegistry();
        const auth = yield* authRegistryWith([
          { ...activeAuthorization, status: "withdrawn" },
        ]);
        return yield* buildOpenAgentsExternalRepoStudyReviewIntakeBinding({
          authorizationRegistry: auth,
          authorizationCandidateRef: activeRefOf(auth),
          generatedAt,
          policyRegistry: pol,
          policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
          reviewRequest: baseReviewRequest,
        });
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.authorizationActive).toBe(false);
    expect(binding.customerAuthorizationRef).toBeNull();
    expect(binding.reviewPreflight.customerAuthorizationPresent).toBe(false);
    expect(binding.reviewPreflight.state).toBe("blocked");
  });

  test("a customer mismatch does not bind even with real published policy + active ref", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const pol = yield* policyRegistry();
        const auth = yield* authRegistryWith([activeAuthorization]);
        return yield* buildOpenAgentsExternalRepoStudyReviewIntakeBinding({
          authorizationRegistry: auth,
          authorizationCandidateRef: activeRefOf(auth),
          generatedAt,
          policyRegistry: pol,
          policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
          reviewRequest: { ...baseReviewRequest, customerRef: "customer.other.v0" },
        });
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.authorizationActive).toBe(false);
    expect(binding.customerAuthorizationRef).toBeNull();
    expect(binding.reviewPreflight.customerAuthorizationPresent).toBe(false);
  });

  test("armed + reviewer sign-off is would-clear-when-armed but still inert", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const pol = yield* policyRegistry();
        const auth = yield* authRegistryWith([activeAuthorization]);
        return yield* buildOpenAgentsExternalRepoStudyReviewIntakeBinding({
          authorizationRegistry: auth,
          authorizationCandidateRef: activeRefOf(auth),
          generatedAt,
          policyRegistry: pol,
          policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
          reviewRequest: baseReviewRequest,
          reviewFlagArmed: true,
          reviewerSignoffPresent: true,
        });
      }),
    );

    expect(binding.wouldClearWhenArmed).toBe(true);
    expect(binding.reviewPreflight.clearanceGate.state).toBe("armed_ready");
    // Still inert even when armed.
    expect(binding.reviewCleared).toBe(false);
    expect(binding.effectsApplied).toBe(false);
    expect(binding.reviewPreflight.reviewCleared).toBe(false);
    expect(binding.reviewPreflight.effectsApplied).toBe(false);
  });

  test("rejects the OpenAgents repo as a binding target", async () => {
    const program = Effect.gen(function* () {
      const pol = yield* policyRegistry();
      const auth = yield* authRegistryWith([activeAuthorization]);
      return yield* buildOpenAgentsExternalRepoStudyReviewIntakeBinding({
        authorizationRegistry: auth,
        authorizationCandidateRef: activeRefOf(auth),
        generatedAt,
        policyRegistry: pol,
        policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
        reviewRequest: { ...baseReviewRequest, repo: "OpenAgentsInc/openagents" },
      });
    });

    await expect(Effect.runPromise(program)).rejects.toThrow();
  });

  test("never leaks private content into the public binding projection", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const pol = yield* policyRegistry();
        const auth = yield* authRegistryWith([activeAuthorization]);
        return yield* buildOpenAgentsExternalRepoStudyReviewIntakeBinding({
          authorizationRegistry: auth,
          authorizationCandidateRef: activeRefOf(auth),
          generatedAt,
          policyRegistry: pol,
          policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
          reviewRequest: baseReviewRequest,
        });
      }),
    );
    const serialized = JSON.stringify(binding);
    for (const secret of SECRET_STRINGS) {
      expect(serialized).not.toContain(secret);
    }
  });
});

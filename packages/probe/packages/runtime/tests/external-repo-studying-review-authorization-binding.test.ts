import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsExternalRepoStudyCustomerAuthorizationRegistry,
  buildOpenAgentsExternalRepoStudyReviewAuthorizationBinding,
  openAgentsExternalRepoStudyReviewAuthorizationBindingHash,
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

type ReviewRequestSansAuthRef = Omit<
  ExternalRepoStudyPrivacyReviewRequest,
  "customerAuthorizationRef"
>;

const baseReviewRequest: ReviewRequestSansAuthRef = {
  customerRef: "customer.acme.v0",
  repo: "ExampleCorp/widget-service",
  dataProcessingAgreementRef: "dpa.acme.widget.v0",
  retentionPolicyRef: "retention.acme.widget.v0",
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

const registryWith = (
  authorizations: ReadonlyArray<ExternalRepoStudyCustomerAuthorizationInput>,
) =>
  buildOpenAgentsExternalRepoStudyCustomerAuthorizationRegistry({
    authorizations,
    generatedAt,
  });

const activeRefOf = (
  registry: OpenAgentsExternalRepoStudyCustomerAuthorizationRegistry,
): string => registry.recordedAuthorizations[0]!.authorizationRef;

describe("external repo studying review<->authorization binding", () => {
  test("binds the authorization ref from a known active authorization, held inert", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registryWith([activeAuthorization]);
        return yield* buildOpenAgentsExternalRepoStudyReviewAuthorizationBinding({
          authorizationRegistry: reg,
          authorizationCandidateRef: activeRefOf(reg),
          generatedAt,
          reviewRequest: baseReviewRequest,
        });
      }),
    );

    expect(binding.schemaRef).toBe(
      "openagents.external_repo_study_review_authorization_binding.v0",
    );
    expect(binding.state).toBe("bound_held");
    expect(binding.bound).toBe(true);
    expect(binding.authorizationActive).toBe(true);
    expect(binding.authorizationRef).toBe(binding.authorizationCandidateRef);

    // The nested review preflight sees the derived ref as present and would
    // clear when armed.
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
      openAgentsExternalRepoStudyReviewAuthorizationBindingHash(binding),
    );
  });

  test("a forged / unknown authorization ref binds nothing and blocks the review", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registryWith([activeAuthorization]);
        return yield* buildOpenAgentsExternalRepoStudyReviewAuthorizationBinding({
          authorizationRegistry: reg,
          authorizationCandidateRef: "customer_authorization.forged.v0",
          generatedAt,
          reviewRequest: baseReviewRequest,
        });
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.bound).toBe(false);
    expect(binding.authorizationActive).toBe(false);
    expect(binding.authorizationRef).toBeNull();
    expect(binding.mismatchRefs).toContain(
      "blocker.external_repo_study_review_authorization_binding.authorization_not_active",
    );

    // The review cannot present an authorization and is blocked.
    expect(binding.reviewPreflight.customerAuthorizationPresent).toBe(false);
    expect(binding.reviewPreflight.state).toBe("blocked");
    expect(binding.reviewPreflight.privacyReviewRef).toBeNull();
    expect(binding.reviewPreflight.blockerRefs).toContain(
      "blocker.external_repo_study_privacy_review.customer_authorization_missing",
    );
  });

  test("a withdrawn authorization never binds (Section 6 revocation)", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registryWith([
          { ...activeAuthorization, status: "withdrawn" },
        ]);
        return yield* buildOpenAgentsExternalRepoStudyReviewAuthorizationBinding({
          authorizationRegistry: reg,
          authorizationCandidateRef: activeRefOf(reg),
          generatedAt,
          reviewRequest: baseReviewRequest,
        });
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.authorizationRef).toBeNull();
    expect(binding.reviewPreflight.customerAuthorizationPresent).toBe(false);
    expect(binding.reviewPreflight.state).toBe("blocked");
  });

  test("a customer mismatch does not bind even with a real active ref", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registryWith([activeAuthorization]);
        return yield* buildOpenAgentsExternalRepoStudyReviewAuthorizationBinding({
          authorizationRegistry: reg,
          authorizationCandidateRef: activeRefOf(reg),
          generatedAt,
          reviewRequest: {
            ...baseReviewRequest,
            customerRef: "customer.other.v0",
          },
        });
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.authorizationRef).toBeNull();
    expect(binding.reviewPreflight.customerAuthorizationPresent).toBe(false);
  });

  test("a repo mismatch does not bind even with a real active ref", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registryWith([activeAuthorization]);
        return yield* buildOpenAgentsExternalRepoStudyReviewAuthorizationBinding({
          authorizationRegistry: reg,
          authorizationCandidateRef: activeRefOf(reg),
          generatedAt,
          reviewRequest: {
            ...baseReviewRequest,
            repo: "ExampleCorp/other-service",
          },
        });
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.authorizationRef).toBeNull();
    expect(binding.reviewPreflight.customerAuthorizationPresent).toBe(false);
  });

  test("armed + reviewer sign-off is would-clear-when-armed but still inert", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registryWith([activeAuthorization]);
        return yield* buildOpenAgentsExternalRepoStudyReviewAuthorizationBinding({
          authorizationRegistry: reg,
          authorizationCandidateRef: activeRefOf(reg),
          generatedAt,
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
      const reg = yield* registryWith([activeAuthorization]);
      return yield* buildOpenAgentsExternalRepoStudyReviewAuthorizationBinding({
        authorizationRegistry: reg,
        authorizationCandidateRef: activeRefOf(reg),
        generatedAt,
        reviewRequest: {
          ...baseReviewRequest,
          repo: "OpenAgentsInc/openagents",
        },
      });
    });

    await expect(Effect.runPromise(program)).rejects.toThrow();
  });

  test("never leaks private content into the public binding projection", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registryWith([activeAuthorization]);
        return yield* buildOpenAgentsExternalRepoStudyReviewAuthorizationBinding({
          authorizationRegistry: reg,
          authorizationCandidateRef: activeRefOf(reg),
          generatedAt,
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

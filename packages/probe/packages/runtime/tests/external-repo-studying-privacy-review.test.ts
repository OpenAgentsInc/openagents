import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsExternalRepoStudyPrivacyReviewPreflight,
  openAgentsExternalRepoStudyPrivacyReviewPreflightHash,
  PRIVACY_REVIEW_MAX_RETENTION_DAYS,
  type ExternalRepoStudyPrivacyReviewRequest,
} from "../src";

const generatedAt = "2026-06-20T00:00:00.000Z" as const;

// A REFS-ONLY privacy-review request. No customer data, PII values, repository
// content, or reviewer notes ever cross this boundary.
const request: ExternalRepoStudyPrivacyReviewRequest = {
  customerRef: "customer.examplecorp.v0",
  repo: "ExampleCorp/widget-service",
  dataProcessingAgreementRef: "dpa.examplecorp.v0",
  retentionPolicyRef: "retention_policy.examplecorp.widget-service.v0",
  retentionDays: 90,
  customerAuthorizationRef: "authorization.examplecorp.widget-service.v0",
  declaredPiiCategories: ["commit_author_email", "contributor_handle"],
  reviewerRef: "reviewer.privacy.448ba824.v0",
};

const SECRET_STRINGS = [
  "gold_answer",
  "rubric_claim",
  "evidence_excerpt",
  "raw customer data",
];

describe("external repo studying privacy review preflight", () => {
  test("passes a complete review request and holds it inert (flag default-OFF)", async () => {
    const preflight = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyPrivacyReviewPreflight({
        generatedAt,
        request,
      }),
    );

    expect(preflight.schemaRef).toBe(
      "openagents.external_repo_study_privacy_review_preflight.v0",
    );
    expect(preflight.state).toBe("review_ready_held");
    expect(preflight.dataProcessingAgreementPresent).toBe(true);
    expect(preflight.retentionPolicyPresent).toBe(true);
    expect(preflight.customerAuthorizationPresent).toBe(true);
    expect(preflight.retentionWithinCap).toBe(true);
    expect(preflight.declaredPiiCategoriesWithinPolicy).toBe(true);

    // Derives the privacyReviewRef the upload preflight consumes.
    expect(preflight.privacyReviewRef).toBe(
      "privacy_review.examplecorp_widget_service.v0",
    );

    // Held inert by default-OFF flag.
    expect(preflight.clearanceGate.state).toBe("inert_disabled");
    expect(preflight.clearanceGate.flagName).toBe(
      "EXTERNAL_REPO_STUDY_PRIVACY_REVIEW_ENABLED",
    );

    // Inert/no-claim guarantees.
    expect(preflight.reviewCleared).toBe(false);
    expect(preflight.effectsApplied).toBe(false);
    expect(preflight.clearanceGate.effectsApplied).toBe(false);
    expect(preflight.wouldClearWhenArmed).toBe(false);
    expect(preflight.customerPublicClaimAllowed).toBe(false);
    expect(preflight.marketplacePackageAllowed).toBe(false);
    expect(preflight.payoutEligible).toBe(false);
    expect(preflight.sourceBoundary).toBe("customer_refs_withheld");

    // Refs-only evidence and stable hash.
    expect(preflight.repo).toBe(request.repo);
    expect(preflight.repo).not.toBe("OpenAgentsInc/openagents");
    expect(preflight.evidenceRefs).toEqual(
      expect.arrayContaining([
        request.customerRef,
        request.dataProcessingAgreementRef,
        request.retentionPolicyRef,
        request.customerAuthorizationRef,
      ]),
    );
    expect(preflight.preflightHash).toBe(
      openAgentsExternalRepoStudyPrivacyReviewPreflightHash(preflight),
    );
    expect(
      `${preflight.safeCopy} ${preflight.unsafeCopyRefs.join(" ")}`,
    ).not.toMatch(/privacy review is live|payout eligible/i);
  });

  test("would-clear-when-armed only once flag + reviewer signoff are present, still inert", async () => {
    const preflight = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyPrivacyReviewPreflight({
        generatedAt,
        request,
        reviewerSignoffPresent: true,
        reviewFlagArmed: true,
      }),
    );

    expect(preflight.clearanceGate.state).toBe("armed_ready");
    expect(preflight.wouldClearWhenArmed).toBe(true);
    // Even armed + ready, no real effect is applied by this module.
    expect(preflight.reviewCleared).toBe(false);
    expect(preflight.effectsApplied).toBe(false);
    expect(preflight.clearanceGate.effectsApplied).toBe(false);
  });

  test("armed without reviewer signoff blocks the clearance gate", async () => {
    const preflight = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyPrivacyReviewPreflight({
        generatedAt,
        request,
        reviewFlagArmed: true,
      }),
    );

    expect(preflight.clearanceGate.state).toBe("armed_blocked");
    expect(preflight.clearanceGate.blockedReasonRefs).toContain(
      "privacy_review.blocked.reviewer_signoff_missing",
    );
    expect(preflight.wouldClearWhenArmed).toBe(false);
    expect(preflight.reviewCleared).toBe(false);
  });

  test("blocks and derives no privacyReviewRef when the DPA or customer authorization is missing", async () => {
    const preflight = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyPrivacyReviewPreflight({
        generatedAt,
        request: {
          ...request,
          dataProcessingAgreementRef: undefined,
          customerAuthorizationRef: undefined,
        },
      }),
    );

    expect(preflight.dataProcessingAgreementPresent).toBe(false);
    expect(preflight.customerAuthorizationPresent).toBe(false);
    expect(preflight.state).toBe("blocked");
    expect(preflight.privacyReviewRef).toBeNull();
    expect(preflight.blockerRefs).toContain(
      "blocker.external_repo_study_privacy_review.data_processing_agreement_missing",
    );
    expect(preflight.blockerRefs).toContain(
      "blocker.external_repo_study_privacy_review.customer_authorization_missing",
    );
  });

  test("blocks when retention is out of bounds or a PII category is out of policy", async () => {
    const preflight = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyPrivacyReviewPreflight({
        generatedAt,
        request: {
          ...request,
          retentionDays: PRIVACY_REVIEW_MAX_RETENTION_DAYS + 1,
          declaredPiiCategories: ["full_name", "home_address"],
        },
      }),
    );

    expect(preflight.retentionWithinCap).toBe(false);
    expect(preflight.declaredPiiCategoriesWithinPolicy).toBe(false);
    expect(preflight.state).toBe("blocked");
    expect(preflight.blockerRefs).toContain(
      "blocker.external_repo_study_privacy_review.retention_window_out_of_bounds",
    );
    expect(preflight.blockerRefs).toContain(
      "blocker.external_repo_study_privacy_review.declared_pii_categories_out_of_policy",
    );
  });

  test("rejects an OpenAgents repo as a privacy review target", async () => {
    const exit = await Effect.runPromiseExit(
      buildOpenAgentsExternalRepoStudyPrivacyReviewPreflight({
        generatedAt,
        request: { ...request, repo: "OpenAgentsInc/openagents" },
      }),
    );
    expect(exit._tag).toBe("Failure");
  });

  test("never leaks private content into the public preflight projection", async () => {
    const preflight = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyPrivacyReviewPreflight({
        generatedAt,
        request,
      }),
    );
    const serialized = JSON.stringify(preflight);
    for (const secret of SECRET_STRINGS) {
      expect(serialized).not.toContain(secret);
    }
  });
});

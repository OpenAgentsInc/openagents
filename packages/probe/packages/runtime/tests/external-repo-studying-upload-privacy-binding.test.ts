import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsExternalRepoStudyPrivacyReviewPreflight,
  buildOpenAgentsExternalRepoStudyUploadPrivacyBinding,
  openAgentsExternalRepoStudyUploadPrivacyBindingHash,
  type ExternalRepoStudyPrivacyReviewRequest,
  type ExternalRepoStudySelfServeUploadRequest,
} from "../src";

const generatedAt = "2026-06-20T00:00:00.000Z" as const;

const privacyRequest: ExternalRepoStudyPrivacyReviewRequest = {
  customerRef: "customer.examplecorp.v0",
  repo: "ExampleCorp/widget-service",
  dataProcessingAgreementRef: "dpa.examplecorp.v0",
  retentionPolicyRef: "retention_policy.examplecorp.widget-service.v0",
  retentionDays: 90,
  customerAuthorizationRef: "authorization.examplecorp.widget-service.v0",
  declaredPiiCategories: ["commit_author_email", "contributor_handle"],
  reviewerRef: "reviewer.privacy.448ba824.v0",
};

// A REFS-ONLY upload request MINUS its privacyReviewRef. The binding derives
// the ref from the cleared review; callers cannot inject their own.
const uploadRequest: Omit<
  ExternalRepoStudySelfServeUploadRequest,
  "privacyReviewRef"
> = {
  customerRef: "customer.examplecorp.v0",
  uploaderRef: "contributor.pylon.448ba824.v0",
  repo: "ExampleCorp/widget-service",
  uploadManifestDigest: "sha256:" + "a".repeat(64),
  declaredByteSize: 4 * 1024 * 1024,
  fileCount: 128,
  scanAttestationRef: "scan.attestation.examplecorp.widget-service.clean.v0",
  uploaderTermsAccepted: true,
};

const SECRET_STRINGS = [
  "gold_answer",
  "rubric_claim",
  "evidence_excerpt",
  "archive bytes",
];

const buildReview = (
  overrides: Partial<ExternalRepoStudyPrivacyReviewRequest> = {},
) =>
  buildOpenAgentsExternalRepoStudyPrivacyReviewPreflight({
    generatedAt,
    request: { ...privacyRequest, ...overrides },
  });

describe("external repo studying upload privacy binding", () => {
  test("binds the upload to a cleared, matching review and stays inert", async () => {
    const review = await Effect.runPromise(buildReview());
    const binding = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyUploadPrivacyBinding({
        generatedAt,
        privacyReviewPreflight: review,
        uploadRequest,
      }),
    );

    expect(binding.schemaRef).toBe(
      "openagents.external_repo_study_upload_privacy_binding.v0",
    );
    expect(binding.state).toBe("bound_held");
    expect(binding.bound).toBe(true);
    expect(binding.customerMatches).toBe(true);
    expect(binding.repoMatches).toBe(true);
    expect(binding.privacyReviewCleared).toBe(true);
    expect(binding.mismatchRefs).toEqual([]);

    // The bound ref is the one the review derived; the upload consumed it.
    expect(binding.privacyReviewRef).toBe(review.privacyReviewRef);
    expect(binding.uploadPreflight.privacyReviewPresent).toBe(true);
    expect(binding.uploadPreflight.state).toBe("intake_ready_held");

    // Inert/no-claim guarantees (binding + nested upload).
    expect(binding.intakeAdmitted).toBe(false);
    expect(binding.ingested).toBe(false);
    expect(binding.effectsApplied).toBe(false);
    expect(binding.customerPublicClaimAllowed).toBe(false);
    expect(binding.marketplacePackageAllowed).toBe(false);
    expect(binding.payoutEligible).toBe(false);
    expect(binding.wouldIngestWhenArmed).toBe(false);
    expect(binding.sourceBoundary).toBe("customer_refs_withheld");
    expect(binding.uploadPreflight.effectsApplied).toBe(false);

    expect(binding.bindingHash).toBe(
      openAgentsExternalRepoStudyUploadPrivacyBindingHash(binding),
    );
  });

  test("a blocked review binds no ref; the upload blocks on a missing privacy review", async () => {
    const review = await Effect.runPromise(
      buildReview({ dataProcessingAgreementRef: undefined }),
    );
    expect(review.state).toBe("blocked");
    expect(review.privacyReviewRef).toBeNull();

    const binding = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyUploadPrivacyBinding({
        generatedAt,
        privacyReviewPreflight: review,
        uploadRequest,
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.bound).toBe(false);
    expect(binding.privacyReviewCleared).toBe(false);
    expect(binding.privacyReviewRef).toBeNull();
    expect(binding.mismatchRefs).toContain(
      "blocker.external_repo_study_upload_privacy_binding.privacy_review_not_cleared",
    );

    // The upload's privacy gate is NOT satisfied by any forged string.
    expect(binding.uploadPreflight.privacyReviewPresent).toBe(false);
    expect(binding.uploadPreflight.state).toBe("blocked");
    expect(binding.uploadPreflight.blockerRefs).toContain(
      "blocker.external_repo_study_self_serve_upload.privacy_review_missing",
    );
  });

  test("a customer-ref mismatch leaves the binding unbound", async () => {
    const review = await Effect.runPromise(buildReview());
    const binding = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyUploadPrivacyBinding({
        generatedAt,
        privacyReviewPreflight: review,
        uploadRequest: {
          ...uploadRequest,
          customerRef: "customer.someoneelse.v0",
        },
      }),
    );

    expect(binding.customerMatches).toBe(false);
    expect(binding.state).toBe("unbound");
    expect(binding.privacyReviewRef).toBeNull();
    expect(binding.mismatchRefs).toContain(
      "blocker.external_repo_study_upload_privacy_binding.customer_ref_mismatch",
    );
    expect(binding.uploadPreflight.privacyReviewPresent).toBe(false);
  });

  test("a repo mismatch leaves the binding unbound", async () => {
    const review = await Effect.runPromise(buildReview());
    const binding = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyUploadPrivacyBinding({
        generatedAt,
        privacyReviewPreflight: review,
        uploadRequest: { ...uploadRequest, repo: "ExampleCorp/other-service" },
      }),
    );

    expect(binding.repoMatches).toBe(false);
    expect(binding.state).toBe("unbound");
    expect(binding.mismatchRefs).toContain(
      "blocker.external_repo_study_upload_privacy_binding.repo_mismatch",
    );
  });

  test("even armed with owner + reviewer signoff, no real effect is applied", async () => {
    const review = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyPrivacyReviewPreflight({
        generatedAt,
        request: privacyRequest,
        reviewerSignoffPresent: true,
        reviewFlagArmed: true,
      }),
    );
    const binding = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyUploadPrivacyBinding({
        generatedAt,
        ownerSignoffPresent: true,
        privacyReviewPreflight: review,
        uploadFlagArmed: true,
        uploadRequest,
      }),
    );

    expect(binding.state).toBe("bound_held");
    expect(binding.uploadPreflight.intakeGate.state).toBe("armed_ready");
    expect(binding.wouldIngestWhenArmed).toBe(true);
    // Armed-ready is still inert by construction.
    expect(binding.effectsApplied).toBe(false);
    expect(binding.intakeAdmitted).toBe(false);
    expect(binding.ingested).toBe(false);
    expect(binding.uploadPreflight.effectsApplied).toBe(false);
  });

  test("rejects an OpenAgents repo as a binding target", async () => {
    const review = await Effect.runPromise(buildReview());
    const exit = await Effect.runPromiseExit(
      buildOpenAgentsExternalRepoStudyUploadPrivacyBinding({
        generatedAt,
        privacyReviewPreflight: review,
        uploadRequest: { ...uploadRequest, repo: "OpenAgentsInc/openagents" },
      }),
    );
    expect(exit._tag).toBe("Failure");
  });

  test("never leaks private content into the public binding projection", async () => {
    const review = await Effect.runPromise(buildReview());
    const binding = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyUploadPrivacyBinding({
        generatedAt,
        privacyReviewPreflight: review,
        uploadRequest,
      }),
    );
    const serialized = JSON.stringify(binding);
    for (const secret of SECRET_STRINGS) {
      expect(serialized).not.toContain(secret);
    }
  });
});

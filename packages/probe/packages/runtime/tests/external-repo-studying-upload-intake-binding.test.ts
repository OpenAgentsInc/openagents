import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsExternalRepoStudyPrivacyReviewPreflight,
  buildOpenAgentsExternalRepoStudyScanAttestationRegistry,
  buildOpenAgentsExternalRepoStudyUploadIntakeBinding,
  openAgentsExternalRepoStudyUploadIntakeBindingHash,
  type ExternalRepoStudyPrivacyReviewRequest,
  type ExternalRepoStudyScanAttestationInput,
  type ExternalRepoStudySelfServeUploadRequest,
  type OpenAgentsExternalRepoStudyPrivacyReviewPreflight,
} from "../src";

const generatedAt = "2026-06-20T00:00:00.000Z" as const;

const MANIFEST_DIGEST =
  "sha256:1111111111111111111111111111111111111111111111111111111111111111" as const;

const CUSTOMER_REF = "customer.examplecorp.v0" as const;
const REPO = "ExampleCorp/widget-service" as const;

const reviewRequest: ExternalRepoStudyPrivacyReviewRequest = {
  customerRef: CUSTOMER_REF,
  repo: REPO,
  dataProcessingAgreementRef: "dpa.examplecorp.v0",
  retentionPolicyRef: "retention.examplecorp.v0",
  retentionDays: 90,
  customerAuthorizationRef: "authorization.examplecorp.v0",
  declaredPiiCategories: ["none"],
  reviewerRef: "reviewer.privacy.v0",
};

const cleanAttestation: ExternalRepoStudyScanAttestationInput = {
  customerRef: CUSTOMER_REF,
  findingsCount: 0,
  repo: REPO,
  scannerRef: "scanner.secret_malware.v0",
  uploadManifestDigest: MANIFEST_DIGEST,
  verdict: "clean",
};

// A REFS-ONLY upload request MINUS BOTH its privacyReviewRef and its
// scanAttestationRef. The binding derives both refs; callers cannot inject
// either.
const uploadRequest: Omit<
  ExternalRepoStudySelfServeUploadRequest,
  "privacyReviewRef" | "scanAttestationRef"
> = {
  customerRef: CUSTOMER_REF,
  uploaderRef: "contributor.pylon.448ba824.v0",
  repo: REPO,
  uploadManifestDigest: MANIFEST_DIGEST,
  declaredByteSize: 4 * 1024 * 1024,
  fileCount: 128,
  uploaderTermsAccepted: true,
};

const SECRET_STRINGS = [
  "gold_answer",
  "rubric_claim",
  "evidence_excerpt",
  "archive bytes",
  "raw customer data",
];

const buildReview = (
  overrides: Partial<ExternalRepoStudyPrivacyReviewRequest> = {},
) =>
  buildOpenAgentsExternalRepoStudyPrivacyReviewPreflight({
    generatedAt,
    request: { ...reviewRequest, ...overrides },
  });

const buildRegistry = (
  attestations: ReadonlyArray<ExternalRepoStudyScanAttestationInput> = [
    cleanAttestation,
  ],
) =>
  buildOpenAgentsExternalRepoStudyScanAttestationRegistry({
    attestations,
    generatedAt,
  });

describe("external repo studying upload intake binding", () => {
  test("binds BOTH gates and stays inert when review cleared + scan verified", async () => {
    const review = await Effect.runPromise(buildReview());
    const registry = await Effect.runPromise(buildRegistry());
    const scanRef = registry.recordedAttestations[0]!.attestationRef;

    const binding = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyUploadIntakeBinding({
        generatedAt,
        privacyReviewPreflight: review,
        scanAttestationCandidateRef: scanRef,
        scanAttestationRegistry: registry,
        uploadRequest,
      }),
    );

    expect(binding.schemaRef).toBe(
      "openagents.external_repo_study_upload_intake_binding.v0",
    );
    expect(binding.state).toBe("bound_held");
    expect(binding.bound).toBe(true);
    expect(binding.privacyReviewCleared).toBe(true);
    expect(binding.scanAttestationVerified).toBe(true);
    expect(binding.mismatchRefs).toEqual([]);

    // Both refs are derived and consumed by the upload.
    expect(binding.privacyReviewRef).toBe(review.privacyReviewRef);
    expect(binding.scanAttestationRef).toBe(scanRef);
    expect(binding.uploadPreflight.privacyReviewPresent).toBe(true);
    expect(binding.uploadPreflight.scanAttestationPresent).toBe(true);
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
      openAgentsExternalRepoStudyUploadIntakeBindingHash(binding),
    );
  });

  test("a blocked review leaves the privacy gate unbound; upload blocks on privacy", async () => {
    const review = await Effect.runPromise(
      buildReview({ customerAuthorizationRef: undefined }),
    );
    expect(review.state).toBe("blocked");
    const registry = await Effect.runPromise(buildRegistry());
    const scanRef = registry.recordedAttestations[0]!.attestationRef;

    const binding = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyUploadIntakeBinding({
        generatedAt,
        privacyReviewPreflight: review,
        scanAttestationCandidateRef: scanRef,
        scanAttestationRegistry: registry,
        uploadRequest,
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.bound).toBe(false);
    expect(binding.privacyReviewCleared).toBe(false);
    // The scan gate is still genuinely verified...
    expect(binding.scanAttestationVerified).toBe(true);
    expect(binding.scanAttestationRef).toBe(scanRef);
    // ...but the privacy gate is not, so no privacy ref is bound.
    expect(binding.privacyReviewRef).toBeNull();
    expect(binding.mismatchRefs).toContain(
      "blocker.external_repo_study_upload_intake_binding.privacy_review_not_cleared",
    );
    expect(binding.uploadPreflight.privacyReviewPresent).toBe(false);
    expect(binding.uploadPreflight.state).toBe("blocked");
    expect(binding.uploadPreflight.blockerRefs).toContain(
      "blocker.external_repo_study_self_serve_upload.privacy_review_missing",
    );
  });

  test("a forged scan ref leaves the clean-scan gate unbound; upload blocks on scan", async () => {
    const review = await Effect.runPromise(buildReview());
    const registry = await Effect.runPromise(buildRegistry());

    const binding = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyUploadIntakeBinding({
        generatedAt,
        privacyReviewPreflight: review,
        scanAttestationCandidateRef: "scan_attestation.forged.deadbeef.v0",
        scanAttestationRegistry: registry,
        uploadRequest,
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.scanAttestationVerified).toBe(false);
    expect(binding.scanAttestationRef).toBeNull();
    // The privacy gate is still genuinely cleared.
    expect(binding.privacyReviewCleared).toBe(true);
    expect(binding.privacyReviewRef).toBe(review.privacyReviewRef);
    expect(binding.mismatchRefs).toContain(
      "blocker.external_repo_study_upload_intake_binding.clean_scan_attestation_not_verified",
    );
    expect(binding.uploadPreflight.scanAttestationPresent).toBe(false);
    expect(binding.uploadPreflight.state).toBe("blocked");
    expect(binding.uploadPreflight.blockerRefs).toContain(
      "blocker.external_repo_study_self_serve_upload.clean_scan_attestation_missing",
    );
  });

  test("a review for a DIFFERENT customer does not bind the privacy gate", async () => {
    const review: OpenAgentsExternalRepoStudyPrivacyReviewPreflight =
      await Effect.runPromise(
        buildReview({ customerRef: "customer.other.v0" }),
      );
    const registry = await Effect.runPromise(buildRegistry());
    const scanRef = registry.recordedAttestations[0]!.attestationRef;

    const binding = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyUploadIntakeBinding({
        generatedAt,
        privacyReviewPreflight: review,
        scanAttestationCandidateRef: scanRef,
        scanAttestationRegistry: registry,
        uploadRequest,
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.customerMatches).toBe(false);
    expect(binding.privacyReviewRef).toBeNull();
    expect(binding.uploadPreflight.privacyReviewPresent).toBe(false);
    expect(binding.mismatchRefs).toContain(
      "blocker.external_repo_study_upload_intake_binding.customer_ref_mismatch",
    );
  });

  test("a clean scan for a DIFFERENT manifest does not bind the scan gate", async () => {
    const review = await Effect.runPromise(buildReview());
    const registry = await Effect.runPromise(buildRegistry());
    const scanRef = registry.recordedAttestations[0]!.attestationRef;

    const binding = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyUploadIntakeBinding({
        generatedAt,
        privacyReviewPreflight: review,
        scanAttestationCandidateRef: scanRef,
        scanAttestationRegistry: registry,
        uploadRequest: {
          ...uploadRequest,
          uploadManifestDigest:
            "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        },
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.scanAttestationVerified).toBe(false);
    expect(binding.scanAttestationRef).toBeNull();
    expect(binding.uploadPreflight.scanAttestationPresent).toBe(false);
  });

  test("even armed with owner signoff, no real effect is applied", async () => {
    const review = await Effect.runPromise(buildReview());
    const registry = await Effect.runPromise(buildRegistry());
    const scanRef = registry.recordedAttestations[0]!.attestationRef;

    const binding = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyUploadIntakeBinding({
        generatedAt,
        ownerSignoffPresent: true,
        privacyReviewPreflight: review,
        scanAttestationCandidateRef: scanRef,
        scanAttestationRegistry: registry,
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
    const registry = await Effect.runPromise(buildRegistry());
    const scanRef = registry.recordedAttestations[0]!.attestationRef;
    const exit = await Effect.runPromiseExit(
      buildOpenAgentsExternalRepoStudyUploadIntakeBinding({
        generatedAt,
        privacyReviewPreflight: review,
        scanAttestationCandidateRef: scanRef,
        scanAttestationRegistry: registry,
        uploadRequest: { ...uploadRequest, repo: "OpenAgentsInc/openagents" },
      }),
    );
    expect(exit._tag).toBe("Failure");
  });

  test("never leaks private content into the public binding projection", async () => {
    const review = await Effect.runPromise(buildReview());
    const registry = await Effect.runPromise(buildRegistry());
    const scanRef = registry.recordedAttestations[0]!.attestationRef;
    const binding = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyUploadIntakeBinding({
        generatedAt,
        privacyReviewPreflight: review,
        scanAttestationCandidateRef: scanRef,
        scanAttestationRegistry: registry,
        uploadRequest,
      }),
    );
    const serialized = JSON.stringify(binding);
    for (const secret of SECRET_STRINGS) {
      expect(serialized).not.toContain(secret);
    }
  });
});

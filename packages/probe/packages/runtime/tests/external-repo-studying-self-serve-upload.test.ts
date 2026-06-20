import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsExternalRepoStudySelfServeUploadPreflight,
  openAgentsExternalRepoStudySelfServeUploadPreflightHash,
  SELF_SERVE_UPLOAD_MAX_DECLARED_BYTES,
  SELF_SERVE_UPLOAD_MAX_FILE_COUNT,
  type ExternalRepoStudySelfServeUploadRequest,
} from "../src";

const generatedAt = "2026-06-19T00:00:00.000Z" as const;

// A REFS-ONLY upload request. No archive bytes, file paths, repository tree, or
// uploader PII ever cross this boundary.
const request: ExternalRepoStudySelfServeUploadRequest = {
  customerRef: "customer.examplecorp.v0",
  uploaderRef: "contributor.pylon.448ba824.v0",
  repo: "ExampleCorp/widget-service",
  uploadManifestDigest: "sha256:" + "a".repeat(64),
  declaredByteSize: 4 * 1024 * 1024,
  fileCount: 128,
  scanAttestationRef: "scan.attestation.examplecorp.widget-service.clean.v0",
  privacyReviewRef: "privacy_review.examplecorp.widget-service.v0",
  uploaderTermsAccepted: true,
};

const SECRET_STRINGS = [
  "gold_answer",
  "rubric_claim",
  "evidence_excerpt",
  "archive bytes",
];

describe("external repo studying self-serve upload preflight", () => {
  test("passes a complete upload request and holds it inert (flag default-OFF)", async () => {
    const preflight = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudySelfServeUploadPreflight({
        generatedAt,
        request,
      }),
    );

    expect(preflight.schemaRef).toBe(
      "openagents.external_repo_study_self_serve_upload_preflight.v0",
    );
    expect(preflight.state).toBe("intake_ready_held");
    expect(preflight.declaredByteSizeWithinCap).toBe(true);
    expect(preflight.fileCountWithinCap).toBe(true);
    expect(preflight.scanAttestationPresent).toBe(true);
    expect(preflight.privacyReviewPresent).toBe(true);
    expect(preflight.uploaderTermsAccepted).toBe(true);

    // Held inert by default-OFF flag.
    expect(preflight.intakeGate.state).toBe("inert_disabled");
    expect(preflight.intakeGate.flagName).toBe(
      "EXTERNAL_REPO_STUDY_SELF_SERVE_UPLOAD_ENABLED",
    );

    // Inert/no-claim guarantees.
    expect(preflight.intakeAdmitted).toBe(false);
    expect(preflight.ingested).toBe(false);
    expect(preflight.effectsApplied).toBe(false);
    expect(preflight.intakeGate.effectsApplied).toBe(false);
    expect(preflight.wouldIngestWhenArmed).toBe(false);
    expect(preflight.customerPublicClaimAllowed).toBe(false);
    expect(preflight.marketplacePackageAllowed).toBe(false);
    expect(preflight.payoutEligible).toBe(false);
    expect(preflight.sourceBoundary).toBe("customer_refs_withheld");

    // Refs-only evidence and stable hash.
    expect(preflight.repo).toBe(request.repo);
    expect(preflight.repo).not.toBe("OpenAgentsInc/openagents");
    expect(preflight.evidenceRefs).toEqual(
      expect.arrayContaining([
        request.uploadManifestDigest,
        request.customerRef,
        request.scanAttestationRef,
        request.privacyReviewRef,
      ]),
    );
    expect(preflight.preflightHash).toBe(
      openAgentsExternalRepoStudySelfServeUploadPreflightHash(preflight),
    );
    expect(
      `${preflight.safeCopy} ${preflight.unsafeCopyRefs.join(" ")}`,
    ).not.toMatch(/upload is live|payout eligible/i);
  });

  test("would-ingest-when-armed only once flag + owner signoff are present, still inert", async () => {
    const preflight = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudySelfServeUploadPreflight({
        generatedAt,
        ownerSignoffPresent: true,
        request,
        uploadFlagArmed: true,
      }),
    );

    expect(preflight.intakeGate.state).toBe("armed_ready");
    expect(preflight.wouldIngestWhenArmed).toBe(true);
    // Even armed + ready, no real effect is applied by this module.
    expect(preflight.intakeAdmitted).toBe(false);
    expect(preflight.ingested).toBe(false);
    expect(preflight.effectsApplied).toBe(false);
    expect(preflight.intakeGate.effectsApplied).toBe(false);
  });

  test("armed without owner signoff blocks the intake gate", async () => {
    const preflight = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudySelfServeUploadPreflight({
        generatedAt,
        request,
        uploadFlagArmed: true,
      }),
    );

    expect(preflight.intakeGate.state).toBe("armed_blocked");
    expect(preflight.intakeGate.blockedReasonRefs).toContain(
      "upload.blocked.owner_signoff_missing",
    );
    expect(preflight.wouldIngestWhenArmed).toBe(false);
    expect(preflight.ingested).toBe(false);
  });

  test("blocks when the uploader has not accepted the upload terms", async () => {
    const preflight = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudySelfServeUploadPreflight({
        generatedAt,
        request: { ...request, uploaderTermsAccepted: false },
      }),
    );

    expect(preflight.uploaderTermsAccepted).toBe(false);
    expect(preflight.state).toBe("blocked");
    expect(preflight.blockerRefs).toContain(
      "blocker.external_repo_study_self_serve_upload.uploader_terms_not_accepted",
    );
  });

  test("blocks when the clean-scan attestation or privacy review is missing", async () => {
    const preflight = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudySelfServeUploadPreflight({
        generatedAt,
        request: {
          ...request,
          scanAttestationRef: undefined,
          privacyReviewRef: undefined,
        },
      }),
    );

    expect(preflight.scanAttestationPresent).toBe(false);
    expect(preflight.privacyReviewPresent).toBe(false);
    expect(preflight.state).toBe("blocked");
    expect(preflight.blockerRefs).toContain(
      "blocker.external_repo_study_self_serve_upload.clean_scan_attestation_missing",
    );
    expect(preflight.blockerRefs).toContain(
      "blocker.external_repo_study_self_serve_upload.privacy_review_missing",
    );
  });

  test("blocks when declared size or file count is out of bounds", async () => {
    const preflight = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudySelfServeUploadPreflight({
        generatedAt,
        request: {
          ...request,
          declaredByteSize: SELF_SERVE_UPLOAD_MAX_DECLARED_BYTES + 1,
          fileCount: SELF_SERVE_UPLOAD_MAX_FILE_COUNT + 1,
        },
      }),
    );

    expect(preflight.declaredByteSizeWithinCap).toBe(false);
    expect(preflight.fileCountWithinCap).toBe(false);
    expect(preflight.state).toBe("blocked");
    expect(preflight.blockerRefs).toContain(
      "blocker.external_repo_study_self_serve_upload.declared_byte_size_out_of_bounds",
    );
    expect(preflight.blockerRefs).toContain(
      "blocker.external_repo_study_self_serve_upload.file_count_out_of_bounds",
    );
  });

  test("rejects an OpenAgents repo as a self-serve upload target", async () => {
    const exit = await Effect.runPromiseExit(
      buildOpenAgentsExternalRepoStudySelfServeUploadPreflight({
        generatedAt,
        request: { ...request, repo: "OpenAgentsInc/openagents" },
      }),
    );
    expect(exit._tag).toBe("Failure");
  });

  test("never leaks private content into the public preflight projection", async () => {
    const preflight = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudySelfServeUploadPreflight({
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

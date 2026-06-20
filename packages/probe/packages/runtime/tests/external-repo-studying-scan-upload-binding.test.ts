import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsExternalRepoStudyScanAttestationRegistry,
  buildOpenAgentsExternalRepoStudyScanUploadBinding,
  openAgentsExternalRepoStudyScanUploadBindingHash,
  type ExternalRepoStudyScanAttestationInput,
  type ExternalRepoStudySelfServeUploadRequest,
} from "../src";

const generatedAt = "2026-06-20T00:00:00.000Z" as const;

const MANIFEST_DIGEST =
  "sha256:1111111111111111111111111111111111111111111111111111111111111111" as const;

const cleanAttestation: ExternalRepoStudyScanAttestationInput = {
  customerRef: "customer.examplecorp.v0",
  findingsCount: 0,
  repo: "ExampleCorp/widget-service",
  scannerRef: "scanner.secret_malware.v0",
  uploadManifestDigest: MANIFEST_DIGEST,
  verdict: "clean",
};

// A REFS-ONLY upload request MINUS its scanAttestationRef. The binding derives
// the ref from a registry-verified clean attestation; callers cannot inject
// their own. privacyReviewRef is supplied so the OTHER gate is satisfied and the
// upload can reach intake_ready_held once the scan gate is bound.
const uploadRequest: Omit<
  ExternalRepoStudySelfServeUploadRequest,
  "scanAttestationRef"
> = {
  customerRef: "customer.examplecorp.v0",
  uploaderRef: "contributor.pylon.448ba824.v0",
  repo: "ExampleCorp/widget-service",
  uploadManifestDigest: MANIFEST_DIGEST,
  declaredByteSize: 4 * 1024 * 1024,
  fileCount: 128,
  privacyReviewRef: "external_repo_study_privacy_review_preflight.examplecorp.v0",
  uploaderTermsAccepted: true,
};

const SECRET_STRINGS = [
  "gold_answer",
  "rubric_claim",
  "evidence_excerpt",
  "archive bytes",
  "raw customer data",
];

const buildRegistry = (
  attestations: ReadonlyArray<ExternalRepoStudyScanAttestationInput> = [
    cleanAttestation,
  ],
) =>
  buildOpenAgentsExternalRepoStudyScanAttestationRegistry({
    attestations,
    generatedAt,
  });

describe("external repo studying scan upload binding", () => {
  test("binds the upload to a registry-verified clean attestation and stays inert", async () => {
    const registry = await Effect.runPromise(buildRegistry());
    const recordedRef = registry.recordedAttestations[0]!.attestationRef;

    const binding = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyScanUploadBinding({
        generatedAt,
        scanAttestationCandidateRef: recordedRef,
        scanAttestationRegistry: registry,
        uploadRequest,
      }),
    );

    expect(binding.schemaRef).toBe(
      "openagents.external_repo_study_scan_upload_binding.v0",
    );
    expect(binding.state).toBe("bound_held");
    expect(binding.bound).toBe(true);
    expect(binding.scanAttestationVerified).toBe(true);
    expect(binding.mismatchRefs).toEqual([]);

    // The bound ref is the registry-recorded one; the upload consumed it.
    expect(binding.scanAttestationRef).toBe(recordedRef);
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
      openAgentsExternalRepoStudyScanUploadBindingHash(binding),
    );
  });

  test("a forged ref binds nothing; the upload blocks on a missing clean scan", async () => {
    const registry = await Effect.runPromise(buildRegistry());
    const binding = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyScanUploadBinding({
        generatedAt,
        scanAttestationCandidateRef:
          "scan_attestation.forged.deadbeef.v0",
        scanAttestationRegistry: registry,
        uploadRequest,
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.bound).toBe(false);
    expect(binding.scanAttestationVerified).toBe(false);
    expect(binding.scanAttestationRef).toBeNull();
    expect(binding.mismatchRefs).toContain(
      "blocker.external_repo_study_scan_upload_binding.clean_scan_attestation_not_verified",
    );

    // The upload's clean-scan gate is NOT satisfied by any forged string.
    expect(binding.uploadPreflight.scanAttestationPresent).toBe(false);
    expect(binding.uploadPreflight.state).toBe("blocked");
    expect(binding.uploadPreflight.blockerRefs).toContain(
      "blocker.external_repo_study_self_serve_upload.clean_scan_attestation_missing",
    );
  });

  test("a clean attestation for a DIFFERENT manifest does not bind", async () => {
    const registry = await Effect.runPromise(buildRegistry());
    const recordedRef = registry.recordedAttestations[0]!.attestationRef;

    const binding = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyScanUploadBinding({
        generatedAt,
        scanAttestationCandidateRef: recordedRef,
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

  test("a non-clean (findings) attestation never binds", async () => {
    const registry = await Effect.runPromise(
      buildRegistry([
        {
          ...cleanAttestation,
          findingsCount: 3,
          verdict: "findings",
        },
      ]),
    );
    const recordedRef = registry.recordedAttestations[0]!.attestationRef;

    const binding = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyScanUploadBinding({
        generatedAt,
        scanAttestationCandidateRef: recordedRef,
        scanAttestationRegistry: registry,
        uploadRequest,
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.scanAttestationVerified).toBe(false);
    expect(binding.uploadPreflight.scanAttestationPresent).toBe(false);
  });

  test("even armed with owner signoff, no real effect is applied", async () => {
    const registry = await Effect.runPromise(buildRegistry());
    const recordedRef = registry.recordedAttestations[0]!.attestationRef;

    const binding = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyScanUploadBinding({
        generatedAt,
        ownerSignoffPresent: true,
        scanAttestationCandidateRef: recordedRef,
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
    const registry = await Effect.runPromise(buildRegistry());
    const recordedRef = registry.recordedAttestations[0]!.attestationRef;
    const exit = await Effect.runPromiseExit(
      buildOpenAgentsExternalRepoStudyScanUploadBinding({
        generatedAt,
        scanAttestationCandidateRef: recordedRef,
        scanAttestationRegistry: registry,
        uploadRequest: { ...uploadRequest, repo: "OpenAgentsInc/openagents" },
      }),
    );
    expect(exit._tag).toBe("Failure");
  });

  test("never leaks private content into the public binding projection", async () => {
    const registry = await Effect.runPromise(buildRegistry());
    const recordedRef = registry.recordedAttestations[0]!.attestationRef;
    const binding = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyScanUploadBinding({
        generatedAt,
        scanAttestationCandidateRef: recordedRef,
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

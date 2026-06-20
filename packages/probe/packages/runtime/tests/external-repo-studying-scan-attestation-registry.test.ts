import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsExternalRepoStudyScanAttestationRegistry,
  externalRepoStudyScanAttestationDigest,
  isCleanScanAttestationRef,
  openAgentsExternalRepoStudyScanAttestationRegistryHash,
  type ExternalRepoStudyScanAttestationInput,
} from "../src";

const generatedAt = "2026-06-20T00:00:00.000Z" as const;

const MANIFEST_DIGEST =
  "sha256:1111111111111111111111111111111111111111111111111111111111111111" as const;

const SECRET_STRINGS = [
  "gold_answer",
  "rubric_claim",
  "evidence_excerpt",
  "raw customer data",
];

const cleanAttestation: ExternalRepoStudyScanAttestationInput = {
  customerRef: "customer.acme.v0",
  findingsCount: 0,
  repo: "ExampleCorp/widget-service",
  scannerRef: "scanner.secret_malware.v0",
  uploadManifestDigest: MANIFEST_DIGEST,
  verdict: "clean",
};

describe("external repo studying scan attestation registry", () => {
  test("records a clean attestation as a content-hashed, inert reference", async () => {
    const registry = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyScanAttestationRegistry({
        attestations: [cleanAttestation],
        generatedAt,
      }),
    );

    expect(registry.schemaRef).toBe(
      "openagents.external_repo_study_scan_attestation_registry.v0",
    );
    expect(registry.sourceBoundary).toBe("customer_refs_withheld");
    expect(registry.recordedAttestations).toHaveLength(1);

    const recorded = registry.recordedAttestations[0]!;
    expect(recorded.verdict).toBe("clean");
    expect(recorded.findingsCount).toBe(0);
    expect(recorded.attestationDigest).toBe(
      externalRepoStudyScanAttestationDigest(cleanAttestation),
    );
    expect(recorded.attestationRef.startsWith("scan_attestation.")).toBe(true);

    // Deterministic registry hash.
    expect(registry.registryHash).toBe(
      openAgentsExternalRepoStudyScanAttestationRegistryHash(registry),
    );

    // No-claim / inert guarantees.
    expect(registry.effectsApplied).toBe(false);
    expect(registry.customerPublicClaimAllowed).toBe(false);
    expect(registry.marketplacePackageAllowed).toBe(false);
    expect(registry.payoutEligible).toBe(false);
  });

  test("an empty registry verifies nothing", async () => {
    const registry = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyScanAttestationRegistry({ generatedAt }),
    );
    expect(registry.recordedAttestations).toHaveLength(0);
    expect(
      isCleanScanAttestationRef(registry, "scan_attestation.anything.v0", {
        customerRef: cleanAttestation.customerRef,
        repo: cleanAttestation.repo,
        uploadManifestDigest: MANIFEST_DIGEST,
      }),
    ).toBe(false);
  });

  test("isCleanScanAttestationRef accepts only a known clean ref for the exact manifest", async () => {
    const registry = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyScanAttestationRegistry({
        attestations: [cleanAttestation],
        generatedAt,
      }),
    );
    const ref = registry.recordedAttestations[0]!.attestationRef;
    const match = {
      customerRef: cleanAttestation.customerRef,
      repo: cleanAttestation.repo,
      uploadManifestDigest: MANIFEST_DIGEST,
    };

    // Exact match verifies.
    expect(isCleanScanAttestationRef(registry, ref, match)).toBe(true);

    // Forged / unknown / empty refs do not verify.
    expect(
      isCleanScanAttestationRef(registry, "scan_attestation.forged.v0", match),
    ).toBe(false);
    expect(isCleanScanAttestationRef(registry, undefined, match)).toBe(false);
    expect(isCleanScanAttestationRef(registry, "  ", match)).toBe(false);

    // Customer / repo / manifest mismatches do not verify, even with a real ref.
    expect(
      isCleanScanAttestationRef(registry, ref, {
        ...match,
        customerRef: "customer.other.v0",
      }),
    ).toBe(false);
    expect(
      isCleanScanAttestationRef(registry, ref, {
        ...match,
        repo: "ExampleCorp/other-service",
      }),
    ).toBe(false);
    expect(
      isCleanScanAttestationRef(registry, ref, {
        ...match,
        uploadManifestDigest:
          "sha256:2222222222222222222222222222222222222222222222222222222222222222",
      }),
    ).toBe(false);
  });

  test("a findings verdict is recorded but never verifies as clean", async () => {
    const findings: ExternalRepoStudyScanAttestationInput = {
      ...cleanAttestation,
      findingsCount: 3,
      verdict: "findings",
    };
    const registry = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyScanAttestationRegistry({
        attestations: [findings],
        generatedAt,
      }),
    );
    const ref = registry.recordedAttestations[0]!.attestationRef;
    expect(registry.recordedAttestations[0]!.verdict).toBe("findings");
    expect(
      isCleanScanAttestationRef(registry, ref, {
        customerRef: findings.customerRef,
        repo: findings.repo,
        uploadManifestDigest: MANIFEST_DIGEST,
      }),
    ).toBe(false);
  });

  test("rejects inconsistent verdict/findings counts", async () => {
    await expect(
      Effect.runPromise(
        buildOpenAgentsExternalRepoStudyScanAttestationRegistry({
          attestations: [{ ...cleanAttestation, findingsCount: 2 }],
          generatedAt,
        }),
      ),
    ).rejects.toThrow();

    await expect(
      Effect.runPromise(
        buildOpenAgentsExternalRepoStudyScanAttestationRegistry({
          attestations: [
            { ...cleanAttestation, findingsCount: 0, verdict: "findings" },
          ],
          generatedAt,
        }),
      ),
    ).rejects.toThrow();
  });

  test("refuses to record an attestation for the OpenAgents repo", async () => {
    await expect(
      Effect.runPromise(
        buildOpenAgentsExternalRepoStudyScanAttestationRegistry({
          attestations: [
            { ...cleanAttestation, repo: "OpenAgentsInc/openagents" },
          ],
          generatedAt,
        }),
      ),
    ).rejects.toThrow();
  });

  test("serialization leaks no private study material", async () => {
    const registry = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyScanAttestationRegistry({
        attestations: [cleanAttestation],
        generatedAt,
      }),
    );
    const serialized = JSON.stringify(registry);
    for (const secret of SECRET_STRINGS) {
      expect(serialized.includes(secret)).toBe(false);
    }
  });
});

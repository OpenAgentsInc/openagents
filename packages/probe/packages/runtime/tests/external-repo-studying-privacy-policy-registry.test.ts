import { readFile } from "node:fs/promises";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsExternalRepoStudyPrivacyPolicyRegistry,
  EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_DOCUMENT_DIGEST,
  EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_DOCUMENT_PATH,
  EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
  externalRepoStudyPrivacyPolicyDocumentDigest,
  isMatchingPublishedExternalRepoStudyPrivacyPolicyDocument,
  isPublishedExternalRepoStudyPrivacyPolicyRef,
  openAgentsExternalRepoStudyPrivacyPolicyRegistryHash,
  PRIVACY_REVIEW_MAX_RETENTION_DAYS,
} from "../src";

const generatedAt = "2026-06-20T00:00:00.000Z" as const;

const POLICY_V0_DOCUMENT_URL = new URL(
  `../../../../../${EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_DOCUMENT_PATH}`,
  import.meta.url,
);

const SECRET_STRINGS = [
  "gold_answer",
  "rubric_claim",
  "evidence_excerpt",
  "raw customer data",
];

describe("external repo studying privacy policy registry", () => {
  test("publishes the v0 policy as a canonical, content-hashed, inert reference", async () => {
    const registry = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyPrivacyPolicyRegistry({ generatedAt }),
    );

    expect(registry.schemaRef).toBe(
      "openagents.external_repo_study_privacy_policy_registry.v0",
    );
    expect(registry.sourceBoundary).toBe("customer_refs_withheld");
    expect(registry.publishedVersions).toHaveLength(1);

    const v0 = registry.publishedVersions[0]!;
    expect(v0.policyRef).toBe(EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF);
    expect(v0.documentPath).toBe(
      EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_DOCUMENT_PATH,
    );
    expect(v0.status).toBe("published_inert");
    expect(v0.termsDigest.startsWith("sha256:")).toBe(true);

    // Stable, deterministic registry hash.
    expect(registry.registryHash).toBe(
      openAgentsExternalRepoStudyPrivacyPolicyRegistryHash(registry),
    );

    // No-claim guarantees.
    expect(registry.effectsApplied).toBe(false);
    expect(registry.customerPublicClaimAllowed).toBe(false);
    expect(registry.marketplacePackageAllowed).toBe(false);
    expect(registry.payoutEligible).toBe(false);
  });

  test("published caps mirror what the privacy-review preflight enforces", async () => {
    const registry = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyPrivacyPolicyRegistry({ generatedAt }),
    );
    const v0 = registry.publishedVersions[0]!;

    // The published policy stays in lockstep with the enforced retention cap.
    expect(v0.terms.maxRetentionDays).toBe(PRIVACY_REVIEW_MAX_RETENTION_DAYS);

    // And with the closed PII-category set enforced by the preflight.
    expect([...v0.terms.allowedPiiCategories].sort()).toEqual(
      [
        "code_comment_text",
        "commit_author_email",
        "contributor_handle",
        "none",
      ].sort(),
    );

    expect(v0.terms.customerAuthorizationRequired).toBe(true);
    expect(v0.terms.dataProcessingAgreementRequired).toBe(true);
    expect(v0.terms.inertByDefault).toBe(true);
  });

  test("accepts a known published policy ref and rejects unknown/empty ones", async () => {
    const registry = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyPrivacyPolicyRegistry({ generatedAt }),
    );

    expect(
      isPublishedExternalRepoStudyPrivacyPolicyRef(
        registry,
        EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
      ),
    ).toBe(true);

    // A forged / stale / empty ref does not satisfy the published-policy check.
    expect(
      isPublishedExternalRepoStudyPrivacyPolicyRef(
        registry,
        "policy.external_repo_study_privacy.forged",
      ),
    ).toBe(false);
    expect(isPublishedExternalRepoStudyPrivacyPolicyRef(registry, "")).toBe(
      false,
    );
    expect(
      isPublishedExternalRepoStudyPrivacyPolicyRef(registry, undefined),
    ).toBe(false);
  });

  test("is deterministic across builds (stable hash, refs only)", async () => {
    const a = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyPrivacyPolicyRegistry({ generatedAt }),
    );
    const b = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyPrivacyPolicyRegistry({
        generatedAt: "2099-01-01T00:00:00.000Z",
      }),
    );
    // generatedAt is excluded from the hash, so the registry hash is stable.
    expect(a.registryHash).toBe(b.registryHash);
  });

  test("pins the on-disk policy document content (no doc<->registry drift)", async () => {
    const registry = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyPrivacyPolicyRegistry({ generatedAt }),
    );
    const v0 = registry.publishedVersions[0]!;

    const documentText = await readFile(POLICY_V0_DOCUMENT_URL, "utf8");
    const onDiskDigest =
      externalRepoStudyPrivacyPolicyDocumentDigest(documentText);

    // The published digest matches the exported constant...
    expect(v0.documentDigest).toBe(
      EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_DOCUMENT_DIGEST,
    );
    // ...and the actual canonical document on disk. If the legal text drifts
    // from the registry (or vice versa), this assertion fails in CI.
    expect(v0.documentDigest).toBe(onDiskDigest);
    expect(v0.documentDigest.startsWith("sha256:")).toBe(true);
  });

  test("verifies served policy text against the pinned document digest", async () => {
    const registry = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyPrivacyPolicyRegistry({ generatedAt }),
    );
    const documentText = await readFile(POLICY_V0_DOCUMENT_URL, "utf8");

    // The exact canonical text verifies for the known ref.
    expect(
      isMatchingPublishedExternalRepoStudyPrivacyPolicyDocument(
        registry,
        EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
        documentText,
      ),
    ).toBe(true);

    // Tampered text (even a single appended byte) does not verify.
    expect(
      isMatchingPublishedExternalRepoStudyPrivacyPolicyDocument(
        registry,
        EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
        `${documentText}\n<!-- tampered -->`,
      ),
    ).toBe(false);

    // Correct text under an unknown / empty ref does not verify.
    expect(
      isMatchingPublishedExternalRepoStudyPrivacyPolicyDocument(
        registry,
        "policy.external_repo_study_privacy.forged",
        documentText,
      ),
    ).toBe(false);
    expect(
      isMatchingPublishedExternalRepoStudyPrivacyPolicyDocument(
        registry,
        "",
        documentText,
      ),
    ).toBe(false);
  });

  test("never leaks private content into the public registry projection", async () => {
    const registry = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyPrivacyPolicyRegistry({ generatedAt }),
    );
    const serialized = JSON.stringify(registry);
    for (const secret of SECRET_STRINGS) {
      expect(serialized).not.toContain(secret);
    }
  });
});

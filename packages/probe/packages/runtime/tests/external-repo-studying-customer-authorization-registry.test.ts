import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsExternalRepoStudyCustomerAuthorizationRegistry,
  externalRepoStudyCustomerAuthorizationDigest,
  isActiveCustomerAuthorizationRef,
  openAgentsExternalRepoStudyCustomerAuthorizationRegistryHash,
  type ExternalRepoStudyCustomerAuthorizationInput,
} from "../src";

const generatedAt = "2026-06-20T00:00:00.000Z" as const;

const SECRET_STRINGS = [
  "gold_answer",
  "rubric_claim",
  "evidence_excerpt",
  "raw customer data",
];

const activeAuthorization: ExternalRepoStudyCustomerAuthorizationInput = {
  customerRef: "customer.acme.v0",
  effectiveDate: "2026-06-20",
  grantRef: "grant.acme.widget.v0",
  repo: "ExampleCorp/widget-service",
  scope: "external_repo_study",
  status: "active",
};

describe("external repo studying customer authorization registry", () => {
  test("records an active authorization as a content-hashed, inert reference", async () => {
    const registry = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyCustomerAuthorizationRegistry({
        authorizations: [activeAuthorization],
        generatedAt,
      }),
    );

    expect(registry.schemaRef).toBe(
      "openagents.external_repo_study_customer_authorization_registry.v0",
    );
    expect(registry.sourceBoundary).toBe("customer_refs_withheld");
    expect(registry.recordedAuthorizations).toHaveLength(1);

    const recorded = registry.recordedAuthorizations[0]!;
    expect(recorded.status).toBe("active");
    expect(recorded.scope).toBe("external_repo_study");
    expect(recorded.authorizationDigest).toBe(
      externalRepoStudyCustomerAuthorizationDigest(activeAuthorization),
    );
    expect(recorded.authorizationRef.startsWith("customer_authorization.")).toBe(
      true,
    );

    expect(registry.registryHash).toBe(
      openAgentsExternalRepoStudyCustomerAuthorizationRegistryHash(registry),
    );

    expect(registry.effectsApplied).toBe(false);
    expect(registry.customerPublicClaimAllowed).toBe(false);
    expect(registry.marketplacePackageAllowed).toBe(false);
    expect(registry.payoutEligible).toBe(false);
  });

  test("an empty registry verifies nothing", async () => {
    const registry = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyCustomerAuthorizationRegistry({
        generatedAt,
      }),
    );
    expect(registry.recordedAuthorizations).toHaveLength(0);
    expect(
      isActiveCustomerAuthorizationRef(
        registry,
        "customer_authorization.anything.v0",
        {
          customerRef: activeAuthorization.customerRef,
          repo: activeAuthorization.repo,
        },
      ),
    ).toBe(false);
  });

  test("isActiveCustomerAuthorizationRef accepts only a known active ref for the exact customer+repo", async () => {
    const registry = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyCustomerAuthorizationRegistry({
        authorizations: [activeAuthorization],
        generatedAt,
      }),
    );
    const ref = registry.recordedAuthorizations[0]!.authorizationRef;
    const match = {
      customerRef: activeAuthorization.customerRef,
      repo: activeAuthorization.repo,
    };

    expect(isActiveCustomerAuthorizationRef(registry, ref, match)).toBe(true);

    // Forged / unknown / empty refs do not verify.
    expect(
      isActiveCustomerAuthorizationRef(
        registry,
        "customer_authorization.forged.v0",
        match,
      ),
    ).toBe(false);
    expect(isActiveCustomerAuthorizationRef(registry, undefined, match)).toBe(
      false,
    );
    expect(isActiveCustomerAuthorizationRef(registry, "  ", match)).toBe(false);

    // Customer / repo mismatches do not verify, even with a real ref.
    expect(
      isActiveCustomerAuthorizationRef(registry, ref, {
        ...match,
        customerRef: "customer.other.v0",
      }),
    ).toBe(false);
    expect(
      isActiveCustomerAuthorizationRef(registry, ref, {
        ...match,
        repo: "ExampleCorp/other-service",
      }),
    ).toBe(false);
  });

  test("a withdrawn authorization is recorded but never verifies as active", async () => {
    const withdrawn: ExternalRepoStudyCustomerAuthorizationInput = {
      ...activeAuthorization,
      effectiveDate: "2026-06-21",
      status: "withdrawn",
    };
    const registry = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyCustomerAuthorizationRegistry({
        authorizations: [withdrawn],
        generatedAt,
      }),
    );
    const ref = registry.recordedAuthorizations[0]!.authorizationRef;
    expect(registry.recordedAuthorizations[0]!.status).toBe("withdrawn");
    expect(
      isActiveCustomerAuthorizationRef(registry, ref, {
        customerRef: withdrawn.customerRef,
        repo: withdrawn.repo,
      }),
    ).toBe(false);
  });

  test("refuses to record an authorization for the OpenAgents repo", async () => {
    await expect(
      Effect.runPromise(
        buildOpenAgentsExternalRepoStudyCustomerAuthorizationRegistry({
          authorizations: [
            { ...activeAuthorization, repo: "OpenAgentsInc/openagents" },
          ],
          generatedAt,
        }),
      ),
    ).rejects.toThrow();
  });

  test("serialization leaks no private study material", async () => {
    const registry = await Effect.runPromise(
      buildOpenAgentsExternalRepoStudyCustomerAuthorizationRegistry({
        authorizations: [activeAuthorization],
        generatedAt,
      }),
    );
    const serialized = JSON.stringify(registry);
    for (const secret of SECRET_STRINGS) {
      expect(serialized.includes(secret)).toBe(false);
    }
  });
});

import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsExternalRepoStudyCustomerAuthorizationRegistry,
  buildOpenAgentsExternalRepoStudyDsrAuthorizationBinding,
  buildOpenAgentsExternalRepoStudyPrivacyPolicyRegistry,
  EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
  openAgentsExternalRepoStudyDsrAuthorizationBindingHash,
  type ExternalRepoStudyCustomerAuthorizationInput,
  type ExternalRepoStudyDataSubjectRequest,
  type OpenAgentsExternalRepoStudyCustomerAuthorizationRegistry,
} from "../src";

const generatedAt = "2026-06-20T00:00:00.000Z" as const;

const SECRET_STRINGS = [
  "gold_answer",
  "rubric_claim",
  "evidence_excerpt",
  "raw customer data",
  "jane.doe@example.com",
];

type DsrRequestSansAuthRef = Omit<
  ExternalRepoStudyDataSubjectRequest,
  "customerAuthorizationRef"
>;

const baseRequest: DsrRequestSansAuthRef = {
  customerRef: "customer.acme.v0",
  repo: "ExampleCorp/widget-service",
  requestRef: "dsr.acme.2026-06-20.v0",
  subjectRef: "subject.acme.opaque.v0",
  requestType: "erasure",
};

const activeAuthorization: ExternalRepoStudyCustomerAuthorizationInput = {
  customerRef: "customer.acme.v0",
  effectiveDate: "2026-06-20",
  grantRef: "grant.acme.widget.v0",
  repo: "ExampleCorp/widget-service",
  scope: "external_repo_study",
  status: "active",
};

const authRegistryWith = (
  authorizations: ReadonlyArray<ExternalRepoStudyCustomerAuthorizationInput>,
) =>
  buildOpenAgentsExternalRepoStudyCustomerAuthorizationRegistry({
    authorizations,
    generatedAt,
  });

const policyRegistry = () =>
  buildOpenAgentsExternalRepoStudyPrivacyPolicyRegistry({ generatedAt });

const activeRefOf = (
  registry: OpenAgentsExternalRepoStudyCustomerAuthorizationRegistry,
): string => registry.recordedAuthorizations[0]!.authorizationRef;

describe("external repo studying DSR<->authorization binding", () => {
  test("binds the authorization ref from a known active authorization, held inert", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const authReg = yield* authRegistryWith([activeAuthorization]);
        const policyReg = yield* policyRegistry();
        return yield* buildOpenAgentsExternalRepoStudyDsrAuthorizationBinding({
          authorizationRegistry: authReg,
          authorizationCandidateRef: activeRefOf(authReg),
          generatedAt,
          policyRegistry: policyReg,
          policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
          request: baseRequest,
        });
      }),
    );

    expect(binding.schemaRef).toBe(
      "openagents.external_repo_study_dsr_authorization_binding.v0",
    );
    expect(binding.state).toBe("bound_held");
    expect(binding.bound).toBe(true);
    expect(binding.authorizationActive).toBe(true);
    expect(binding.authorizationRef).toBe(binding.authorizationCandidateRef);

    // The nested DSR preflight sees the derived ref as present and is admitted.
    expect(binding.dsrPreflight.customerAuthorizationPresent).toBe(true);
    expect(binding.dsrPreflight.state).toBe("request_ready_held");

    // Inert by construction.
    expect(binding.requestHonored).toBe(false);
    expect(binding.dataExported).toBe(false);
    expect(binding.dataErased).toBe(false);
    expect(binding.authorizationWithdrawn).toBe(false);
    expect(binding.effectsApplied).toBe(false);
    expect(binding.customerPublicClaimAllowed).toBe(false);
    expect(binding.marketplacePackageAllowed).toBe(false);
    expect(binding.payoutEligible).toBe(false);
    expect(binding.dsrPreflight.requestHonored).toBe(false);
    expect(binding.bindingHash).toBe(
      openAgentsExternalRepoStudyDsrAuthorizationBindingHash(binding),
    );
  });

  test("a forged / unknown authorization ref binds nothing and blocks the request", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const authReg = yield* authRegistryWith([activeAuthorization]);
        const policyReg = yield* policyRegistry();
        return yield* buildOpenAgentsExternalRepoStudyDsrAuthorizationBinding({
          authorizationRegistry: authReg,
          authorizationCandidateRef: "customer_authorization.forged.v0",
          generatedAt,
          policyRegistry: policyReg,
          policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
          request: baseRequest,
        });
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.bound).toBe(false);
    expect(binding.authorizationActive).toBe(false);
    expect(binding.authorizationRef).toBeNull();
    expect(binding.mismatchRefs).toContain(
      "blocker.external_repo_study_dsr_authorization_binding.authorization_not_active",
    );

    // The DSR cannot present an authorization and is blocked.
    expect(binding.dsrPreflight.customerAuthorizationPresent).toBe(false);
    expect(binding.dsrPreflight.state).toBe("blocked");
    expect(binding.dsrPreflight.acknowledgementRef).toBeNull();
    expect(binding.dsrPreflight.blockerRefs).toContain(
      "blocker.external_repo_study_data_subject_request.customer_authorization_missing",
    );
  });

  test("a withdrawn authorization never binds (Section 6 revocation)", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const authReg = yield* authRegistryWith([
          { ...activeAuthorization, status: "withdrawn" },
        ]);
        const policyReg = yield* policyRegistry();
        return yield* buildOpenAgentsExternalRepoStudyDsrAuthorizationBinding({
          authorizationRegistry: authReg,
          authorizationCandidateRef: activeRefOf(authReg),
          generatedAt,
          policyRegistry: policyReg,
          policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
          request: baseRequest,
        });
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.authorizationRef).toBeNull();
    expect(binding.dsrPreflight.customerAuthorizationPresent).toBe(false);
    expect(binding.dsrPreflight.state).toBe("blocked");
  });

  test("a customer mismatch does not bind even with a real active ref", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const authReg = yield* authRegistryWith([activeAuthorization]);
        const policyReg = yield* policyRegistry();
        return yield* buildOpenAgentsExternalRepoStudyDsrAuthorizationBinding({
          authorizationRegistry: authReg,
          authorizationCandidateRef: activeRefOf(authReg),
          generatedAt,
          policyRegistry: policyReg,
          policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
          request: { ...baseRequest, customerRef: "customer.other.v0" },
        });
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.authorizationRef).toBeNull();
    expect(binding.dsrPreflight.customerAuthorizationPresent).toBe(false);
  });

  test("a repo mismatch does not bind even with a real active ref", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const authReg = yield* authRegistryWith([activeAuthorization]);
        const policyReg = yield* policyRegistry();
        return yield* buildOpenAgentsExternalRepoStudyDsrAuthorizationBinding({
          authorizationRegistry: authReg,
          authorizationCandidateRef: activeRefOf(authReg),
          generatedAt,
          policyRegistry: policyReg,
          policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
          request: { ...baseRequest, repo: "ExampleCorp/other-service" },
        });
      }),
    );

    expect(binding.state).toBe("unbound");
    expect(binding.authorizationRef).toBeNull();
    expect(binding.dsrPreflight.customerAuthorizationPresent).toBe(false);
  });

  test("armed + handler sign-off is would-fulfil-when-armed but still inert", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const authReg = yield* authRegistryWith([activeAuthorization]);
        const policyReg = yield* policyRegistry();
        return yield* buildOpenAgentsExternalRepoStudyDsrAuthorizationBinding({
          authorizationRegistry: authReg,
          authorizationCandidateRef: activeRefOf(authReg),
          generatedAt,
          handlerSignoffPresent: true,
          policyRegistry: policyReg,
          policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
          request: baseRequest,
          requestFlagArmed: true,
        });
      }),
    );

    expect(binding.wouldFulfillWhenArmed).toBe(true);
    expect(binding.dsrPreflight.fulfilmentGate.state).toBe("armed_ready");
    // Still inert even when armed.
    expect(binding.requestHonored).toBe(false);
    expect(binding.dataErased).toBe(false);
    expect(binding.effectsApplied).toBe(false);
    expect(binding.dsrPreflight.requestHonored).toBe(false);
    expect(binding.dsrPreflight.effectsApplied).toBe(false);
  });

  test("rejects the OpenAgents repo as a binding target", async () => {
    const program = Effect.gen(function* () {
      const authReg = yield* authRegistryWith([activeAuthorization]);
      const policyReg = yield* policyRegistry();
      return yield* buildOpenAgentsExternalRepoStudyDsrAuthorizationBinding({
        authorizationRegistry: authReg,
        authorizationCandidateRef: activeRefOf(authReg),
        generatedAt,
        policyRegistry: policyReg,
        policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
        request: { ...baseRequest, repo: "OpenAgentsInc/openagents" },
      });
    });

    await expect(Effect.runPromise(program)).rejects.toThrow();
  });

  test("never leaks private content into the public binding projection", async () => {
    const binding = await Effect.runPromise(
      Effect.gen(function* () {
        const authReg = yield* authRegistryWith([activeAuthorization]);
        const policyReg = yield* policyRegistry();
        return yield* buildOpenAgentsExternalRepoStudyDsrAuthorizationBinding({
          authorizationRegistry: authReg,
          authorizationCandidateRef: activeRefOf(authReg),
          generatedAt,
          policyRegistry: policyReg,
          policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
          request: baseRequest,
        });
      }),
    );
    const serialized = JSON.stringify(binding);
    for (const secret of SECRET_STRINGS) {
      expect(serialized).not.toContain(secret);
    }
  });
});

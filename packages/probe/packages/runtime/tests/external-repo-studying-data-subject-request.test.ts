import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildOpenAgentsExternalRepoStudyDataSubjectRequestPreflight,
  buildOpenAgentsExternalRepoStudyPrivacyPolicyRegistry,
  EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
  openAgentsExternalRepoStudyDataSubjectRequestPreflightHash,
  type ExternalRepoStudyDataSubjectRequest,
} from "../src";

const generatedAt = "2026-06-20T00:00:00.000Z" as const;

const SECRET_STRINGS = [
  "gold_answer",
  "rubric_claim",
  "evidence_excerpt",
  "raw customer data",
  "jane.doe@example.com",
];

const baseRequest: ExternalRepoStudyDataSubjectRequest = {
  customerRef: "customer.acme.v0",
  repo: "ExampleCorp/widget-service",
  requestRef: "dsr.acme.2026-06-20.v0",
  subjectRef: "subject.acme.opaque.v0",
  requestType: "erasure",
  customerAuthorizationRef: "auth.acme.widget.v0",
};

const registry = () =>
  buildOpenAgentsExternalRepoStudyPrivacyPolicyRegistry({ generatedAt });

describe("external repo studying data-subject request preflight", () => {
  test("admits a well-formed request against a published policy, held inert", async () => {
    const preflight = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registry();
        return yield* buildOpenAgentsExternalRepoStudyDataSubjectRequestPreflight(
          {
            generatedAt,
            policyRegistry: reg,
            policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
            request: baseRequest,
          },
        );
      }),
    );

    expect(preflight.schemaRef).toBe(
      "openagents.external_repo_study_data_subject_request_preflight.v0",
    );
    expect(preflight.state).toBe("request_ready_held");
    expect(preflight.policyPublished).toBe(true);
    expect(preflight.requestRefPresent).toBe(true);
    expect(preflight.subjectRefPresent).toBe(true);
    expect(preflight.customerAuthorizationPresent).toBe(true);
    expect(preflight.requestTypeSupported).toBe(true);
    expect(preflight.acknowledgementRef?.startsWith(
      "data_subject_request_ack.",
    )).toBe(true);
    expect(preflight.blockerRefs).toEqual([]);

    // Inert by construction even though the request would be admitted.
    expect(preflight.requestHonored).toBe(false);
    expect(preflight.dataExported).toBe(false);
    expect(preflight.dataErased).toBe(false);
    expect(preflight.authorizationWithdrawn).toBe(false);
    expect(preflight.effectsApplied).toBe(false);
    expect(preflight.customerPublicClaimAllowed).toBe(false);
    expect(preflight.marketplacePackageAllowed).toBe(false);
    expect(preflight.payoutEligible).toBe(false);
    expect(preflight.fulfilmentGate.state).toBe("inert_disabled");
    expect(preflight.wouldFulfillWhenArmed).toBe(false);
    expect(preflight.preflightHash).toBe(
      openAgentsExternalRepoStudyDataSubjectRequestPreflightHash(preflight),
    );
  });

  test("a forged / unknown policy ref blocks the request", async () => {
    const preflight = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registry();
        return yield* buildOpenAgentsExternalRepoStudyDataSubjectRequestPreflight(
          {
            generatedAt,
            policyRegistry: reg,
            policyRef: "policy.external_repo_study_privacy.forged",
            request: baseRequest,
          },
        );
      }),
    );

    expect(preflight.state).toBe("blocked");
    expect(preflight.policyPublished).toBe(false);
    expect(preflight.acknowledgementRef).toBeNull();
    expect(preflight.blockerRefs).toContain(
      "blocker.external_repo_study_data_subject_request.policy_ref_not_published",
    );
  });

  test("a missing request ref blocks the request", async () => {
    const preflight = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registry();
        return yield* buildOpenAgentsExternalRepoStudyDataSubjectRequestPreflight(
          {
            generatedAt,
            policyRegistry: reg,
            policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
            request: { ...baseRequest, requestRef: "  " },
          },
        );
      }),
    );

    expect(preflight.state).toBe("blocked");
    expect(preflight.requestRefPresent).toBe(false);
    expect(preflight.acknowledgementRef).toBeNull();
    expect(preflight.blockerRefs).toContain(
      "blocker.external_repo_study_data_subject_request.request_ref_missing",
    );
  });

  test("a missing subject ref blocks the request", async () => {
    const preflight = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registry();
        return yield* buildOpenAgentsExternalRepoStudyDataSubjectRequestPreflight(
          {
            generatedAt,
            policyRegistry: reg,
            policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
            request: { ...baseRequest, subjectRef: undefined },
          },
        );
      }),
    );

    expect(preflight.state).toBe("blocked");
    expect(preflight.subjectRefPresent).toBe(false);
    expect(preflight.blockerRefs).toContain(
      "blocker.external_repo_study_data_subject_request.subject_ref_missing",
    );
  });

  test("a missing customer authorization blocks the request", async () => {
    const preflight = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registry();
        return yield* buildOpenAgentsExternalRepoStudyDataSubjectRequestPreflight(
          {
            generatedAt,
            policyRegistry: reg,
            policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
            request: { ...baseRequest, customerAuthorizationRef: undefined },
          },
        );
      }),
    );

    expect(preflight.state).toBe("blocked");
    expect(preflight.customerAuthorizationPresent).toBe(false);
    expect(preflight.blockerRefs).toContain(
      "blocker.external_repo_study_data_subject_request.customer_authorization_missing",
    );
  });

  test("each supported request type derives a distinct acknowledgement ref", async () => {
    const types = [
      "access",
      "rectification",
      "erasure",
      "authorization_withdrawal",
    ] as const;
    const refs = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registry();
        const out: Array<string | null> = [];
        for (const requestType of types) {
          const preflight =
            yield* buildOpenAgentsExternalRepoStudyDataSubjectRequestPreflight({
              generatedAt,
              policyRegistry: reg,
              policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
              request: { ...baseRequest, requestType },
            });
          expect(preflight.state).toBe("request_ready_held");
          out.push(preflight.acknowledgementRef);
        }
        return out;
      }),
    );
    expect(new Set(refs).size).toBe(types.length);
  });

  test("armed + handler sign-off is would-fulfil-when-armed but still inert", async () => {
    const preflight = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registry();
        return yield* buildOpenAgentsExternalRepoStudyDataSubjectRequestPreflight(
          {
            generatedAt,
            policyRegistry: reg,
            policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
            request: baseRequest,
            requestFlagArmed: true,
            handlerSignoffPresent: true,
          },
        );
      }),
    );

    expect(preflight.fulfilmentGate.state).toBe("armed_ready");
    expect(preflight.wouldFulfillWhenArmed).toBe(true);
    // Still inert even when armed.
    expect(preflight.requestHonored).toBe(false);
    expect(preflight.dataErased).toBe(false);
    expect(preflight.dataExported).toBe(false);
    expect(preflight.authorizationWithdrawn).toBe(false);
    expect(preflight.effectsApplied).toBe(false);
    expect(preflight.fulfilmentGate.effectsApplied).toBe(false);
  });

  test("armed without handler sign-off is blocked, not would-fulfil", async () => {
    const preflight = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registry();
        return yield* buildOpenAgentsExternalRepoStudyDataSubjectRequestPreflight(
          {
            generatedAt,
            policyRegistry: reg,
            policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
            request: baseRequest,
            requestFlagArmed: true,
            handlerSignoffPresent: false,
          },
        );
      }),
    );

    expect(preflight.fulfilmentGate.state).toBe("armed_blocked");
    expect(preflight.fulfilmentGate.blockedReasonRefs).toContain(
      "data_subject_request.blocked.handler_signoff_missing",
    );
    expect(preflight.wouldFulfillWhenArmed).toBe(false);
  });

  test("rejects the OpenAgents repo as a request target", async () => {
    const program = Effect.gen(function* () {
      const reg = yield* registry();
      return yield* buildOpenAgentsExternalRepoStudyDataSubjectRequestPreflight({
        generatedAt,
        policyRegistry: reg,
        policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
        request: { ...baseRequest, repo: "OpenAgentsInc/openagents" },
      });
    });

    await expect(Effect.runPromise(program)).rejects.toThrow();
  });

  test("never leaks private content into the public preflight projection", async () => {
    const preflight = await Effect.runPromise(
      Effect.gen(function* () {
        const reg = yield* registry();
        return yield* buildOpenAgentsExternalRepoStudyDataSubjectRequestPreflight(
          {
            generatedAt,
            policyRegistry: reg,
            policyRef: EXTERNAL_REPO_STUDY_PRIVACY_POLICY_V0_REF,
            request: baseRequest,
          },
        );
      }),
    );
    const serialized = JSON.stringify(preflight);
    for (const secret of SECRET_STRINGS) {
      expect(serialized).not.toContain(secret);
    }
  });
});

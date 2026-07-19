import { Effect } from "effect";
import { describe, expect, it } from "vite-plus/test";

import { AuthorityService, makeAuthorityServiceLayer } from "./index";

const actions = [
  "create_managed_sandbox",
  "list_managed_sandboxes",
  "inspect_managed_sandbox",
  "dispatch_managed_sandbox_work",
  "interrupt_managed_sandbox_turn",
  "stop_managed_sandbox",
  "resume_managed_sandbox",
  "delete_managed_sandbox",
] as const;

const conditions = [
  "condition.owner_scope",
  "condition.capability_broker",
  "condition.managed_sandbox_scope",
  "condition.managed_sandbox_budget",
  "condition.managed_sandbox_runtime_admission",
  "condition.verification",
  "condition.redaction",
  "condition.rollback",
] as const;

const profile = {
  profileRef: "openagents.owner-delegated-autonomy",
  revision: 6,
  lifecycle: "admitted",
  authorityMayAmplify: false,
  explicitDenyWins: true,
  grants: [
    {
      grantRef: "grant.sarah_managed_sandbox",
      roles: ["sarah_orchestrator"],
      actions,
      resources: ["authenticated_owner_openagents_managed_sandboxes"],
      programs: ["program.managed_agent_sandboxes"],
      conditionRefs: conditions,
    },
  ],
  reservedActions: ["operate_generic_container", "export_cloud_credential"],
};

const request = (overrides: Record<string, unknown> = {}) => ({
  requestRef: "request.sbx.authority.1",
  actorRef: "principal.sarah",
  actorRole: "sarah_orchestrator",
  action: "create_managed_sandbox",
  resource: "authenticated_owner_openagents_managed_sandboxes",
  programRef: "program.managed_agent_sandboxes",
  triggerRef: "owner.thread.sarah.1",
  conditionResults: conditions.map((conditionRef) => ({
    conditionRef,
    passed: true,
    evidenceRefs: [`evidence.${conditionRef}`],
  })),
  startedAt: "2026-07-19T00:00:00.000Z",
  ...overrides,
});

const resolve = (input: unknown) =>
  Effect.runPromise(
    Effect.gen(function* () {
      const authority = yield* AuthorityService;
      return yield* authority.resolve(input);
    }).pipe(Effect.provide(makeAuthorityServiceLayer(profile))),
  );

describe("Sarah managed-sandbox authority", () => {
  it("admits exactly the eight closed actions when every runtime condition passes", async () => {
    const decisions = await Promise.all(actions.map((action) => resolve(request({ action }))));
    for (const decision of decisions) {
      expect(decision).toMatchObject({
        _tag: "Allowed",
        grantRef: "grant.sarah_managed_sandbox",
      });
    }
  });

  it("refuses generic cloud/container actions and cross-program or cross-resource requests", async () => {
    const decisions = await Promise.all(
      [
        { action: "operate_generic_container" },
        { action: "run_gcloud" },
        { resource: "google_cloud_project_openagentsgemini" },
        { programRef: "program.promise_growth_revenue" },
      ].map((overrides) => resolve(request(overrides))),
    );
    for (const decision of decisions) {
      expect(decision).toMatchObject({ _tag: "Denied" });
    }
  });

  it("fails closed until target admission and every scope/budget gate pass", async () => {
    const conditionResults = request().conditionResults.map((result) =>
      result.conditionRef === "condition.managed_sandbox_runtime_admission"
        ? {
            conditionRef: result.conditionRef,
            passed: false,
            evidenceRefs: result.evidenceRefs,
          }
        : result,
    );
    await expect(resolve(request({ conditionResults }))).resolves.toMatchObject({
      _tag: "Denied",
      reason: "condition_failed",
    });
  });
});

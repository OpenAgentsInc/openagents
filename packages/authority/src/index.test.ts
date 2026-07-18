import { Effect } from "effect";
import { describe, expect, it } from "vite-plus/test";

import { AuthorityService, makeAuthorityServiceLayer } from "./index";

const profile = {
  profileRef: "authority.fixture",
  revision: 1,
  lifecycle: "admitted",
  authorityMayAmplify: false,
  explicitDenyWins: true,
  grants: [
    {
      grantRef: "grant.read",
      roles: ["orchestrator"],
      actions: ["read_context"],
      resources: ["owner_context"],
      programs: ["program.company"],
      conditionRefs: ["condition.owner_scope"],
    },
  ],
  reservedActions: ["increase_own_authority", "export_secret"],
};

const request = (action = "read_context") => ({
  requestRef: "request.fixture",
  actorRef: "actor.fixture",
  actorRole: "orchestrator",
  action,
  resource: "owner_context",
  programRef: "program.company",
  triggerRef: "trigger.fixture",
  conditionResults: [
    {
      conditionRef: "condition.owner_scope",
      passed: true,
      evidenceRefs: ["evidence.owner.fixture"],
    },
  ],
  startedAt: "2026-07-18T00:00:00.000Z",
});

describe("AuthorityService", () => {
  it("admits only the exact role, action, resource, program, and conditions", async () => {
    const decision = await Effect.runPromise(
      Effect.gen(function* () {
        const authority = yield* AuthorityService;
        return yield* authority.resolve(request());
      }).pipe(Effect.provide(makeAuthorityServiceLayer(profile))),
    );
    expect(decision._tag).toBe("Allowed");
  });

  it("denies self-amplification even when the caller possesses the profile", async () => {
    const decision = await Effect.runPromise(
      Effect.gen(function* () {
        const authority = yield* AuthorityService;
        return yield* authority.resolve(request("increase_own_authority"));
      }).pipe(Effect.provide(makeAuthorityServiceLayer(profile))),
    );
    expect(decision).toMatchObject({ _tag: "Denied", reason: "reserved_action" });
  });

  it("fails closed when a required condition is absent", async () => {
    const input = { ...request(), conditionResults: [] };
    const decision = await Effect.runPromise(
      Effect.gen(function* () {
        const authority = yield* AuthorityService;
        return yield* authority.resolve(input);
      }).pipe(Effect.provide(makeAuthorityServiceLayer(profile))),
    );
    expect(decision).toMatchObject({ _tag: "Denied", reason: "condition_missing" });
  });
});

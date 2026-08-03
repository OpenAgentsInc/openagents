import { Effect, Layer } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  decodeWorkSnapshot,
  emptyWorkCommandAuthorityState,
  inMemoryWorkCommandStateStoreLayer,
  WorkCommandAuthority,
  WorkCommandAuthorityLive,
} from "../src/index.ts";

const observedAt = "2026-08-03T10:00:00Z";
const ownerRef = "principal:omega:owner";
const agentPrincipalRef = "principal:omega:agent";
const organizationRef = "organization:openagents";
const capabilityRef = "capability:work-command:execute";

const snapshot = decodeWorkSnapshot({
  summary: {
    contractVersion: "openagents.all_work_boundary.v1",
    workRef: "work:omega:214",
    title: "Separate agent execution identities",
    domain: "development",
    workClass: "task",
    state: "active",
    priority: "urgent",
    ownerRef,
    assignee: null,
    agentDelegate: null,
    portfolio: { organizationRef },
    sourceAuthority: {
      kind: "imported_read_only",
      sourceRef: "github:OpenAgentsInc/omega#214",
      adapterVersion: "github-bootstrap-v1",
      writable: false,
    },
    revision: 7,
    updatedAt: observedAt,
    freshness: { state: "fresh", observedAt },
    completeness: { state: "complete", gapRefs: [] },
    redaction: {
      privacyClass: "private",
      redactedFieldCount: 0,
      policyRef: "policy:all-work-private-v1",
    },
  },
  issue: { workRef: "work:omega:214", identifier: "omega#214", state: "active", revision: 7 },
  relations: [],
  threadRefs: [],
  sessionRefs: [],
  agentSessionRefs: [],
  agentActivityRefs: [],
  runRefs: [],
  intentRefs: [],
  eventRefs: [],
  receiptRefs: [],
  evidenceRefs: [],
  verificationRefs: [],
  ownerDispositionRefs: [],
});

const request = (
  expectedRevision: number,
  id: string,
  command: Record<string, unknown>,
  effectivePrincipalRef = ownerRef,
) => ({
  intentRef: `intent:${id}`,
  idempotencyKey: `work-command-${id}`,
  expectedRevision,
  effectivePrincipalRef,
  organizationRef,
  capabilityRef,
  workRef: "work:omega:214",
  occurredAt: `2026-08-03T10:${String(expectedRevision).padStart(2, "0")}:00Z`,
  command,
});

const layer = () =>
  WorkCommandAuthorityLive.pipe(
    Layer.provide(
      inMemoryWorkCommandStateStoreLayer(
        emptyWorkCommandAuthorityState({
          snapshot,
          organizationRef,
          authorizedPrincipalRefs: [ownerRef, agentPrincipalRef],
        }),
      ),
    ),
  );

const grant = {
  grantRef: "delegation-grant:omega-214:1",
  agentRef: "agent:codex:local",
  issuedBy: ownerRef,
  generation: 1,
  capabilityRefs: ["capability:repository:mutate"],
  toolRefs: ["tool:codex"],
  hostRef: "host:owner-mac",
  budgetLimit: 100_000,
  expiresAt: "2026-08-04T10:00:00Z",
  privacyClass: "private",
  evidenceRequired: true,
  claimRef: "repository-claim:omega-214",
  leaseRef: "lease:omega-214",
};

describe("All Work command admission authority", () => {
  it("refuses delegation before a human assignee is accountable", async () => {
    const attempt = Effect.gen(function* () {
      const authority = yield* WorkCommandAuthority;
      return yield* authority
        .execute(request(7, "delegate-unassigned", { command: "delegate", grant }))
        .pipe(Effect.flip);
    }).pipe(Effect.provide(layer()));

    await expect(Effect.runPromise(attempt)).resolves.toMatchObject({
      reason: "invalid_delegation",
    });
  });

  it("keeps assignee and delegate separate and fences activity after revocation", async () => {
    const journey = Effect.gen(function* () {
      const authority = yield* WorkCommandAuthority;
      const assigned = yield* authority.execute(
        request(7, "assign", {
          command: "assign",
          assignee: { kind: "human", principalRef: ownerRef },
        }),
      );
      const delegated = yield* authority.execute(
        request(8, "delegate", { command: "delegate", grant }),
      );
      const unassignWhileDelegated = yield* authority
        .execute(request(9, "unassign-active-delegation", { command: "unassign" }))
        .pipe(Effect.flip);
      const started = yield* authority.execute(
        request(9, "start", {
          command: "start_agent_session",
          threadRef: "thread:omega-214",
          sessionRef: "session:omega-214",
          agentSessionRef: "agent-session:omega-214",
          runRef: "run:omega-214",
          grantRef: grant.grantRef,
          expectedGeneration: 1,
          hostRef: grant.hostRef,
        }),
      );
      const activity = yield* authority.execute(
        request(10, "activity", {
          command: "record_activity",
          activityRef: "agent-activity:omega-214:progress",
          sessionRef: "session:omega-214",
          runRef: "run:omega-214",
          expectedGeneration: 1,
          kind: "progress",
          summary: "Implemented the shared admission boundary.",
          providerEventRef: "provider-event:codex:1",
          lossRefs: ["loss:provider:omitted-internal-frame"],
          effectRef: "effect:omega-214:1",
        }),
      );
      const revoked = yield* authority.execute(
        request(11, "revoke", {
          command: "revoke",
          grantRef: grant.grantRef,
          expectedGeneration: 1,
          reason: "Owner stopped the delegation.",
        }),
      );
      const late = yield* authority
        .execute(
          request(12, "late", {
            command: "record_activity",
            activityRef: "agent-activity:omega-214:late",
            sessionRef: "session:omega-214",
            runRef: "run:omega-214",
            expectedGeneration: 1,
            kind: "result",
            summary: "Late provider result.",
            providerEventRef: "provider-event:codex:late",
            lossRefs: [],
            effectRef: "effect:omega-214:late",
          }),
        )
        .pipe(Effect.flip);
      const state = yield* authority.read;
      return {
        assigned,
        delegated,
        unassignWhileDelegated,
        started,
        activity,
        revoked,
        late,
        state,
      };
    }).pipe(Effect.provide(layer()));

    const result = await Effect.runPromise(journey);
    expect(result.assigned.snapshot.summary.assignee?.principalRef).toBe(ownerRef);
    expect(result.delegated.snapshot.summary).toMatchObject({
      assignee: { principalRef: ownerRef },
      agentDelegate: { agentRef: grant.agentRef, generation: 1 },
    });
    expect(result.unassignWhileDelegated).toMatchObject({ reason: "delegation_conflict" });
    expect(result.started.snapshot).toMatchObject({
      sessionRefs: ["session:omega-214"],
      runRefs: ["run:omega-214"],
    });
    expect(result.activity.receipt.outcome.effectRef).toBe("effect:omega-214:1");
    expect(result.revoked.snapshot.summary.agentDelegate).toBeNull();
    expect(result.late).toMatchObject({ reason: "stale_generation" });
    expect(result.state.sessions[0]?.state).toBe("revoked");
    expect(result.state.activities[0]).toMatchObject({
      providerEventRef: "provider-event:codex:1",
      lossRefs: ["loss:provider:omitted-internal-frame"],
    });
    expect(result.revoked.receipt.githubWriteCount).toBe(0);
  });

  it("replays exactly and rejects a changed idempotency replay", async () => {
    const firstRequest = request(7, "evidence", {
      command: "attach_evidence",
      evidenceRef: "evidence:omega-214:contract",
    });
    const journey = Effect.gen(function* () {
      const authority = yield* WorkCommandAuthority;
      const first = yield* authority.execute(firstRequest);
      const replay = yield* authority.execute(firstRequest);
      const conflict = yield* authority
        .execute({
          ...firstRequest,
          command: { command: "attach_evidence", evidenceRef: "evidence:omega-214:changed" },
        })
        .pipe(Effect.flip);
      return { first, replay, conflict };
    }).pipe(Effect.provide(layer()));

    const result = await Effect.runPromise(journey);
    expect(result.replay).toEqual(result.first);
    expect(result.conflict).toMatchObject({ reason: "idempotency_conflict" });
  });

  it("keeps verification separate and reserves disposition for the accountable human", async () => {
    const journey = Effect.gen(function* () {
      const authority = yield* WorkCommandAuthority;
      const verification = yield* authority.execute(
        request(7, "verification", {
          command: "attach_verification",
          verificationRef: "verification:omega-214:contract",
        }),
      );
      const agentAttempt = yield* authority
        .execute(
          request(
            8,
            "agent-disposition",
            {
              command: "owner_disposition",
              dispositionRef: "owner-disposition:omega-214:agent",
              decision: "accepted",
              verificationRefs: ["verification:omega-214:contract"],
              summary: "Agent self-acceptance must fail.",
            },
            agentPrincipalRef,
          ),
        )
        .pipe(Effect.flip);
      const disposition = yield* authority.execute(
        request(8, "owner-disposition", {
          command: "owner_disposition",
          dispositionRef: "owner-disposition:omega-214:owner",
          decision: "needs_changes",
          verificationRefs: ["verification:omega-214:contract"],
          summary: "The owner requests another iteration.",
        }),
      );
      const state = yield* authority.read;
      return { verification, agentAttempt, disposition, state };
    }).pipe(Effect.provide(layer()));

    const result = await Effect.runPromise(journey);
    expect(result.verification.snapshot.ownerDispositionRefs).toEqual([]);
    expect(result.agentAttempt).toMatchObject({ reason: "owner_disposition_forbidden" });
    expect(result.disposition.snapshot).toMatchObject({
      verificationRefs: ["verification:omega-214:contract"],
      ownerDispositionRefs: ["owner-disposition:omega-214:owner"],
    });
    expect(result.state.requests.at(-1)?.command).toMatchObject({
      command: "owner_disposition",
      decision: "needs_changes",
    });
  });
});

import {
  KHALA_CODE_TEAM_MEMBERSHIP_ENTITY_TYPE,
  SyncScope,
  SyncVersion,
  SyncVersionWatermark,
} from "@openagentsinc/khala-sync";
import { KhalaSyncClientStoreError, type ConfirmedEntity } from "@openagentsinc/khala-sync-client";
import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  DesktopThreadVisibilitySyncAuthorizationRequestSchemaLiteral,
  evaluateDesktopThreadVisibilityFromConfirmedSync,
} from "./thread-visibility-sync-authority.ts";

const WORKSPACE = "scope.team.team_1";
const THREAD = "thread.visibility.sync.1";

const receipt = (audience: unknown, administratorAccess: unknown = { kind: "none" }) => ({
  schema: "openagents.thread_disclosure_receipt.v1",
  receiptRef: "receipt.visibility.sync.1",
  intentRef: "intent.visibility.sync.1",
  idempotencyKey: "idempotency.visibility.sync.1",
  threadRef: THREAD,
  observedAt: "2026-07-17T19:30:00Z",
  kind: "thread.visibility.set",
  result: {
    status: "visibility_applied",
    visibilityVersion: 3,
    target: { audience, administratorAccess },
  },
});

const request = (audience: unknown, administratorAccess?: unknown) => ({
  schema: DesktopThreadVisibilitySyncAuthorizationRequestSchemaLiteral,
  actorRef: "user.actor",
  ownerRef: "user.owner",
  receipt: receipt(audience, administratorAccess),
});

const membershipEntity = (
  overrides: Partial<{
    entityId: string;
    teamId: string;
    userId: string;
    role: "owner" | "admin" | "member" | "viewer";
    status: "active" | "invited" | "removed";
  }> = {},
): ConfirmedEntity => {
  const entityId = overrides.entityId ?? "membership.team_1.user_actor";
  return {
    entityType: KHALA_CODE_TEAM_MEMBERSHIP_ENTITY_TYPE,
    entityId,
    version: SyncVersion.make(1),
    postImageJson: JSON.stringify({
      membershipId: entityId,
      teamId: overrides.teamId ?? "team_1",
      userId: overrides.userId ?? "user.actor",
      role: overrides.role ?? "member",
      status: overrides.status ?? "active",
      invitedByUserId: null,
      joinedAt: "2026-07-17T19:00:00Z",
      createdAt: "2026-07-17T19:00:00Z",
      updatedAt: "2026-07-17T19:00:00Z",
      removedAt: null,
    }),
  };
};

const harness = (
  input: Readonly<{
    entities?: ReadonlyArray<ConfirmedEntity>;
    phases?: ReadonlyArray<"live" | "must_refetch" | "denied" | "idle">;
    lastDeltaAt?: number | null;
    failRead?: boolean;
  }> = {},
) => {
  let reads = 0;
  let stateCalls = 0;
  const phases = input.phases ?? ["live"];
  return {
    dependencies: {
      session: {
        state: (_scope: SyncScope) => {
          const phase = phases[Math.min(stateCalls, phases.length - 1)]!;
          stateCalls += 1;
          return phase === "live"
            ? { phase, cursor: SyncVersionWatermark.make(1) }
            : phase === "must_refetch"
              ? { phase, reason: "access_changed" }
              : phase === "denied"
                ? { phase, reason: "access_denied" }
                : { phase };
        },
        lastDeltaAt: (_scope: SyncScope) =>
          input.lastDeltaAt === undefined ? 1 : input.lastDeltaAt,
      },
      store: {
        readEntities: (_scope: SyncScope, _entityType?: string) => {
          reads += 1;
          return input.failRead
            ? Effect.fail(new KhalaSyncClientStoreError("storage_failure", "private store detail"))
            : Effect.succeed(input.entities ?? []);
        },
      },
    },
    reads: () => reads,
  };
};

const run = (dependencies: ReturnType<typeof harness>["dependencies"], input: unknown) =>
  Effect.runPromise(evaluateDesktopThreadVisibilityFromConfirmedSync(dependencies, input));

describe("Desktop confirmed Sync visibility authority", () => {
  test("keeps owner and internet-readable authorization lookup-free", async () => {
    for (const [actorRef, audience, basis] of [
      ["user.owner", { kind: "owner_only" }, "owner"],
      ["user.actor", { kind: "internet_readable" }, "internet_readable"],
    ] as const) {
      const testHarness = harness({ failRead: true, phases: ["denied"] });
      await expect(
        run(testHarness.dependencies, { ...request(audience), actorRef }),
      ).resolves.toMatchObject({ status: "authorized", basis });
      expect(testHarness.reads()).toBe(0);
    }
  });

  test("authorizes live confirmed members and administrators", async () => {
    const member = harness({ entities: [membershipEntity()] });
    await expect(
      run(member.dependencies, request({ kind: "workspace_members", workspaceRef: WORKSPACE })),
    ).resolves.toMatchObject({ status: "authorized", basis: "workspace_member" });

    const admin = harness({ entities: [membershipEntity({ role: "admin" })] });
    await expect(
      run(
        admin.dependencies,
        request(
          { kind: "named_group", workspaceRef: WORKSPACE, groupRef: "group.reviewers" },
          { kind: "workspace_admins", workspaceRef: WORKSPACE },
        ),
      ),
    ).resolves.toMatchObject({ status: "authorized", basis: "workspace_administrator" });
  });

  test("treats active viewers as members but never infers named-group membership", async () => {
    const viewer = harness({ entities: [membershipEntity({ role: "viewer" })] });
    await expect(
      run(viewer.dependencies, request({ kind: "workspace_members", workspaceRef: WORKSPACE })),
    ).resolves.toMatchObject({ status: "authorized", basis: "workspace_member" });
    await expect(
      run(
        viewer.dependencies,
        request({ kind: "named_group", workspaceRef: WORKSPACE, groupRef: "group.reviewers" }),
      ),
    ).resolves.toMatchObject({ status: "denied", reason: "no_matching_authority" });
  });

  test("denies confirmed absence, another actor, and inactive membership", async () => {
    for (const entities of [
      [],
      [membershipEntity({ userId: "user.other" })],
      [membershipEntity({ status: "removed" })],
    ]) {
      await expect(
        run(
          harness({ entities }).dependencies,
          request({ kind: "workspace_members", workspaceRef: WORKSPACE }),
        ),
      ).resolves.toMatchObject({ status: "denied", reason: "no_matching_authority" });
    }
  });

  test("fails closed when Sync is not live and confirmed", async () => {
    for (const options of [
      { phases: ["idle"] as const },
      { phases: ["must_refetch"] as const },
      { phases: ["denied"] as const },
      { phases: ["live"] as const, lastDeltaAt: null },
      { phases: ["live"] as const, failRead: true },
      { phases: ["live", "must_refetch"] as const, entities: [membershipEntity()] },
    ]) {
      await expect(
        run(
          harness(options).dependencies,
          request({ kind: "workspace_members", workspaceRef: WORKSPACE }),
        ),
      ).resolves.toEqual({ status: "rejected", reason: "authority_unavailable" });
    }
  });

  test("fails closed on malformed, cross-team, or ambiguous confirmed entities", async () => {
    const malformed: ConfirmedEntity = { ...membershipEntity(), postImageJson: "{" };
    for (const entities of [
      [malformed],
      [membershipEntity({ teamId: "team_other" })],
      [membershipEntity(), membershipEntity({ entityId: "membership.duplicate" })],
    ]) {
      await expect(
        run(
          harness({ entities }).dependencies,
          request({ kind: "workspace_members", workspaceRef: WORKSPACE }),
        ),
      ).resolves.toEqual({ status: "rejected", reason: "authority_unavailable" });
    }
  });

  test("rejects malformed requests before lookup and unsupported workspace refs fail closed", async () => {
    const testHarness = harness({ failRead: true });
    await expect(
      run(testHarness.dependencies, { ...request({ kind: "owner_only" }), raw: "x" }),
    ).resolves.toEqual({ status: "rejected", reason: "invalid_request" });
    expect(testHarness.reads()).toBe(0);

    await expect(
      run(
        testHarness.dependencies,
        request({ kind: "workspace_members", workspaceRef: "workspace.unbound" }),
      ),
    ).resolves.toEqual({ status: "rejected", reason: "authority_unavailable" });
    expect(testHarness.reads()).toBe(0);
  });
});

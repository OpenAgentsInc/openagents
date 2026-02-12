import { Effect, Option } from "effect";
import { describe, expect, it } from "vitest";

import {
  createTaskImpl,
  getTaskImpl,
  listTaskEventsImpl,
  listTasksImpl,
  transitionTaskImpl,
} from "../../convex/lightning/tasks";
import { setGlobalPauseImpl, setOwnerKillSwitchImpl } from "../../convex/lightning/security";
import { makeInMemoryDb } from "./inMemoryDb";

const run = <A>(effect: Effect.Effect<A>) => Effect.runPromise(effect);

const authedCtx = (db: any, subject = "user-1") => ({
  db,
  auth: {
    getUserIdentity: () => Effect.succeed(Option.some({ subject })),
  },
});

describe("convex/lightning task orchestration", () => {
  it("creates L402 task records with idempotency and owner scoping", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "user-1");

    const first = await run(
      createTaskImpl(ctx, {
        request: {
          url: "https://api.example.com/premium",
          method: "GET",
          maxSpendMsats: 25_000,
        },
        idempotencyKey: "same-request",
        source: "tests",
      }),
    );

    expect(first.ok).toBe(true);
    expect(first.existed).toBe(false);
    expect(first.task.ownerId).toBe("user-1");
    expect(first.task.status).toBe("queued");

    const second = await run(
      createTaskImpl(ctx, {
        request: {
          url: "https://api.example.com/premium",
          method: "GET",
          maxSpendMsats: 25_000,
        },
        idempotencyKey: "same-request",
        source: "tests",
      }),
    );

    expect(second.ok).toBe(true);
    expect(second.existed).toBe(true);
    expect(second.task.taskId).toBe(first.task.taskId);
    expect(db.__tables.lightningTasks).toHaveLength(1);

    const fetched = await run(getTaskImpl(ctx, { taskId: first.task.taskId }));
    expect(fetched.task.taskId).toBe(first.task.taskId);

    const listed = await run(listTasksImpl(ctx, { limit: 10 }));
    expect(listed.tasks).toHaveLength(1);
    expect(listed.tasks[0]?.taskId).toBe(first.task.taskId);
  });

  it("enforces same-user access for get and transition", async () => {
    const db = makeInMemoryDb();
    const ownerCtx = authedCtx(db, "owner-1");
    const otherCtx = authedCtx(db, "owner-2");

    const created = await run(
      createTaskImpl(ownerCtx, {
        request: {
          url: "https://api.example.com/private",
          method: "POST",
          body: "{\"foo\":\"bar\"}",
          maxSpendMsats: 1_000,
        },
      }),
    );

    await expect(run(getTaskImpl(otherCtx, { taskId: created.task.taskId }))).rejects.toThrow(/forbidden/);
    await expect(
      run(
        transitionTaskImpl(otherCtx, {
          taskId: created.task.taskId,
          toStatus: "running",
          actor: "desktop_executor",
        }),
      ),
    ).rejects.toThrow(/forbidden/);
  });

  it("validates transition graph and records auditable events", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "user-1");

    const created = await run(
      createTaskImpl(ctx, {
        request: {
          url: "https://api.example.com/resource",
          method: "GET",
          maxSpendMsats: 7_000,
        },
      }),
    );
    const taskId = created.task.taskId;

    const running = await run(
      transitionTaskImpl(ctx, {
        taskId,
        toStatus: "running",
        actor: "desktop_executor",
      }),
    );
    expect(running.changed).toBe(true);
    expect(running.task.status).toBe("running");
    expect(running.task.attemptCount).toBe(1);

    const failed = await run(
      transitionTaskImpl(ctx, {
        taskId,
        toStatus: "failed",
        actor: "desktop_executor",
        errorCode: "payment_failed",
        errorMessage: "invoice expired",
      }),
    );
    expect(failed.task.status).toBe("failed");
    expect(failed.task.lastErrorCode).toBe("payment_failed");
    expect(failed.task.lastErrorMessage).toContain("invoice");

    const retried = await run(
      transitionTaskImpl(ctx, {
        taskId,
        toStatus: "queued",
        actor: "system",
        reason: "retry",
      }),
    );
    expect(retried.task.status).toBe("queued");

    await expect(
      run(
        transitionTaskImpl(ctx, {
          taskId,
          toStatus: "paid",
          actor: "desktop_executor",
        }),
      ),
    ).rejects.toThrow(/invalid_transition/);

    const events = await run(listTaskEventsImpl(ctx, { taskId, limit: 20 }));
    expect(events.events.map((event) => event.toStatus)).toEqual(["queued", "running", "failed", "queued"]);
    expect(events.events[2]?.errorCode).toBe("payment_failed");
  });

  it("blocks new paid requests when global pause or owner kill switch is active", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "owner-1");
    const opsCtx = {
      db,
      auth: {
        getUserIdentity: () => Effect.succeed(Option.none()),
      },
    };
    const previousSecret = process.env.OA_LIGHTNING_OPS_SECRET;
    process.env.OA_LIGHTNING_OPS_SECRET = "ops-secret";

    try {
      await run(
        setGlobalPauseImpl(opsCtx as any, {
          secret: "ops-secret",
          active: true,
          reason: "global pause test",
        }),
      );

      const blockedByGlobal = await run(
        createTaskImpl(ctx, {
          request: {
            url: "https://api.example.com/global-block",
            method: "GET",
            maxSpendMsats: 1_500,
          },
        }),
      );

      expect(blockedByGlobal.task.status).toBe("blocked");
      expect(blockedByGlobal.task.lastErrorCode).toBe("global_pause_active");
      expect(blockedByGlobal.task.lastErrorMessage).toContain("global");

      await run(
        setGlobalPauseImpl(opsCtx as any, {
          secret: "ops-secret",
          active: false,
        }),
      );
      await run(
        setOwnerKillSwitchImpl(opsCtx as any, {
          secret: "ops-secret",
          ownerId: "owner-1",
          active: true,
          reason: "owner pause test",
        }),
      );

      const blockedByOwner = await run(
        createTaskImpl(ctx, {
          request: {
            url: "https://api.example.com/owner-block",
            method: "GET",
            maxSpendMsats: 1_500,
          },
        }),
      );

      expect(blockedByOwner.task.status).toBe("blocked");
      expect(blockedByOwner.task.lastErrorCode).toBe("owner_kill_switch_active");

      await run(
        setOwnerKillSwitchImpl(opsCtx as any, {
          secret: "ops-secret",
          ownerId: "owner-1",
          active: false,
        }),
      );

      const recovered = await run(
        createTaskImpl(ctx, {
          request: {
            url: "https://api.example.com/recovered",
            method: "GET",
            maxSpendMsats: 1_500,
          },
        }),
      );
      expect(recovered.task.status).toBe("queued");
    } finally {
      process.env.OA_LIGHTNING_OPS_SECRET = previousSecret;
    }
  });
});

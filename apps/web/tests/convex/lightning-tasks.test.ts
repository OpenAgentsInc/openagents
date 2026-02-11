import { Effect, Option } from "effect";
import { describe, expect, it } from "vitest";

import {
  createTaskImpl,
  getTaskImpl,
  listTaskEventsImpl,
  listTasksImpl,
  transitionTaskImpl,
} from "../../convex/lightning/tasks";
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
});

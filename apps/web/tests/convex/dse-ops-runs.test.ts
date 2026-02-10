import { describe, expect, it } from "vitest";
import { Effect, Option } from "effect";

import { makeInMemoryDb } from "./inMemoryDb";

import { appendEventImpl, finishRunImpl, startRunImpl } from "../../convex/dse/opsRuns";

const run = <A>(effect: Effect.Effect<A>) => Effect.runPromise(effect);

const authedCtx = (db: any, subject = "user-1") => ({
  db,
  auth: {
    getUserIdentity: () => Effect.succeed(Option.some({ subject })),
  },
});

describe("convex/dse ops runs (admin-only)", () => {
  it("records start -> event -> finish with bounded events", async () => {
    const db = makeInMemoryDb();
    const admin = authedCtx(db, "user_dse_admin");

    await run(startRunImpl(admin, { runId: "opsrun_test_1", commitSha: "abc123", baseUrl: "https://example.com" }));
    expect(db.__tables.dseOpsRuns).toHaveLength(1);
    expect(db.__tables.dseOpsRuns[0]?.status).toBe("running");
    expect(db.__tables.dseOpsRunEvents).toHaveLength(1);

    await run(
      appendEventImpl(admin, {
        runId: "opsrun_test_1",
        level: "info",
        phase: "phase1.test",
        message: "hello",
        json: { a: 1 },
      }),
    );
    expect(db.__tables.dseOpsRunEvents).toHaveLength(2);

    await run(
      finishRunImpl(admin, {
        runId: "opsrun_test_1",
        status: "finished",
        summaryJson: { ok: true, note: "done" },
      }),
    );
    expect(db.__tables.dseOpsRuns[0]?.status).toBe("finished");
    expect(typeof db.__tables.dseOpsRuns[0]?.endedAtMs).toBe("number");
    expect(db.__tables.dseOpsRunEvents).toHaveLength(3);
  });

  it("is idempotent by runId (second startRun is a no-op)", async () => {
    const db = makeInMemoryDb();
    const admin = authedCtx(db, "user_dse_admin");

    const first = await run(startRunImpl(admin, { runId: "opsrun_test_2" }));
    expect(first.existed).toBe(false);
    expect(db.__tables.dseOpsRuns).toHaveLength(1);
    expect(db.__tables.dseOpsRunEvents).toHaveLength(1);

    const second = await run(startRunImpl(admin, { runId: "opsrun_test_2" }));
    expect(second.existed).toBe(true);
    expect(db.__tables.dseOpsRuns).toHaveLength(1);
    expect(db.__tables.dseOpsRunEvents).toHaveLength(1);
  });

  it("rejects non-admin subjects", async () => {
    const db = makeInMemoryDb();
    const user = authedCtx(db, "user-1");

    await expect(run(startRunImpl(user, { runId: "opsrun_nope" }))).rejects.toThrow(/unauthorized/);
  });
});


import { describe, expect, it } from "vitest";
import { Effect, Option } from "effect";

import { makeInMemoryDb } from "./inMemoryDb";

import { getReportImpl, listReportsImpl, putReportImpl } from "../../convex/dse/evalReports";

const run = <A>(effect: Effect.Effect<A>) => Effect.runPromise(effect);

const anonCtx = (db: any) => ({
  db,
  auth: {
    getUserIdentity: () => Effect.succeed(Option.none()),
  },
});

const authedCtx = (db: any, subject = "user-1") => ({
  db,
  auth: {
    getUserIdentity: () => Effect.succeed(Option.some({ subject })),
  },
});

describe("convex/dse Phase 7 eval reports store", () => {
  it("stores eval reports idempotently by (signatureId, evalHash)", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "admin-1");

    const signatureId = "@openagents/test/Sig.v1";
    const evalHash = "sha256:eval-1";

    const first = await run(
      putReportImpl(ctx, {
        signatureId,
        evalHash,
        compiled_id: "c1",
        datasetId: "dataset-1",
        datasetHash: "sha256:ds-1",
        rewardId: "reward-1",
        rewardVersion: 1,
        split: "holdout",
        selectedExampleIdsHash: "sha256:sel-1",
        n: 10,
        json: { ok: true, summary: { reward: 0.5 } },
      }),
    );
    expect(first.existed).toBe(false);

    const second = await run(
      putReportImpl(ctx, {
        signatureId,
        evalHash,
        compiled_id: "c1",
        datasetId: "dataset-1",
        datasetHash: "sha256:ds-1",
        rewardId: "reward-1",
        rewardVersion: 1,
        split: "holdout",
        selectedExampleIdsHash: "sha256:sel-1",
        n: 10,
        json: { ok: true, summary: { reward: 0.5 } },
      }),
    );
    expect(second.existed).toBe(true);

    expect(db.__tables.dseEvalReports).toHaveLength(1);

    const got = await run(getReportImpl(ctx, { signatureId, evalHash }));
    expect(got.ok).toBe(true);
    expect(got.report?.compiled_id).toBe("c1");
    expect(got.report?.rewardId).toBe("reward-1");
    expect(got.report?.split).toBe("holdout");

    const listed = await run(listReportsImpl(ctx, { signatureId }));
    expect(listed.ok).toBe(true);
    expect(listed.reports).toHaveLength(1);
    expect(listed.reports[0]?.evalHash).toBe(evalHash);
  });

  it("requires auth (MVP safety default)", async () => {
    const db = makeInMemoryDb();
    const ctx = anonCtx(db);

    await expect(run(listReportsImpl(ctx as any, { signatureId: "@openagents/test/Sig.v1" }))).rejects.toThrow(/unauthorized/);
  });
});


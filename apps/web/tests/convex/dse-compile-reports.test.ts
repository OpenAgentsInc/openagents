import { describe, expect, it } from "vitest";
import { Effect, Option } from "effect";

import { makeInMemoryDb } from "./inMemoryDb";

import { getReportImpl, listReportsImpl, putReportImpl } from "../../convex/dse/compileReports";

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

describe("convex/dse Stage 5 compile reports store", () => {
  it("stores compile reports idempotently by (signatureId, jobHash, datasetHash)", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "admin-1");

    const signatureId = "@openagents/test/Sig.v1";
    const jobHash = "job-1";
    const datasetHash = "data-1";

    const first = await run(
      putReportImpl(ctx, {
        signatureId,
        jobHash,
        datasetId: "dataset-1",
        datasetHash,
        compiled_id: "c1",
        json: { hello: "world" },
      }),
    );
    expect(first.existed).toBe(false);

    const second = await run(
      putReportImpl(ctx, {
        signatureId,
        jobHash,
        datasetId: "dataset-1",
        datasetHash,
        compiled_id: "c1",
        json: { hello: "world" },
      }),
    );
    expect(second.existed).toBe(true);

    expect(db.__tables.dseCompileReports).toHaveLength(1);

    const got = await run(getReportImpl(ctx, { signatureId, jobHash, datasetHash }));
    expect(got.ok).toBe(true);
    expect(got.report?.compiled_id).toBe("c1");
    expect(got.report?.datasetId).toBe("dataset-1");
    expect(got.report?.json?.hello).toBe("world");

    const listed = await run(listReportsImpl(ctx, { signatureId }));
    expect(listed.ok).toBe(true);
    expect(listed.reports).toHaveLength(1);
    expect(listed.reports[0]?.jobHash).toBe(jobHash);
  });

  it("requires auth (MVP safety default)", async () => {
    const db = makeInMemoryDb();
    const ctx = anonCtx(db);
    await expect(
      run(
        listReportsImpl(ctx as any, { signatureId: "@openagents/test/Sig.v1" }),
      ),
    ).rejects.toThrow(/unauthorized/);
  });
});


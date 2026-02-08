import { describe, expect, it } from "vitest";
import { Effect, Option } from "effect";

import { makeInMemoryDb } from "./inMemoryDb";

import { getExampleImpl, listExamplesImpl, putExampleImpl } from "../../convex/dse/examples";

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

describe("convex/dse Stage 4 dataset (labeled examples)", () => {
  it("lists examples deterministically sorted by exampleId (independent of insertion order)", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "admin-1");

    const signatureId = "@openagents/test/Sig.v1";

    await run(
      putExampleImpl(ctx, {
        signatureId,
        exampleId: "b",
        inputJson: { message: "b" },
        expectedJson: { action: "none" },
        split: "train",
        tags: ["seed"],
        source: "seed",
      }),
    );
    await run(
      putExampleImpl(ctx, {
        signatureId,
        exampleId: "a",
        inputJson: { message: "a" },
        expectedJson: { action: "tool", toolName: "identity_update" },
        split: "train",
        source: "seed",
      }),
    );
    await run(
      putExampleImpl(ctx, {
        signatureId,
        exampleId: "c",
        inputJson: { message: "c" },
        expectedJson: { action: "none" },
        split: "test",
        source: "seed",
      }),
    );

    const listed = await run(listExamplesImpl(ctx, { signatureId }));
    expect(listed.ok).toBe(true);
    expect(listed.examples.map((e) => e.exampleId)).toEqual(["a", "b", "c"]);
  });

  it("filters by split and supports upsert without changing createdAtMs", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "admin-1");

    const signatureId = "@openagents/test/Sig.v1";

    const first = await run(
      putExampleImpl(ctx, {
        signatureId,
        exampleId: "ex1",
        inputJson: { message: "hi" },
        expectedJson: { action: "none" },
        split: "train",
        source: "seed",
      }),
    );
    expect(first.existed).toBe(false);

    const afterFirst = await run(getExampleImpl(ctx, { signatureId, exampleId: "ex1" }));
    expect(afterFirst.example).toBeTruthy();
    const createdAtMs = Number(afterFirst.example?.createdAtMs ?? 0);
    expect(createdAtMs).toBeGreaterThan(0);

    const second = await run(
      putExampleImpl(ctx, {
        signatureId,
        exampleId: "ex1",
        inputJson: { message: "hi" },
        expectedJson: { action: "tool", toolName: "user_update" },
        split: "train",
        source: "seed",
      }),
    );
    expect(second.existed).toBe(true);

    const got = await run(getExampleImpl(ctx, { signatureId, exampleId: "ex1" }));
    expect(got.example?.createdAtMs).toBe(createdAtMs);
    expect(Number(got.example?.updatedAtMs ?? 0)).toBeGreaterThanOrEqual(createdAtMs);
    expect(got.example?.expectedJson?.toolName).toBe("user_update");

    await run(
      putExampleImpl(ctx, {
        signatureId,
        exampleId: "ex2",
        inputJson: { message: "yo" },
        expectedJson: { action: "none" },
        split: "test",
        source: "seed",
      }),
    );

    const trainOnly = await run(listExamplesImpl(ctx, { signatureId, split: "train" }));
    expect(trainOnly.examples.map((e) => e.exampleId)).toEqual(["ex1"]);
  });

  it("requires auth (MVP safety default)", async () => {
    const db = makeInMemoryDb();
    const ctx = anonCtx(db);

    await expect(run(listExamplesImpl(ctx as any, { signatureId: "@openagents/test/Sig.v1" }))).rejects.toThrow(
      /unauthorized/,
    );
  });
});

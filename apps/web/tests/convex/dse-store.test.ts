import { describe, expect, it } from "vitest";
import { Effect, Option } from "effect";

import { makeInMemoryDb } from "./inMemoryDb";

import { putArtifactImpl } from "../../convex/dse/artifacts";
import { clearActiveImpl, getActiveImpl, rollbackActiveImpl, setActiveImpl } from "../../convex/dse/active";
import { recordPredictReceiptImpl } from "../../convex/dse/receipts";

import { ensureOwnedThreadImpl } from "../../convex/autopilot/threads";
import { createRunImpl } from "../../convex/autopilot/messages";

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

describe("convex/dse Stage 2 stores (artifacts + active pointer + receipts)", () => {
  it("supports store -> promote -> rollback (active pointer history)", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "admin-1");

    const signatureId = "@openagents/test/Sig.v1";

    await run(putArtifactImpl(ctx, { signatureId, compiled_id: "c1", json: { hello: "c1" } }));
    await run(putArtifactImpl(ctx, { signatureId, compiled_id: "c2", json: { hello: "c2" } }));

    await run(setActiveImpl(ctx, { signatureId, compiled_id: "c1", reason: "promote c1" }));
    expect(db.__tables.dseActiveArtifacts).toHaveLength(1);

    const first = await run(getActiveImpl(ctx, { signatureId }));
    expect(first.compiled_id).toBe("c1");

    await run(setActiveImpl(ctx, { signatureId, compiled_id: "c2", reason: "promote c2" }));
    const second = await run(getActiveImpl(ctx, { signatureId }));
    expect(second.compiled_id).toBe("c2");

    const rolled = await run(rollbackActiveImpl(ctx, { signatureId, reason: "rollback" }));
    expect(rolled.ok).toBe(true);
    expect(rolled.compiled_id).toBe("c1");

    const afterRollback = await run(getActiveImpl(ctx, { signatureId }));
    expect(afterRollback.compiled_id).toBe("c1");

    await run(clearActiveImpl(ctx, { signatureId, reason: "clear" }));
    const afterClear = await run(getActiveImpl(ctx, { signatureId }));
    expect(afterClear.compiled_id).toBe(null);

    // History should be append-only and attributable (actor optional).
    expect(db.__tables.dseActiveArtifactHistory.length).toBeGreaterThanOrEqual(4);
    const actions = db.__tables.dseActiveArtifactHistory.map((h) => h.action);
    expect(actions).toEqual(["set", "set", "rollback", "clear"]);
    expect(db.__tables.dseActiveArtifactHistory[0]?.actorUserId).toBe("admin-1");
  });

  it("refuses to setActive when the artifact is missing", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "admin-1");

    await expect(run(setActiveImpl(ctx, { signatureId: "@openagents/test/Missing.v1", compiled_id: "nope" }))).rejects.toThrow(
      /artifact_not_found/,
    );
  });

  it("records DSE predict receipts into the thread receipts stream (owner-only)", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "user-1");
    const ensured = await run(ensureOwnedThreadImpl(ctx));
    const threadId = ensured.threadId;

    const created = await run(createRunImpl(ctx, { threadId, text: "hi" }));

    await run(
      recordPredictReceiptImpl(ctx, {
        threadId,
        runId: created.runId,
        receipt: {
          format: "openagents.dse.predict_receipt",
          formatVersion: 1,
          receiptId: "rcpt-1",
          createdAt: "2026-02-08T00:00:00Z",
          signatureId: "@openagents/test/Sig.v1",
          compiled_id: "c1",
          hashes: {
            inputSchemaHash: "h1",
            outputSchemaHash: "h2",
            promptIrHash: "h3",
            paramsHash: "h4",
          },
          model: {},
          timing: { startedAtMs: 1, endedAtMs: 2, durationMs: 1 },
          result: { _tag: "Ok" },
        },
      }),
    );

    expect(db.__tables.receipts).toHaveLength(1);
    expect(db.__tables.receipts[0]?.kind).toBe("dse.predict");
    expect(db.__tables.receipts[0]?.receiptId).toBe("rcpt-1");
    expect(db.__tables.receipts[0]?.signatureId).toBe("@openagents/test/Sig.v1");
    expect(db.__tables.receipts[0]?.compiled_id).toBe("c1");
  });
});

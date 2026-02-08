import { describe, expect, it } from "vitest";
import { Effect, Option } from "effect";

import { makeInMemoryDb } from "./inMemoryDb";

import { putArtifactImpl } from "../../convex/dse/artifacts";
import { setActiveImpl } from "../../convex/dse/active";
import { getCanaryImpl, startCanaryImpl, stopCanaryImpl } from "../../convex/dse/canary";
import { recordPredictReceiptImpl } from "../../convex/dse/receipts";

import { ensureAnonThreadImpl } from "../../convex/autopilot/threads";
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

describe("convex/dse Stage 6 canary config + auto-stop", () => {
  it("starts and stops a canary (auth required; stores control + canary compiled ids)", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "admin-1");

    const signatureId = "@openagents/test/Sig.v1";

    await run(putArtifactImpl(ctx, { signatureId, compiled_id: "control", json: { ok: true } }));
    await run(putArtifactImpl(ctx, { signatureId, compiled_id: "canary", json: { ok: true } }));
    await run(setActiveImpl(ctx, { signatureId, compiled_id: "control", reason: "baseline" }));

    const started = await run(
      startCanaryImpl(ctx, {
        signatureId,
        canary_compiled_id: "canary",
        rolloutPct: 25,
        minSamples: 2,
        maxErrorRate: 0.5,
        reason: "test",
      }),
    );
    expect(started.ok).toBe(true);
    expect(started.control_compiled_id).toBe("control");
    expect(started.canary_compiled_id).toBe("canary");
    expect(started.rolloutPct).toBe(25);

    const got = await run(getCanaryImpl(ctx as any, { signatureId }));
    expect(got.canary?.enabled).toBe(true);
    expect(got.canary?.control_compiled_id).toBe("control");
    expect(got.canary?.canary_compiled_id).toBe("canary");

    const stopped = await run(stopCanaryImpl(ctx, { signatureId, reason: "done" }));
    expect(stopped.ok).toBe(true);
    expect(stopped.existed).toBe(true);

    const after = await run(getCanaryImpl(ctx as any, { signatureId }));
    expect(after.canary).toBe(null);
  });

  it("auto-stops a canary when error rate exceeds threshold (observed via dse.predict receipts)", async () => {
    const db = makeInMemoryDb();
    const admin = authedCtx(db, "admin-1");
    const anon = anonCtx(db);

    const signatureId = "@openagents/test/Sig.v1";
    await run(putArtifactImpl(admin, { signatureId, compiled_id: "control", json: { ok: true } }));
    await run(putArtifactImpl(admin, { signatureId, compiled_id: "canary", json: { ok: true } }));
    await run(setActiveImpl(admin, { signatureId, compiled_id: "control", reason: "baseline" }));

    await run(
      startCanaryImpl(admin, {
        signatureId,
        canary_compiled_id: "canary",
        rolloutPct: 100,
        minSamples: 2,
        maxErrorRate: 0.4,
        reason: "test",
      }),
    );

    const threadId = "thread-1";
    const anonKey = "anon-1";
    await run(ensureAnonThreadImpl(anon, { threadId, anonKey }));
    const created = await run(createRunImpl(anon, { threadId, anonKey, text: "hi" }));

    const mkReceipt = (id: string) => ({
      format: "openagents.dse.predict_receipt",
      formatVersion: 1,
      receiptId: id,
      createdAt: "2026-02-08T00:00:00Z",
      signatureId,
      compiled_id: "canary",
      hashes: {
        inputSchemaHash: "h1",
        outputSchemaHash: "h2",
        promptIrHash: "h3",
        paramsHash: "h4",
      },
      model: {},
      timing: { startedAtMs: 1, endedAtMs: 2, durationMs: 1 },
      result: { _tag: "Error", errorName: "Oops", message: "bad" },
    });

    await run(recordPredictReceiptImpl(anon, { threadId, anonKey, runId: created.runId, receipt: mkReceipt("r1") }));
    await run(recordPredictReceiptImpl(anon, { threadId, anonKey, runId: created.runId, receipt: mkReceipt("r2") }));

    // Canary config should be removed.
    expect(db.__tables.dseCanaries).toHaveLength(0);
    expect(db.__tables.dseCanaryHistory.length).toBeGreaterThan(0);
    const last = db.__tables.dseCanaryHistory.at(-1) as any;
    expect(last.action).toBe("auto_stop");
    expect(String(last.reason ?? "")).toContain("error_rate_exceeded");
  });
});


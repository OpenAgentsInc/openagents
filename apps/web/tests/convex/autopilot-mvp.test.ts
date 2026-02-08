import { describe, expect, it } from "vitest";
import { Effect, Option } from "effect";

import { makeInMemoryDb } from "./inMemoryDb";

import { FIRST_OPEN_WELCOME_MESSAGE } from "../../convex/autopilot/defaults";
import { getBlueprintImpl, resetBlueprintImpl, setBlueprintImpl } from "../../convex/autopilot/blueprint";
import {
  appendPartsImpl,
  createRunImpl,
  finalizeRunImpl,
  getThreadSnapshotImpl,
  isCancelRequestedImpl,
  requestCancelImpl,
} from "../../convex/autopilot/messages";
import { resetThreadImpl } from "../../convex/autopilot/reset";
import { claimAnonThreadImpl, ensureAnonThreadImpl, ensureOwnedThreadImpl } from "../../convex/autopilot/threads";

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

describe("convex/autopilot MVP impls", () => {
  it("ensureAnonThread seeds thread + blueprint + welcome and is idempotent", async () => {
    const db = makeInMemoryDb();
    const ctx = anonCtx(db);

    const threadId = "thread-1";
    const anonKey = "anon-key-1";

    await run(ensureAnonThreadImpl(ctx, { threadId, anonKey }));

    expect(db.__tables.threads).toHaveLength(1);
    expect(db.__tables.threads[0]).toMatchObject({ threadId, anonKey });

    expect(db.__tables.blueprints).toHaveLength(1);
    expect(db.__tables.blueprints[0]).toMatchObject({ threadId });

    expect(db.__tables.messages).toHaveLength(1);
    expect(db.__tables.messages[0]).toMatchObject({
      threadId,
      role: "assistant",
      status: "final",
      text: FIRST_OPEN_WELCOME_MESSAGE,
    });

    // Idempotent.
    await run(ensureAnonThreadImpl(ctx, { threadId, anonKey }));
    expect(db.__tables.threads).toHaveLength(1);
    expect(db.__tables.blueprints).toHaveLength(1);
    expect(db.__tables.messages).toHaveLength(1);

    // Anon key is not a bearer token; mismatches are forbidden.
    await expect(run(ensureAnonThreadImpl(ctx, { threadId, anonKey: "other" }))).rejects.toThrow(/forbidden/);
  });

  it("ensureOwnedThread creates a per-user default thread and forbids mismatched ownership", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "user-1");

    const first = await run(ensureOwnedThreadImpl(ctx));
    expect(first.ok).toBe(true);
    expect(typeof first.threadId).toBe("string");

    expect(db.__tables.users).toHaveLength(1);
    expect(db.__tables.users[0]).toMatchObject({ userId: "user-1", defaultThreadId: first.threadId });

    expect(db.__tables.threads).toHaveLength(1);
    expect(db.__tables.threads[0]).toMatchObject({ threadId: first.threadId, ownerId: "user-1" });

    expect(db.__tables.blueprints).toHaveLength(1);
    expect(db.__tables.messages).toHaveLength(1);

    // Idempotent: returns the same thread id and does not duplicate seed rows.
    const second = await run(ensureOwnedThreadImpl(ctx));
    expect(second.threadId).toBe(first.threadId);
    expect(db.__tables.threads).toHaveLength(1);
    expect(db.__tables.blueprints).toHaveLength(1);
    expect(db.__tables.messages).toHaveLength(1);

    // Forbidden if stored defaultThreadId points at a thread owned by a different user.
    const db2 = makeInMemoryDb();
    await db2.insert("users", { userId: "user-1", createdAtMs: 1, defaultThreadId: "thread-x" });
    await db2.insert("threads", { threadId: "thread-x", ownerId: "user-2", createdAtMs: 1, updatedAtMs: 1 });
    await expect(run(ensureOwnedThreadImpl(authedCtx(db2, "user-1")))).rejects.toThrow(/forbidden/);
  });

  it("claimAnonThread transfers anon thread ownership and persists defaultThreadId", async () => {
    const db = makeInMemoryDb();
    const threadId = "thread-claim-1";
    const anonKey = "anon-claim-1";

    await run(ensureAnonThreadImpl(anonCtx(db), { threadId, anonKey }));

    const claimed = await run(claimAnonThreadImpl(authedCtx(db, "user-1"), { threadId, anonKey }));
    expect(claimed).toEqual({ ok: true, threadId });

    expect(db.__tables.threads).toHaveLength(1);
    expect(db.__tables.threads[0].ownerId).toBe("user-1");
    expect(db.__tables.threads[0].anonKey).toBeUndefined();

    expect(db.__tables.users).toHaveLength(1);
    expect(db.__tables.users[0]).toMatchObject({ userId: "user-1", defaultThreadId: threadId });

    // Other users cannot claim an already-owned thread.
    await expect(run(claimAnonThreadImpl(authedCtx(db, "user-2"), { threadId, anonKey }))).rejects.toThrow(/forbidden/);
  });

  it("createRun + appendParts + finalizeRun persists canonical chat state (idempotent parts)", async () => {
    const db = makeInMemoryDb();
    const threadId = "thread-run-1";
    const anonKey = "anon-run-1";
    const ctx = anonCtx(db);

    await run(ensureAnonThreadImpl(ctx, { threadId, anonKey }));

    const created = await run(createRunImpl(ctx, { threadId, anonKey, text: "hello" }));
    expect(created.ok).toBe(true);
    expect(typeof created.runId).toBe("string");
    expect(typeof created.userMessageId).toBe("string");
    expect(typeof created.assistantMessageId).toBe("string");

    const appended = await run(
      appendPartsImpl(ctx, {
        threadId,
        anonKey,
        runId: created.runId,
        messageId: created.assistantMessageId,
        parts: [
          { seq: 0, part: { type: "text-start", id: "t1" } },
          { seq: 1, part: { type: "text-delta", id: "t1", delta: "a" } },
          { seq: 1, part: { type: "text-delta", id: "t1", delta: "DUP" } }, // should be ignored
          { seq: 1.9, part: { type: "text-delta", id: "t1", delta: "FLOOR" } }, // floors to 1 (ignored)
        ],
      }),
    );
    expect(appended.ok).toBe(true);
    expect(appended.inserted).toBe(2);

    const finalized = await run(
      finalizeRunImpl(ctx, {
        threadId,
        anonKey,
        runId: created.runId,
        messageId: created.assistantMessageId,
        status: "final",
        text: "result",
      }),
    );
    expect(finalized).toEqual({ ok: true });

    const snapshot = await run(getThreadSnapshotImpl(ctx, { threadId, anonKey, maxMessages: 200, maxParts: 5000 }));
    expect(snapshot.ok).toBe(true);

    const roles = snapshot.messages.map((m: any) => m.role);
    expect(roles).toContain("assistant"); // welcome + assistant response
    expect(roles).toContain("user");

    const assistant = snapshot.messages.find((m: any) => m.messageId === created.assistantMessageId);
    expect(assistant?.status).toBe("final");
    expect(assistant?.text).toBe("result");

    const seqs = snapshot.parts.map((p: any) => Number(p.seq));
    expect(seqs).toEqual([0, 1]);
  });

  it("cancel is persisted on the run (requestCancel + isCancelRequested)", async () => {
    const db = makeInMemoryDb();
    const threadId = "thread-cancel-1";
    const anonKey = "anon-cancel-1";
    const ctx = anonCtx(db);

    await run(ensureAnonThreadImpl(ctx, { threadId, anonKey }));
    const created = await run(createRunImpl(ctx, { threadId, anonKey, text: "hi" }));

    const before = await run(isCancelRequestedImpl(ctx, { threadId, anonKey, runId: created.runId }));
    expect(before).toEqual({ ok: true, cancelRequested: false });

    await run(requestCancelImpl(ctx, { threadId, anonKey, runId: created.runId }));

    const after = await run(isCancelRequestedImpl(ctx, { threadId, anonKey, runId: created.runId }));
    expect(after).toEqual({ ok: true, cancelRequested: true });
  });

  it("resetThread deletes messages/parts/runs/receipts and re-seeds welcome + default blueprint", async () => {
    const db = makeInMemoryDb();
    const threadId = "thread-reset-1";
    const anonKey = "anon-reset-1";
    const ctx = anonCtx(db);

    await run(ensureAnonThreadImpl(ctx, { threadId, anonKey }));
    const created = await run(createRunImpl(ctx, { threadId, anonKey, text: "hello" }));

    await run(
      appendPartsImpl(ctx, {
        threadId,
        anonKey,
        runId: created.runId,
        messageId: created.assistantMessageId,
        parts: [{ seq: 0, part: { type: "text-delta", id: "t1", delta: "x" } }],
      }),
    );

    await db.insert("receipts", { threadId, runId: created.runId, kind: "model", json: { ok: true }, createdAtMs: 1 });

    // Mutate blueprint away from default to ensure reset restores it.
    await run(setBlueprintImpl(ctx, { threadId, anonKey, blueprint: { foo: "bar" } }));

    await run(resetThreadImpl(ctx, { threadId, anonKey }));

    expect(db.__tables.runs).toHaveLength(0);
    expect(db.__tables.messageParts).toHaveLength(0);
    expect(db.__tables.receipts).toHaveLength(0);

    expect(db.__tables.messages).toHaveLength(1);
    expect(db.__tables.messages[0]).toMatchObject({
      threadId,
      role: "assistant",
      status: "final",
      text: FIRST_OPEN_WELCOME_MESSAGE,
    });

    const blueprint = await run(getBlueprintImpl(ctx, { threadId, anonKey }));
    expect(blueprint.ok).toBe(true);
    expect(blueprint.blueprint?.bootstrapState?.threadId).toBe(threadId);
    expect(blueprint.blueprint?.docs?.identity?.name).toBe("Autopilot");
  });

  it("blueprint get/set/reset works (Convex canonical state)", async () => {
    const db = makeInMemoryDb();
    const threadId = "thread-blueprint-1";
    const anonKey = "anon-blueprint-1";
    const ctx = anonCtx(db);

    await run(ensureAnonThreadImpl(ctx, { threadId, anonKey }));

    const initial = await run(getBlueprintImpl(ctx, { threadId, anonKey }));
    expect(initial.ok).toBe(true);
    expect(initial.blueprint?.bootstrapState?.threadId).toBe(threadId);

    const set = await run(setBlueprintImpl(ctx, { threadId, anonKey, blueprint: { hello: "world" } }));
    expect(set.ok).toBe(true);
    const afterSet = await run(getBlueprintImpl(ctx, { threadId, anonKey }));
    expect(afterSet.blueprint).toEqual({ hello: "world" });

    const reset = await run(resetBlueprintImpl(ctx, { threadId, anonKey }));
    expect(reset.ok).toBe(true);
    const afterReset = await run(getBlueprintImpl(ctx, { threadId, anonKey }));
    expect(afterReset.blueprint?.bootstrapState?.threadId).toBe(threadId);
    expect(afterReset.blueprint?.docs?.identity?.name).toBe("Autopilot");
  });

  it("thread snapshots can be requested with maxMessages/maxParts = 0", async () => {
    const db = makeInMemoryDb();
    const threadId = "thread-snapshot-0";
    const anonKey = "anon-snapshot-0";
    const ctx = anonCtx(db);

    await run(ensureAnonThreadImpl(ctx, { threadId, anonKey }));
    const created = await run(createRunImpl(ctx, { threadId, anonKey, text: "hi" }));
    await run(
      appendPartsImpl(ctx, {
        threadId,
        anonKey,
        runId: created.runId,
        messageId: created.assistantMessageId,
        parts: [{ seq: 0, part: { type: "text-delta", delta: "x" } }],
      }),
    );

    const snapshot = await run(getThreadSnapshotImpl(ctx, { threadId, anonKey, maxMessages: 0, maxParts: 0 }));
    expect(snapshot.ok).toBe(true);
    expect(snapshot.messages).toEqual([]);
    expect(snapshot.parts).toEqual([]);
  });
});


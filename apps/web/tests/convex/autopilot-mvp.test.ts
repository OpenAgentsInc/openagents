import { describe, expect, it } from "vitest";
import { Effect, Option } from "effect";

import { makeInMemoryDb } from "./inMemoryDb";

import { FIRST_OPEN_WELCOME_MESSAGE } from "../../convex/autopilot/defaults";
import { getBlueprintImpl, resetBlueprintImpl, setBlueprintImpl } from "../../convex/autopilot/blueprint";
import { listFeatureRequestsForThreadImpl, recordFeatureRequestImpl } from "../../convex/autopilot/featureRequests";
import { getThreadTraceBundleImpl } from "../../convex/autopilot/traces";
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
    // Legacy threads may retain anonKey even after claim; access is owner-only in the product path.
    expect(db.__tables.threads[0].anonKey).toBe(anonKey);

    expect(db.__tables.users).toHaveLength(1);
    expect(db.__tables.users[0]).toMatchObject({ userId: "user-1", defaultThreadId: threadId });

    // Best-effort: legacy callers should not throw. Ownership must remain unchanged.
    const other = await run(claimAnonThreadImpl(authedCtx(db, "user-2"), { threadId, anonKey }));
    expect(other).toEqual({ ok: true, threadId });
    expect(db.__tables.threads[0].ownerId).toBe("user-1");
  });

  it("createRun + appendParts + finalizeRun persists canonical chat state (idempotent parts)", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "user-1");
    const ensured = await run(ensureOwnedThreadImpl(ctx));
    const threadId = ensured.threadId;

    const created = await run(createRunImpl(ctx, { threadId, text: "hello" }));
    expect(created.ok).toBe(true);
    expect(typeof created.runId).toBe("string");
    expect(typeof created.userMessageId).toBe("string");
    expect(typeof created.assistantMessageId).toBe("string");

    const appended = await run(
      appendPartsImpl(ctx, {
        threadId,
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
        runId: created.runId,
        messageId: created.assistantMessageId,
        status: "final",
        text: "result",
      }),
    );
    expect(finalized).toEqual({ ok: true });

    const snapshot = await run(getThreadSnapshotImpl(ctx, { threadId, maxMessages: 200, maxParts: 5000 }));
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
    const ctx = authedCtx(db, "user-1");
    const ensured = await run(ensureOwnedThreadImpl(ctx));
    const threadId = ensured.threadId;
    const created = await run(createRunImpl(ctx, { threadId, text: "hi" }));

    const before = await run(isCancelRequestedImpl(ctx, { threadId, runId: created.runId }));
    expect(before).toEqual({ ok: true, cancelRequested: false });

    await run(requestCancelImpl(ctx, { threadId, runId: created.runId }));

    const after = await run(isCancelRequestedImpl(ctx, { threadId, runId: created.runId }));
    expect(after).toEqual({ ok: true, cancelRequested: true });
  });

  it("records feature requests idempotently by runId and lists them per thread", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "user-1");
    const ensured = await run(ensureOwnedThreadImpl(ctx));
    const threadId = ensured.threadId;
    const created = await run(createRunImpl(ctx, { threadId, text: "connect github and cloud codex" }));

    const first = await run(
      recordFeatureRequestImpl(ctx, {
        threadId,
        runId: created.runId,
        messageId: created.userMessageId,
        userText: "Connect to my private GitHub and run Codex remotely in cloud.",
        capabilityKey: "github_cloud_codex",
        capabilityLabel: "GitHub + cloud Codex execution",
        summary: "User asks for GitHub integration and remote cloud Codex execution.",
        confidence: 0.94,
        notifyWhenAvailable: false,
        source: {
          signatureId: "@openagents/autopilot/feedback/DetectUpgradeRequest.v1",
          receiptId: "receipt-1",
          modelId: "moonshotai/kimi-k2.5",
          provider: "openrouter",
        },
      }),
    );
    expect(first.ok).toBe(true);
    expect(first.existed).toBe(false);
    expect(first.featureRequestId.startsWith("fr_")).toBe(true);

    const second = await run(
      recordFeatureRequestImpl(ctx, {
        threadId,
        runId: created.runId,
        messageId: created.userMessageId,
        userText: "Connect to my private GitHub and run Codex remotely in cloud.",
        capabilityKey: "github_cloud_codex",
        capabilityLabel: "GitHub + cloud Codex execution",
        summary: "Same request, idempotent update path.",
        confidence: 0.9,
        notifyWhenAvailable: true,
        source: {
          signatureId: "@openagents/autopilot/feedback/DetectUpgradeRequest.v1",
          receiptId: "receipt-2",
          modelId: "moonshotai/kimi-k2.5",
          provider: "openrouter",
        },
      }),
    );
    expect(second.ok).toBe(true);
    expect(second.existed).toBe(true);
    expect(second.featureRequestId).toBe(first.featureRequestId);

    const listed = await run(listFeatureRequestsForThreadImpl(ctx, { threadId, limit: 10 }));
    expect(listed.ok).toBe(true);
    expect(listed.featureRequests).toHaveLength(1);
    expect(listed.featureRequests[0]).toMatchObject({
      featureRequestId: first.featureRequestId,
      threadId,
      runId: created.runId,
      capabilityKey: "github_cloud_codex",
      notifyWhenAvailable: true,
    });
  });

  it("returns a thread trace bundle with messages, parts, runs, receipts, and feature requests", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "user-1");
    const ensured = await run(ensureOwnedThreadImpl(ctx));
    const threadId = ensured.threadId;
    const created = await run(createRunImpl(ctx, { threadId, text: "trace me" }));

    await run(
      appendPartsImpl(ctx, {
        threadId,
        runId: created.runId,
        messageId: created.assistantMessageId,
        parts: [
          { seq: 0, part: { type: "text-start", id: "trace-1" } },
          { seq: 1, part: { type: "text-delta", id: "trace-1", delta: "hello" } },
          { seq: 2, part: { type: "finish", reason: "stop" } },
        ],
      }),
    );

    await run(
      finalizeRunImpl(ctx, {
        threadId,
        runId: created.runId,
        messageId: created.assistantMessageId,
        status: "final",
        text: "hello",
      }),
    );

    await db.insert("receipts", {
      threadId,
      runId: created.runId,
      kind: "dse.predict",
      json: { ok: true, receiptId: "r-test" },
      receiptId: "r-test",
      signatureId: "@openagents/autopilot/feedback/DetectUpgradeRequest.v1",
      createdAtMs: 1,
    });

    await run(
      recordFeatureRequestImpl(ctx, {
        threadId,
        runId: created.runId,
        messageId: created.userMessageId,
        userText: "connect github",
        capabilityKey: "github_repo_access",
        capabilityLabel: "GitHub repository access",
        summary: "User asked for GitHub repository access.",
        confidence: 0.9,
        notifyWhenAvailable: false,
        source: {
          signatureId: "@openagents/autopilot/feedback/DetectUpgradeRequest.v1",
        },
      }),
    );

    const bundle = await run(
      getThreadTraceBundleImpl(ctx, {
        threadId,
        includeDseState: true,
      }),
    );

    expect(bundle.ok).toBe(true);
    expect(bundle.thread.threadId).toBe(threadId);
    expect(bundle.summary.messageCount).toBeGreaterThan(0);
    expect(bundle.summary.partCount).toBeGreaterThan(0);
    expect(bundle.summary.runCount).toBeGreaterThan(0);
    expect(bundle.summary.receiptCount).toBeGreaterThan(0);
    expect(bundle.summary.featureRequestCount).toBeGreaterThan(0);
    expect(Array.isArray(bundle.messages)).toBe(true);
    expect(Array.isArray(bundle.parts)).toBe(true);
    expect(Array.isArray(bundle.runs)).toBe(true);
    expect(Array.isArray(bundle.receipts)).toBe(true);
    expect(Array.isArray(bundle.featureRequests)).toBe(true);
  });

  it("resetThread deletes messages/parts/runs/receipts and re-seeds welcome + default blueprint", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "user-1");
    const ensured = await run(ensureOwnedThreadImpl(ctx));
    const threadId = ensured.threadId;
    const created = await run(createRunImpl(ctx, { threadId, text: "hello" }));

    await run(
      appendPartsImpl(ctx, {
        threadId,
        runId: created.runId,
        messageId: created.assistantMessageId,
        parts: [{ seq: 0, part: { type: "text-delta", id: "t1", delta: "x" } }],
      }),
    );

    await db.insert("receipts", { threadId, runId: created.runId, kind: "model", json: { ok: true }, createdAtMs: 1 });

    // Mutate blueprint away from default to ensure reset restores it.
    await run(setBlueprintImpl(ctx, { threadId, blueprint: { foo: "bar" } }));

    await run(resetThreadImpl(ctx, { threadId }));

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

    const blueprint = await run(getBlueprintImpl(ctx, { threadId }));
    expect(blueprint.ok).toBe(true);
    expect(blueprint.blueprint?.bootstrapState?.threadId).toBe(threadId);
    expect(blueprint.blueprint?.docs?.identity?.name).toBe("Autopilot");
  });

  it("blueprint get/set/reset works (Convex canonical state)", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "user-1");
    const ensured = await run(ensureOwnedThreadImpl(ctx));
    const threadId = ensured.threadId;

    const initial = await run(getBlueprintImpl(ctx, { threadId }));
    expect(initial.ok).toBe(true);
    expect(initial.blueprint?.bootstrapState?.threadId).toBe(threadId);

    const set = await run(setBlueprintImpl(ctx, { threadId, blueprint: { hello: "world" } }));
    expect(set.ok).toBe(true);
    const afterSet = await run(getBlueprintImpl(ctx, { threadId }));
    expect(afterSet.blueprint).toEqual({ hello: "world" });

    const reset = await run(resetBlueprintImpl(ctx, { threadId }));
    expect(reset.ok).toBe(true);
    const afterReset = await run(getBlueprintImpl(ctx, { threadId }));
    expect(afterReset.blueprint?.bootstrapState?.threadId).toBe(threadId);
    expect(afterReset.blueprint?.docs?.identity?.name).toBe("Autopilot");
  });

  it("thread snapshots can be requested with maxMessages/maxParts = 0", async () => {
    const db = makeInMemoryDb();
    const ctx = authedCtx(db, "user-1");
    const ensured = await run(ensureOwnedThreadImpl(ctx));
    const threadId = ensured.threadId;
    const created = await run(createRunImpl(ctx, { threadId, text: "hi" }));
    await run(
      appendPartsImpl(ctx, {
        threadId,
        runId: created.runId,
        messageId: created.assistantMessageId,
        parts: [{ seq: 0, part: { type: "text-delta", delta: "x" } }],
      }),
    );

    const snapshot = await run(getThreadSnapshotImpl(ctx, { threadId, maxMessages: 0, maxParts: 0 }));
    expect(snapshot.ok).toBe(true);
    expect(snapshot.messages).toEqual([]);
    expect(snapshot.parts).toEqual([]);
  });
});

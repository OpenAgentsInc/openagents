import { v } from "convex/values";
import { Effect } from "effect";

import type { EffectMutationCtx, EffectQueryCtx } from "../effect/ctx";
import { effectMutation, effectQuery } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

import { assertThreadAccess } from "../autopilot/access";
import { requireOpsAdmin } from "./opsAdmin";

const nowMs = () => Date.now();

const asString = (u: unknown): string | null => (typeof u === "string" && u.length > 0 ? u : null);

const recordCanaryHistory = (
  ctx: EffectMutationCtx,
  input: {
    readonly signatureId: string;
    readonly action: "auto_stop";
    readonly control_compiled_id?: string | undefined;
    readonly canary_compiled_id?: string | undefined;
    readonly rolloutPct?: number | undefined;
    readonly okCount?: number | undefined;
    readonly errorCount?: number | undefined;
    readonly reason?: string | undefined;
  },
) =>
  tryPromise(() =>
    ctx.db.insert("dseCanaryHistory", {
      signatureId: input.signatureId,
      action: input.action,
      ...(input.control_compiled_id ? { control_compiled_id: input.control_compiled_id } : {}),
      ...(input.canary_compiled_id ? { canary_compiled_id: input.canary_compiled_id } : {}),
      ...(typeof input.rolloutPct === "number" ? { rolloutPct: input.rolloutPct } : {}),
      ...(typeof input.okCount === "number" ? { okCount: input.okCount } : {}),
      ...(typeof input.errorCount === "number" ? { errorCount: input.errorCount } : {}),
      ...(input.reason ? { reason: input.reason } : {}),
      createdAtMs: nowMs(),
    }),
  );

export const recordPredictReceiptImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly threadId: string;
    readonly anonKey?: string | undefined;
    readonly runId: string;
    readonly receipt: unknown;
  },
) =>
  Effect.gen(function* () {
    yield* assertThreadAccess(ctx, args);

    const receipt = args.receipt as any;
    const receiptId = asString(receipt?.receiptId);
    const signatureId = asString(receipt?.signatureId);
    const compiled_id = asString(receipt?.compiled_id);

    yield* tryPromise(() =>
      ctx.db.insert("receipts", {
        threadId: args.threadId,
        runId: args.runId,
        kind: "dse.predict",
        json: args.receipt,
        ...(receiptId ? { receiptId } : {}),
        ...(signatureId ? { signatureId } : {}),
        ...(compiled_id ? { compiled_id } : {}),
        createdAtMs: nowMs(),
      }),
    );

    // Stage 6: MVP auto-stop canary on high error rate.
    // This is intentionally simple (count-based) and only observes the active canary compiled_id.
    yield* Effect.gen(function* () {
      if (!signatureId || !compiled_id) return;

      const canary = yield* tryPromise(() =>
        ctx.db
          .query("dseCanaries")
          .withIndex("by_signatureId", (q) => q.eq("signatureId", signatureId))
          .unique(),
      );

      if (!canary) return;
      if (!(canary as any).enabled) return;

      const canary_compiled_id = String((canary as any).canary_compiled_id ?? "");
      if (!canary_compiled_id || canary_compiled_id !== compiled_id) return;

      const okCount0 = Number((canary as any).okCount ?? 0);
      const errorCount0 = Number((canary as any).errorCount ?? 0);
      const minSamples = Math.max(1, Math.floor(Number((canary as any).minSamples ?? 20)));
      const maxErrorRateRaw = Number((canary as any).maxErrorRate ?? 0.2);
      const maxErrorRate = Number.isFinite(maxErrorRateRaw) ? Math.max(0, Math.min(1, maxErrorRateRaw)) : 0.2;

      const tag = String(receipt?.result?._tag ?? "");
      const ok = tag === "Ok";
      const err = tag === "Error";
      if (!ok && !err) return;

      const okCount = okCount0 + (ok ? 1 : 0);
      const errorCount = errorCount0 + (err ? 1 : 0);
      const total = okCount + errorCount;

      // Update counters first (so a concurrent reader sees the latest counts).
      yield* tryPromise(() =>
        ctx.db.patch((canary as any)._id, { okCount, errorCount, updatedAtMs: nowMs() }),
      );

      if (total < minSamples) return;
      const errorRate = total === 0 ? 0 : errorCount / total;
      if (errorRate <= maxErrorRate) return;

      // Auto-stop: remove canary config and record a history entry.
      yield* tryPromise(() => ctx.db.delete((canary as any)._id));

      yield* recordCanaryHistory(ctx, {
        signatureId,
        action: "auto_stop",
        control_compiled_id: asString((canary as any).control_compiled_id) ?? undefined,
        canary_compiled_id: canary_compiled_id,
        rolloutPct: typeof (canary as any).rolloutPct === "number" ? Number((canary as any).rolloutPct) : undefined,
        okCount,
        errorCount,
        reason: `error_rate_exceeded errorRate=${errorRate.toFixed(3)} maxErrorRate=${maxErrorRate.toFixed(3)} minSamples=${minSamples}`,
      });
    }).pipe(Effect.catchAll(() => Effect.void));

    return { ok: true };
  });

export const recordPredictReceipt = effectMutation({
  args: {
    threadId: v.string(),
    anonKey: v.optional(v.string()),
    runId: v.string(),
    receipt: v.any(),
  },
  returns: v.object({ ok: v.boolean() }),
  handler: recordPredictReceiptImpl,
});

export const getPredictReceiptByReceiptIdImpl = (
  ctx: EffectQueryCtx,
  args: { readonly receiptId: string },
) =>
  Effect.gen(function* () {
    const row = yield* tryPromise(() =>
      ctx.db.query("receipts").withIndex("by_receiptId", (q) => q.eq("receiptId", args.receiptId)).unique(),
    );

    if (!row) return { ok: true as const, receipt: null };

    const kind = String((row as any).kind ?? "");
    if (kind !== "dse.predict") return { ok: true as const, receipt: null };

    const threadId = String((row as any).threadId ?? "");
    const runId = String((row as any).runId ?? "");

    yield* assertThreadAccess(ctx, { threadId });

    return {
      ok: true as const,
      receipt: {
        threadId,
        runId,
        json: (row as any).json ?? null,
        createdAtMs: Number((row as any).createdAtMs ?? 0),
      },
    };
  });

export const getPredictReceiptByReceiptId = effectQuery({
  args: { receiptId: v.string() },
  returns: v.object({
    ok: v.boolean(),
    receipt: v.union(
      v.null(),
      v.object({
        threadId: v.string(),
        runId: v.string(),
        json: v.any(),
        createdAtMs: v.number(),
      }),
    ),
  }),
  handler: getPredictReceiptByReceiptIdImpl,
});

export const getPredictReceiptByReceiptIdAdminImpl = (
  ctx: EffectQueryCtx,
  args: { readonly receiptId: string },
) =>
  Effect.gen(function* () {
    yield* requireOpsAdmin(ctx);

    const row = yield* tryPromise(() =>
      ctx.db.query("receipts").withIndex("by_receiptId", (q) => q.eq("receiptId", args.receiptId)).unique(),
    );

    if (!row) return { ok: true as const, receipt: null };

    const kind = String((row as any).kind ?? "");
    if (kind !== "dse.predict") return { ok: true as const, receipt: null };

    const threadId = String((row as any).threadId ?? "");
    const runId = String((row as any).runId ?? "");

    return {
      ok: true as const,
      receipt: {
        threadId,
        runId,
        json: (row as any).json ?? null,
        createdAtMs: Number((row as any).createdAtMs ?? 0),
      },
    };
  });

export const getPredictReceiptByReceiptIdAdmin = effectQuery({
  args: { receiptId: v.string() },
  returns: v.object({
    ok: v.boolean(),
    receipt: v.union(
      v.null(),
      v.object({
        threadId: v.string(),
        runId: v.string(),
        json: v.any(),
        createdAtMs: v.number(),
      }),
    ),
  }),
  handler: getPredictReceiptByReceiptIdAdminImpl,
});

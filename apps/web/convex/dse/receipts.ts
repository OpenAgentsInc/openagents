import { v } from "convex/values";
import { Effect } from "effect";

import type { EffectMutationCtx } from "../effect/ctx";
import { effectMutation } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

import { assertThreadAccess } from "../autopilot/access";

const nowMs = () => Date.now();

const asString = (u: unknown): string | null => (typeof u === "string" && u.length > 0 ? u : null);

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


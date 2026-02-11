import { v } from "convex/values";
import { Effect } from "effect";

import type { EffectMutationCtx } from "../effect/ctx";
import { effectMutation } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

const BATCH_SIZE = 256;

const assertResetSecret = (secret: string) => {
  const expected = process.env.OA_TEST_RESET_SECRET;
  if (!expected) {
    throw new Error("testing.resetAll is disabled (OA_TEST_RESET_SECRET not set)");
  }
  if (secret !== expected) {
    throw new Error("forbidden");
  }
};

const deleteAllFrom = (ctx: EffectMutationCtx, table: any) =>
  Effect.gen(function* () {
    let deleted = 0;
    // Delete in batches so we don't exceed mutation limits if the dev DB has residue.
    for (;;) {
      const rows: any[] = yield* tryPromise(() => ctx.db.query(table).take(BATCH_SIZE));
      if (!rows.length) break;
      yield* Effect.forEach(rows, (row) => tryPromise(() => ctx.db.delete(row._id)), { discard: true });
      deleted += rows.length;
    }
    return deleted;
  });

export const resetAllImpl = (ctx: EffectMutationCtx, args: { readonly secret: string }) =>
  Effect.gen(function* () {
    // This endpoint is *only* for wiping dev deployments used by tests.
    // Never set OA_TEST_RESET_SECRET in production.
    assertResetSecret(args.secret);

    // Children first.
    const deleted: Record<string, number> = {};
    deleted.dseActiveArtifactHistory = yield* deleteAllFrom(ctx, "dseActiveArtifactHistory");
    deleted.dseActiveArtifacts = yield* deleteAllFrom(ctx, "dseActiveArtifacts");
    deleted.dseArtifacts = yield* deleteAllFrom(ctx, "dseArtifacts");
    deleted.receipts = yield* deleteAllFrom(ctx, "receipts");
    deleted.messageParts = yield* deleteAllFrom(ctx, "messageParts");
    deleted.runs = yield* deleteAllFrom(ctx, "runs");
    deleted.messages = yield* deleteAllFrom(ctx, "messages");
    deleted.blueprints = yield* deleteAllFrom(ctx, "blueprints");
    deleted.autopilotFeatureRequests = yield* deleteAllFrom(ctx, "autopilotFeatureRequests");
    deleted.threads = yield* deleteAllFrom(ctx, "threads");
    deleted.users = yield* deleteAllFrom(ctx, "users");

    return { ok: true as const, deleted };
  });

/**
 * Wipe the entire dev deployment state (Autopilot MVP tables).
 *
 * Security: guarded by OA_TEST_RESET_SECRET and intended for tests only.
 */
export const resetAll = effectMutation({
  args: { secret: v.string() },
  returns: v.object({ ok: v.boolean(), deleted: v.record(v.string(), v.number()) }),
  handler: resetAllImpl,
});

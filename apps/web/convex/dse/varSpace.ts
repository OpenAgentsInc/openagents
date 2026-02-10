import { v } from "convex/values";
import { Effect } from "effect";

import type { EffectMutationCtx, EffectQueryCtx } from "../effect/ctx";
import { effectMutation, effectQuery } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

import { assertThreadAccess } from "../autopilot/access";

const nowMs = () => Date.now();

const approxCharsOfJson = (value: unknown): number => {
  try {
    return JSON.stringify(value).length;
  } catch {
    return 0;
  }
};

type Row = {
  readonly _id: any;
  readonly name?: unknown;
  readonly kind?: unknown;
  readonly approxChars?: unknown;
  readonly blob?: unknown;
  readonly json?: unknown;
};

const queryOne = (
  ctx: EffectQueryCtx | EffectMutationCtx,
  args: { readonly threadId: string; readonly runId: string; readonly name: string },
) =>
  tryPromise(() =>
    ctx.db
      .query("dseVarSpace")
      .withIndex("by_threadId_runId_name", (q) =>
        q.eq("threadId", args.threadId).eq("runId", args.runId).eq("name", args.name),
      )
      .unique(),
  );

export const getVarImpl = (
  ctx: EffectQueryCtx,
  args: { readonly threadId: string; readonly runId: string; readonly name: string },
) =>
  Effect.gen(function* () {
    yield* assertThreadAccess(ctx, args);
    const row = (yield* queryOne(ctx, args)) as Row | null;
    if (!row) return { ok: true as const, value: null };

    const kind = String(row.kind ?? "");
    if (kind === "json") {
      return {
        ok: true as const,
        value: { _tag: "Json" as const, value: (row as any).json, approxChars: Number(row.approxChars ?? 0) },
      };
    }
    if (kind === "blob") {
      return { ok: true as const, value: { _tag: "Blob" as const, blob: (row as any).blob } };
    }

    // Defensive: unknown kinds are treated as missing.
    return { ok: true as const, value: null };
  });

export const getVar = effectQuery({
  args: {
    threadId: v.string(),
    runId: v.string(),
    name: v.string(),
  },
  returns: v.object({
    ok: v.boolean(),
    value: v.union(v.null(), v.any()),
  }),
  handler: getVarImpl,
});

export const putJsonImpl = (
  ctx: EffectMutationCtx,
  args: { readonly threadId: string; readonly runId: string; readonly name: string; readonly value: unknown },
) =>
  Effect.gen(function* () {
    yield* assertThreadAccess(ctx, args);
    const existing = (yield* queryOne(ctx, args)) as Row | null;

    const approxChars = approxCharsOfJson(args.value);
    const patch = {
      kind: "json" as const,
      json: args.value,
      approxChars,
      blob: undefined,
      updatedAtMs: nowMs(),
    };

    if (existing) {
      yield* tryPromise(() => ctx.db.patch((existing as any)._id, patch as any));
    } else {
      yield* tryPromise(() =>
        ctx.db.insert("dseVarSpace", {
          threadId: args.threadId,
          runId: args.runId,
          name: args.name,
          kind: "json",
          json: args.value,
          approxChars,
          createdAtMs: nowMs(),
          updatedAtMs: nowMs(),
        }),
      );
    }

    return { ok: true as const };
  });

export const putJson = effectMutation({
  args: {
    threadId: v.string(),
    runId: v.string(),
    name: v.string(),
    value: v.any(),
  },
  returns: v.object({ ok: v.boolean() }),
  handler: putJsonImpl,
});

export const putBlobImpl = (
  ctx: EffectMutationCtx,
  args: { readonly threadId: string; readonly runId: string; readonly name: string; readonly blob: unknown },
) =>
  Effect.gen(function* () {
    yield* assertThreadAccess(ctx, args);
    const existing = (yield* queryOne(ctx, args)) as Row | null;

    const patch = {
      kind: "blob" as const,
      blob: args.blob,
      json: undefined,
      approxChars: undefined,
      updatedAtMs: nowMs(),
    };

    if (existing) {
      yield* tryPromise(() => ctx.db.patch((existing as any)._id, patch as any));
    } else {
      yield* tryPromise(() =>
        ctx.db.insert("dseVarSpace", {
          threadId: args.threadId,
          runId: args.runId,
          name: args.name,
          kind: "blob",
          blob: args.blob,
          createdAtMs: nowMs(),
          updatedAtMs: nowMs(),
        }),
      );
    }

    return { ok: true as const };
  });

export const putBlob = effectMutation({
  args: {
    threadId: v.string(),
    runId: v.string(),
    name: v.string(),
    blob: v.any(),
  },
  returns: v.object({ ok: v.boolean() }),
  handler: putBlobImpl,
});

export const delImpl = (
  ctx: EffectMutationCtx,
  args: { readonly threadId: string; readonly runId: string; readonly name: string },
) =>
  Effect.gen(function* () {
    yield* assertThreadAccess(ctx, args);
    const row = (yield* queryOne(ctx, args)) as Row | null;
    if (row) yield* tryPromise(() => ctx.db.delete((row as any)._id));
    return { ok: true as const };
  });

export const del = effectMutation({
  args: { threadId: v.string(), runId: v.string(), name: v.string() },
  returns: v.object({ ok: v.boolean() }),
  handler: delImpl,
});

export const listImpl = (
  ctx: EffectQueryCtx,
  args: { readonly threadId: string; readonly runId: string },
) =>
  Effect.gen(function* () {
    yield* assertThreadAccess(ctx, args);

    const rows = yield* tryPromise(() =>
      ctx.db
        .query("dseVarSpace")
        .withIndex("by_threadId_runId_updatedAtMs", (q) =>
          q.eq("threadId", args.threadId).eq("runId", args.runId),
        )
        .collect(),
    );

    const out = (rows as ReadonlyArray<Row>).map((r) => {
      const name = String(r.name ?? "");
      const kind = String(r.kind ?? "");
      if (kind === "json") {
        return { name, kind: "json" as const, approxChars: Number((r as any).approxChars ?? 0) };
      }
      if (kind === "blob") {
        return { name, kind: "blob" as const, blob: (r as any).blob };
      }
      return { name, kind: "json" as const, approxChars: 0 };
    });

    out.sort((a, b) => a.name.localeCompare(b.name));

    return { ok: true as const, vars: out };
  });

export const list = effectQuery({
  args: { threadId: v.string(), runId: v.string() },
  returns: v.object({ ok: v.boolean(), vars: v.array(v.any()) }),
  handler: listImpl,
});


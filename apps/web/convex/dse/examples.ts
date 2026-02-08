import { v } from "convex/values";
import { Effect } from "effect";

import type { EffectMutationCtx, EffectQueryCtx } from "../effect/ctx";
import { effectMutation, effectQuery } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

import { getSubject } from "../autopilot/access";

const nowMs = () => Date.now();

const asStringArray = (u: unknown): Array<string> | null => {
  if (!Array.isArray(u)) return null;
  const out: string[] = [];
  for (const v of u) {
    if (typeof v === "string" && v.length > 0) out.push(v);
  }
  return out;
};

const splitSchema = v.union(v.literal("train"), v.literal("dev"), v.literal("test"));

type Split = "train" | "dev" | "test";

const splitOrNull = (u: unknown): Split | null =>
  u === "train" || u === "dev" || u === "test" ? (u as Split) : null;

const requireAuthed = (ctx: EffectQueryCtx | EffectMutationCtx) =>
  getSubject(ctx).pipe(
    Effect.flatMap((subject) => (subject ? Effect.succeed(subject) : Effect.fail(new Error("unauthorized")))),
  );

export const putExampleImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly signatureId: string;
    readonly exampleId: string;
    readonly inputJson: unknown;
    readonly expectedJson: unknown;
    readonly split?: Split | undefined;
    readonly tags?: ReadonlyArray<string> | undefined;
    readonly source?: string | undefined;
  },
) =>
  Effect.gen(function* () {
    yield* requireAuthed(ctx);

    const existing = yield* tryPromise(() =>
      ctx.db
        .query("dseExamples")
        .withIndex("by_signatureId_exampleId", (q) =>
          q.eq("signatureId", args.signatureId).eq("exampleId", args.exampleId),
        )
        .unique(),
    );

    const now = nowMs();
    const patch = {
      inputJson: args.inputJson,
      expectedJson: args.expectedJson,
      split: args.split,
      tags: Array.isArray(args.tags) ? [...args.tags] : undefined,
      source: typeof args.source === "string" ? args.source : undefined,
      updatedAtMs: now,
    };

    if (existing) {
      yield* tryPromise(() => ctx.db.patch((existing as any)._id, patch));
      return { ok: true, existed: true as const };
    }

    yield* tryPromise(() =>
      ctx.db.insert("dseExamples", {
        signatureId: args.signatureId,
        exampleId: args.exampleId,
        inputJson: args.inputJson,
        expectedJson: args.expectedJson,
        split: args.split,
        tags: Array.isArray(args.tags) ? [...args.tags] : undefined,
        source: typeof args.source === "string" ? args.source : undefined,
        createdAtMs: now,
        updatedAtMs: now,
      }),
    );

    return { ok: true, existed: false as const };
  });

export const putExample = effectMutation({
  args: {
    signatureId: v.string(),
    exampleId: v.string(),
    inputJson: v.any(),
    expectedJson: v.any(),
    split: v.optional(splitSchema),
    tags: v.optional(v.array(v.string())),
    source: v.optional(v.string()),
  },
  returns: v.object({ ok: v.boolean(), existed: v.boolean() }),
  handler: putExampleImpl,
});

export const getExampleImpl = (
  ctx: EffectQueryCtx,
  args: { readonly signatureId: string; readonly exampleId: string },
) =>
  Effect.gen(function* () {
    yield* requireAuthed(ctx);

    const row = yield* tryPromise(() =>
      ctx.db
        .query("dseExamples")
        .withIndex("by_signatureId_exampleId", (q) =>
          q.eq("signatureId", args.signatureId).eq("exampleId", args.exampleId),
        )
        .unique(),
    );

    if (!row) return { ok: true, example: null };

    return {
      ok: true,
      example: {
        signatureId: String((row as any).signatureId ?? ""),
        exampleId: String((row as any).exampleId ?? ""),
        inputJson: (row as any).inputJson ?? null,
        expectedJson: (row as any).expectedJson ?? null,
        split: splitOrNull((row as any).split),
        tags: asStringArray((row as any).tags),
        source: typeof (row as any).source === "string" ? String((row as any).source) : null,
        createdAtMs: Number((row as any).createdAtMs ?? 0),
        updatedAtMs: Number((row as any).updatedAtMs ?? 0),
      },
    };
  });

export const getExample = effectQuery({
  args: {
    signatureId: v.string(),
    exampleId: v.string(),
  },
  returns: v.object({
    ok: v.boolean(),
    example: v.union(
      v.null(),
      v.object({
        signatureId: v.string(),
        exampleId: v.string(),
        inputJson: v.any(),
        expectedJson: v.any(),
        split: v.union(v.null(), splitSchema),
        tags: v.union(v.null(), v.array(v.string())),
        source: v.union(v.null(), v.string()),
        createdAtMs: v.number(),
        updatedAtMs: v.number(),
      }),
    ),
  }),
  handler: getExampleImpl,
});

export const listExamplesImpl = (
  ctx: EffectQueryCtx,
  args: { readonly signatureId: string; readonly split?: Split | undefined; readonly limit?: number | undefined },
) =>
  Effect.gen(function* () {
    yield* requireAuthed(ctx);

    const limit =
      typeof args.limit === "number" && Number.isFinite(args.limit)
        ? Math.max(0, Math.min(500, Math.floor(args.limit)))
        : 200;

    const rows: any[] =
      limit === 0
        ? []
        : yield* tryPromise(() =>
            ctx.db
              .query("dseExamples")
              .withIndex("by_signatureId_exampleId", (q) => q.eq("signatureId", args.signatureId))
              .collect(),
          );

    const targetSplit = args.split;

    const examples = rows
      .map((r) => ({
        signatureId: String(r.signatureId ?? ""),
        exampleId: String(r.exampleId ?? ""),
        inputJson: r.inputJson ?? null,
        expectedJson: r.expectedJson ?? null,
        split: splitOrNull(r.split),
        tags: asStringArray(r.tags),
        source: typeof r.source === "string" ? String(r.source) : null,
        createdAtMs: Number(r.createdAtMs ?? 0),
        updatedAtMs: Number(r.updatedAtMs ?? 0),
      }))
      .filter((ex) => (targetSplit ? ex.split === targetSplit : true))
      // Deterministic order: stable across DBs and independent of insertion order.
      .sort((a, b) => a.exampleId.localeCompare(b.exampleId))
      .slice(0, limit);

    return { ok: true, examples };
  });

export const listExamples = effectQuery({
  args: {
    signatureId: v.string(),
    split: v.optional(splitSchema),
    limit: v.optional(v.number()),
  },
  returns: v.object({
    ok: v.boolean(),
    examples: v.array(
      v.object({
        signatureId: v.string(),
        exampleId: v.string(),
        inputJson: v.any(),
        expectedJson: v.any(),
        split: v.union(v.null(), splitSchema),
        tags: v.union(v.null(), v.array(v.string())),
        source: v.union(v.null(), v.string()),
        createdAtMs: v.number(),
        updatedAtMs: v.number(),
      }),
    ),
  }),
  handler: listExamplesImpl,
});

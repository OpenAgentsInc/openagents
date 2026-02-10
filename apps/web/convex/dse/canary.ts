import { v } from "convex/values";
import { Effect } from "effect";

import type { EffectMutationCtx, EffectQueryCtx } from "../effect/ctx";
import { effectMutation, effectQuery } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

import { getSubject } from "../autopilot/access";

const nowMs = () => Date.now();

const requireAuthed = (ctx: EffectQueryCtx | EffectMutationCtx) =>
  getSubject(ctx).pipe(
    Effect.flatMap((subject) => (subject ? Effect.succeed(subject) : Effect.fail(new Error("unauthorized")))),
  );

const clampInt = (n: number, min: number, max: number): number =>
  Math.max(min, Math.min(max, Math.floor(n)));

const clamp01 = (n: number): number => (Number.isFinite(n) ? Math.max(0, Math.min(1, n)) : 0);

const recordHistory = (
  ctx: EffectMutationCtx,
  input: {
    readonly signatureId: string;
    readonly action: "start" | "stop" | "auto_stop" | "update";
    readonly control_compiled_id?: string | undefined;
    readonly canary_compiled_id?: string | undefined;
    readonly rolloutPct?: number | undefined;
    readonly okCount?: number | undefined;
    readonly errorCount?: number | undefined;
    readonly reason?: string | undefined;
    readonly actorUserId?: string | undefined;
  },
) =>
  Effect.gen(function* () {
    yield* tryPromise(() =>
      ctx.db.insert("dseCanaryHistory", {
        signatureId: input.signatureId,
        action: input.action,
        ...(input.control_compiled_id ? { control_compiled_id: input.control_compiled_id } : {}),
        ...(input.canary_compiled_id ? { canary_compiled_id: input.canary_compiled_id } : {}),
        ...(typeof input.rolloutPct === "number" ? { rolloutPct: input.rolloutPct } : {}),
        ...(typeof input.okCount === "number" ? { okCount: input.okCount } : {}),
        ...(typeof input.errorCount === "number" ? { errorCount: input.errorCount } : {}),
        ...(input.reason ? { reason: input.reason } : {}),
        ...(input.actorUserId ? { actorUserId: input.actorUserId } : {}),
        createdAtMs: nowMs(),
      }),
    );
  });

export const getCanaryImpl = (
  ctx: EffectQueryCtx,
  args: { readonly signatureId: string; readonly threadId?: string | undefined },
) =>
  Effect.gen(function* () {
    const row = yield* tryPromise(() =>
      ctx.db
        .query("dseCanaries")
        .withIndex("by_signatureId", (q) => q.eq("signatureId", args.signatureId))
        .unique(),
    );

    if (!row) return { ok: true, canary: null };

    return {
      ok: true,
      canary: {
        signatureId: String((row as any).signatureId ?? ""),
        enabled: Boolean((row as any).enabled),
        control_compiled_id: String((row as any).control_compiled_id ?? ""),
        canary_compiled_id: String((row as any).canary_compiled_id ?? ""),
        rolloutPct: Number((row as any).rolloutPct ?? 0),
        salt: String((row as any).salt ?? ""),
        okCount: Number((row as any).okCount ?? 0),
        errorCount: Number((row as any).errorCount ?? 0),
        minSamples: Number((row as any).minSamples ?? 0),
        maxErrorRate: Number((row as any).maxErrorRate ?? 0),
        createdAtMs: Number((row as any).createdAtMs ?? 0),
        updatedAtMs: Number((row as any).updatedAtMs ?? 0),
      },
    };
  });

export const getCanary = effectQuery({
  args: { signatureId: v.string(), threadId: v.optional(v.string()) },
  returns: v.object({
    ok: v.boolean(),
    canary: v.union(
      v.null(),
      v.object({
        signatureId: v.string(),
        enabled: v.boolean(),
        control_compiled_id: v.string(),
        canary_compiled_id: v.string(),
        rolloutPct: v.number(),
        salt: v.string(),
        okCount: v.number(),
        errorCount: v.number(),
        minSamples: v.number(),
        maxErrorRate: v.number(),
        createdAtMs: v.number(),
        updatedAtMs: v.number(),
      }),
    ),
  }),
  handler: getCanaryImpl,
});

export const startCanaryImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly signatureId: string;
    readonly canary_compiled_id: string;
    readonly rolloutPct: number;
    readonly salt?: string | undefined;
    readonly minSamples?: number | undefined;
    readonly maxErrorRate?: number | undefined;
    readonly reason?: string | undefined;
  },
) =>
  Effect.gen(function* () {
    const actorUserId = yield* requireAuthed(ctx);

    const rolloutPct = clampInt(args.rolloutPct, 0, 100);
    const minSamples = clampInt(typeof args.minSamples === "number" ? args.minSamples : 20, 1, 10_000);
    const maxErrorRate = clamp01(typeof args.maxErrorRate === "number" ? args.maxErrorRate : 0.2);
    const salt =
      typeof args.salt === "string" && args.salt.trim().length > 0 ? args.salt.trim() : crypto.randomUUID();

    // Control = current active pointer.
    const active = yield* tryPromise(() =>
      ctx.db
        .query("dseActiveArtifacts")
        .withIndex("by_signatureId", (q) => q.eq("signatureId", args.signatureId))
        .unique(),
    );
    if (!active) return yield* Effect.fail(new Error("control_missing"));

    const control_compiled_id = String((active as any).compiled_id ?? "");
    if (!control_compiled_id) return yield* Effect.fail(new Error("control_missing"));

    // Ensure artifacts exist for control+canary.
    const controlArtifact = yield* tryPromise(() =>
      ctx.db
        .query("dseArtifacts")
        .withIndex("by_signatureId_compiled_id", (q) =>
          q.eq("signatureId", args.signatureId).eq("compiled_id", control_compiled_id),
        )
        .unique(),
    );
    if (!controlArtifact) return yield* Effect.fail(new Error("control_artifact_missing"));

    const canaryArtifact = yield* tryPromise(() =>
      ctx.db
        .query("dseArtifacts")
        .withIndex("by_signatureId_compiled_id", (q) =>
          q.eq("signatureId", args.signatureId).eq("compiled_id", args.canary_compiled_id),
        )
        .unique(),
    );
    if (!canaryArtifact) return yield* Effect.fail(new Error("canary_artifact_missing"));

    const existing = yield* tryPromise(() =>
      ctx.db
        .query("dseCanaries")
        .withIndex("by_signatureId", (q) => q.eq("signatureId", args.signatureId))
        .unique(),
    );

    const now = nowMs();
    if (existing) {
      yield* tryPromise(() =>
        ctx.db.patch((existing as any)._id, {
          control_compiled_id,
          canary_compiled_id: args.canary_compiled_id,
          rolloutPct,
          salt,
          enabled: true,
          okCount: 0,
          errorCount: 0,
          minSamples,
          maxErrorRate,
          updatedAtMs: now,
        }),
      );
    } else {
      yield* tryPromise(() =>
        ctx.db.insert("dseCanaries", {
          signatureId: args.signatureId,
          control_compiled_id,
          canary_compiled_id: args.canary_compiled_id,
          rolloutPct,
          salt,
          enabled: true,
          okCount: 0,
          errorCount: 0,
          minSamples,
          maxErrorRate,
          createdAtMs: now,
          updatedAtMs: now,
        }),
      );
    }

    yield* recordHistory(ctx, {
      signatureId: args.signatureId,
      action: existing ? "update" : "start",
      control_compiled_id,
      canary_compiled_id: args.canary_compiled_id,
      rolloutPct,
      okCount: 0,
      errorCount: 0,
      reason: args.reason,
      actorUserId,
    });

    return {
      ok: true,
      signatureId: args.signatureId,
      control_compiled_id,
      canary_compiled_id: args.canary_compiled_id,
      rolloutPct,
    };
  });

export const startCanary = effectMutation({
  args: {
    signatureId: v.string(),
    canary_compiled_id: v.string(),
    rolloutPct: v.number(),
    salt: v.optional(v.string()),
    minSamples: v.optional(v.number()),
    maxErrorRate: v.optional(v.number()),
    reason: v.optional(v.string()),
  },
  returns: v.object({
    ok: v.boolean(),
    signatureId: v.string(),
    control_compiled_id: v.string(),
    canary_compiled_id: v.string(),
    rolloutPct: v.number(),
  }),
  handler: startCanaryImpl,
});

export const stopCanaryImpl = (
  ctx: EffectMutationCtx,
  args: { readonly signatureId: string; readonly reason?: string | undefined },
) =>
  Effect.gen(function* () {
    const actorUserId = yield* requireAuthed(ctx);

    const existing = yield* tryPromise(() =>
      ctx.db
        .query("dseCanaries")
        .withIndex("by_signatureId", (q) => q.eq("signatureId", args.signatureId))
        .unique(),
    );
    if (!existing) return { ok: true, existed: false as const };

    const okCount = Number((existing as any).okCount ?? 0);
    const errorCount = Number((existing as any).errorCount ?? 0);
    const control_compiled_id = String((existing as any).control_compiled_id ?? "");
    const canary_compiled_id = String((existing as any).canary_compiled_id ?? "");
    const rolloutPct = Number((existing as any).rolloutPct ?? 0);

    yield* tryPromise(() => ctx.db.delete((existing as any)._id));

    yield* recordHistory(ctx, {
      signatureId: args.signatureId,
      action: "stop",
      control_compiled_id,
      canary_compiled_id,
      rolloutPct,
      okCount,
      errorCount,
      reason: args.reason,
      actorUserId,
    });

    return {
      ok: true,
      existed: true as const,
      signatureId: args.signatureId,
      control_compiled_id,
      canary_compiled_id,
      rolloutPct,
    };
  });

export const stopCanary = effectMutation({
  args: {
    signatureId: v.string(),
    reason: v.optional(v.string()),
  },
  returns: v.object({
    ok: v.boolean(),
    existed: v.boolean(),
    signatureId: v.optional(v.string()),
    control_compiled_id: v.optional(v.string()),
    canary_compiled_id: v.optional(v.string()),
    rolloutPct: v.optional(v.number()),
  }),
  handler: stopCanaryImpl,
});

export const listCanaryHistoryImpl = (
  ctx: EffectQueryCtx,
  args: { readonly signatureId: string; readonly limit?: number | undefined },
) =>
  Effect.gen(function* () {
    yield* requireAuthed(ctx);

    const limit =
      typeof args.limit === "number" && Number.isFinite(args.limit)
        ? Math.max(0, Math.min(200, Math.floor(args.limit)))
        : 50;

    const rows: any[] =
      limit === 0
        ? []
        : yield* tryPromise(() =>
            ctx.db
              .query("dseCanaryHistory")
              .withIndex("by_signatureId_createdAtMs", (q) => q.eq("signatureId", args.signatureId))
              .order("desc")
              .take(limit),
          );

    return {
      ok: true as const,
      history: rows.map((r) => ({
        signatureId: String(r.signatureId ?? ""),
        action:
          r.action === "start" || r.action === "stop" || r.action === "auto_stop" || r.action === "update"
            ? r.action
            : "start",
        control_compiled_id: typeof r.control_compiled_id === "string" ? String(r.control_compiled_id) : null,
        canary_compiled_id: typeof r.canary_compiled_id === "string" ? String(r.canary_compiled_id) : null,
        rolloutPct: typeof r.rolloutPct === "number" ? Number(r.rolloutPct) : null,
        okCount: typeof r.okCount === "number" ? Number(r.okCount) : null,
        errorCount: typeof r.errorCount === "number" ? Number(r.errorCount) : null,
        reason: typeof r.reason === "string" ? String(r.reason) : null,
        actorUserId: typeof r.actorUserId === "string" ? String(r.actorUserId) : null,
        createdAtMs: Number(r.createdAtMs ?? 0),
      })),
    };
  });

export const listCanaryHistory = effectQuery({
  args: { signatureId: v.string(), limit: v.optional(v.number()) },
  returns: v.object({
    ok: v.boolean(),
    history: v.array(
      v.object({
        signatureId: v.string(),
        action: v.union(v.literal("start"), v.literal("stop"), v.literal("auto_stop"), v.literal("update")),
        control_compiled_id: v.union(v.null(), v.string()),
        canary_compiled_id: v.union(v.null(), v.string()),
        rolloutPct: v.union(v.null(), v.number()),
        okCount: v.union(v.null(), v.number()),
        errorCount: v.union(v.null(), v.number()),
        reason: v.union(v.null(), v.string()),
        actorUserId: v.union(v.null(), v.string()),
        createdAtMs: v.number(),
      }),
    ),
  }),
  handler: listCanaryHistoryImpl,
});

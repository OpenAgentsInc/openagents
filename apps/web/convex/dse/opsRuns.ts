import { v } from "convex/values";
import { Effect } from "effect";

import type { EffectMutationCtx } from "../effect/ctx";
import { effectMutation } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

import { requireOpsAdmin } from "./opsAdmin";

const nowMs = () => Date.now();

const normalizeOptionalString = (raw: unknown, maxLen: number): string | undefined => {
  if (typeof raw !== "string") return undefined;
  const s = raw.trim();
  if (!s) return undefined;
  return s.slice(0, maxLen);
};

const normalizeOptionalStringArray = (raw: unknown, maxItems: number, maxLen: number): Array<string> | undefined => {
  if (!Array.isArray(raw)) return undefined;
  const out: string[] = [];
  for (const v of raw) {
    if (typeof v !== "string") continue;
    const s = v.trim();
    if (!s) continue;
    out.push(s.slice(0, maxLen));
    if (out.length >= maxItems) break;
  }
  return out.length ? out : undefined;
};

const safeJsonOrTruncated = (raw: unknown, maxChars: number): unknown => {
  try {
    const s = JSON.stringify(raw);
    if (typeof s !== "string") return { _tag: "unserializable_json" };
    if (s.length <= maxChars) return raw;
    return { _tag: "truncated_json", approxChars: s.length, preview: s.slice(0, maxChars) };
  } catch (err) {
    return { _tag: "unserializable_json", error: String(err) };
  }
};

type RunStatus = "running" | "finished" | "failed";

export const startRunImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly runId: string;
    readonly commitSha?: string | undefined;
    readonly baseUrl?: string | undefined;
    readonly signatureIds?: ReadonlyArray<string> | undefined;
    readonly notes?: string | undefined;
    readonly links?: unknown;
  },
) =>
  Effect.gen(function* () {
    const actorUserId = yield* requireOpsAdmin(ctx);

    const runId = String(args.runId ?? "").trim();
    if (!runId) return yield* Effect.fail(new Error("invalid_input"));
    if (runId.length > 200) return yield* Effect.fail(new Error("invalid_input"));

    const existing = yield* tryPromise(() =>
      ctx.db.query("dseOpsRuns").withIndex("by_runId", (q) => q.eq("runId", runId)).unique(),
    );
    if (existing) return { ok: true as const, existed: true as const, runId };

    const now = nowMs();
    const commitSha = normalizeOptionalString(args.commitSha, 80);
    const baseUrl = normalizeOptionalString(args.baseUrl, 300);
    const signatureIds = normalizeOptionalStringArray(args.signatureIds, 50, 200);
    const notes = normalizeOptionalString(args.notes, 10_000);
    const links = args.links === undefined ? undefined : safeJsonOrTruncated(args.links, 50_000);

    yield* tryPromise(() =>
      ctx.db.insert("dseOpsRuns", {
        runId,
        status: "running",
        startedAtMs: now,
        ...(commitSha ? { commitSha } : {}),
        ...(baseUrl ? { baseUrl } : {}),
        actorUserId,
        ...(signatureIds ? { signatureIds } : {}),
        ...(notes ? { notes } : {}),
        ...(links === undefined ? {} : { links }),
        createdAtMs: now,
        updatedAtMs: now,
      }),
    );

    yield* tryPromise(() =>
      ctx.db.insert("dseOpsRunEvents", {
        runId,
        tsMs: now,
        level: "info",
        phase: "phase1.start",
        message: "ops run started",
        createdAtMs: now,
      }),
    );

    return { ok: true as const, existed: false as const, runId };
  });

export const startRun = effectMutation({
  args: {
    runId: v.string(),
    commitSha: v.optional(v.string()),
    baseUrl: v.optional(v.string()),
    signatureIds: v.optional(v.array(v.string())),
    notes: v.optional(v.string()),
    links: v.optional(v.any()),
  },
  returns: v.object({ ok: v.boolean(), existed: v.boolean(), runId: v.string() }),
  handler: startRunImpl,
});

type EventLevel = "info" | "warn" | "error";

export const appendEventImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly runId: string;
    readonly level: EventLevel;
    readonly phase?: string | undefined;
    readonly message: string;
    readonly json?: unknown;
    readonly tsMs?: number | undefined;
  },
) =>
  Effect.gen(function* () {
    yield* requireOpsAdmin(ctx);

    const runId = String(args.runId ?? "").trim();
    if (!runId) return yield* Effect.fail(new Error("invalid_input"));
    if (runId.length > 200) return yield* Effect.fail(new Error("invalid_input"));

    const runRow = yield* tryPromise(() =>
      ctx.db.query("dseOpsRuns").withIndex("by_runId", (q) => q.eq("runId", runId)).unique(),
    );
    if (!runRow) return yield* Effect.fail(new Error("run_not_found"));

    const level = args.level;
    const phase = normalizeOptionalString(args.phase, 80);
    const message = String(args.message ?? "").trim().slice(0, 2_000);
    if (!message) return yield* Effect.fail(new Error("invalid_input"));

    const now = nowMs();
    const tsMs =
      typeof args.tsMs === "number" && Number.isFinite(args.tsMs) ? Math.max(0, Math.floor(args.tsMs)) : now;
    const json = args.json === undefined ? undefined : safeJsonOrTruncated(args.json, 50_000);

    yield* tryPromise(() =>
      ctx.db.insert("dseOpsRunEvents", {
        runId,
        tsMs,
        level,
        ...(phase ? { phase } : {}),
        message,
        ...(json === undefined ? {} : { json }),
        createdAtMs: now,
      }),
    );

    yield* tryPromise(() => ctx.db.patch((runRow as any)._id, { updatedAtMs: now }));

    return { ok: true as const };
  });

export const appendEvent = effectMutation({
  args: {
    runId: v.string(),
    level: v.union(v.literal("info"), v.literal("warn"), v.literal("error")),
    phase: v.optional(v.string()),
    message: v.string(),
    json: v.optional(v.any()),
    tsMs: v.optional(v.number()),
  },
  returns: v.object({ ok: v.boolean() }),
  handler: appendEventImpl,
});

export const finishRunImpl = (
  ctx: EffectMutationCtx,
  args: { readonly runId: string; readonly status: Exclude<RunStatus, "running">; readonly summaryJson?: unknown },
) =>
  Effect.gen(function* () {
    yield* requireOpsAdmin(ctx);

    const runId = String(args.runId ?? "").trim();
    if (!runId) return yield* Effect.fail(new Error("invalid_input"));
    if (runId.length > 200) return yield* Effect.fail(new Error("invalid_input"));

    const runRow = yield* tryPromise(() =>
      ctx.db.query("dseOpsRuns").withIndex("by_runId", (q) => q.eq("runId", runId)).unique(),
    );
    if (!runRow) return yield* Effect.fail(new Error("run_not_found"));

    const status = args.status === "failed" ? ("failed" as const) : ("finished" as const);

    const now = nowMs();
    const summaryJson = args.summaryJson === undefined ? undefined : safeJsonOrTruncated(args.summaryJson, 50_000);
    yield* tryPromise(() =>
      ctx.db.patch((runRow as any)._id, {
        status,
        endedAtMs: now,
        ...(summaryJson === undefined ? {} : { summaryJson }),
        updatedAtMs: now,
      }),
    );

    yield* tryPromise(() =>
      ctx.db.insert("dseOpsRunEvents", {
        runId,
        tsMs: now,
        level: status === "failed" ? ("error" as const) : ("info" as const),
        phase: "phase1.finish",
        message: `ops run finished status=${status}`,
        ...(summaryJson === undefined ? {} : { json: summaryJson }),
        createdAtMs: now,
      }),
    );

    return { ok: true as const };
  });

export const finishRun = effectMutation({
  args: {
    runId: v.string(),
    status: v.union(v.literal("finished"), v.literal("failed")),
    summaryJson: v.optional(v.any()),
  },
  returns: v.object({ ok: v.boolean() }),
  handler: finishRunImpl,
});

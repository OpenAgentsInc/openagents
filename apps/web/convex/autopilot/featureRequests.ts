import { v } from "convex/values";
import { Effect } from "effect";

import type { EffectMutationCtx, EffectQueryCtx } from "../effect/ctx";
import { effectMutation, effectQuery } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

import { assertThreadAccess } from "./access";

const nowMs = () => Date.now();
const newId = () => crypto.randomUUID();

const clampString = (value: string, max: number): string => value.trim().slice(0, Math.max(0, max));

const normalizeCapabilityKey = (value: string): string => {
  const normalized = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
  return normalized.length > 0 ? normalized.slice(0, 120) : "unknown";
};

const clampConfidence = (value: number): number => {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(1, value));
};

export const recordFeatureRequestImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly threadId: string;
    readonly anonKey?: string | undefined;
    readonly runId: string;
    readonly messageId: string;
    readonly userText: string;
    readonly capabilityKey: string;
    readonly capabilityLabel: string;
    readonly summary: string;
    readonly confidence: number;
    readonly notifyWhenAvailable: boolean;
    readonly source: {
      readonly signatureId: string;
      readonly compiled_id?: string;
      readonly receiptId?: string;
      readonly modelId?: string;
      readonly provider?: string;
      readonly route?: string;
      readonly fallbackModelId?: string;
    };
  },
) =>
  Effect.gen(function* () {
    const thread = yield* assertThreadAccess(ctx, args);

    const runId = clampString(args.runId, 128);
    const messageId = clampString(args.messageId, 128);
    const userText = clampString(args.userText, 8_000);
    const capabilityKey = normalizeCapabilityKey(args.capabilityKey);
    const capabilityLabel = clampString(args.capabilityLabel, 160) || capabilityKey;
    const summary = clampString(args.summary, 240);
    const confidence = clampConfidence(args.confidence);
    const sourceSignatureId = clampString(args.source.signatureId, 220);
    const now = nowMs();

    const existing = yield* tryPromise(() =>
      ctx.db
        .query("autopilotFeatureRequests")
        .withIndex("by_runId", (q) => q.eq("runId", runId))
        .unique(),
    );

    const source = {
      signatureId: sourceSignatureId,
      ...(args.source.compiled_id ? { compiled_id: clampString(args.source.compiled_id, 220) } : {}),
      ...(args.source.receiptId ? { receiptId: clampString(args.source.receiptId, 220) } : {}),
      ...(args.source.modelId ? { modelId: clampString(args.source.modelId, 220) } : {}),
      ...(args.source.provider ? { provider: clampString(args.source.provider, 120) } : {}),
      ...(args.source.route ? { route: clampString(args.source.route, 160) } : {}),
      ...(args.source.fallbackModelId ? { fallbackModelId: clampString(args.source.fallbackModelId, 220) } : {}),
    };

    if (existing) {
      yield* tryPromise(() =>
        ctx.db.patch(existing._id, {
          messageId,
          userText,
          capabilityKey,
          capabilityLabel,
          summary,
          confidence,
          notifyWhenAvailable: Boolean(args.notifyWhenAvailable),
          source,
          updatedAtMs: now,
        }),
      );
      yield* tryPromise(() => ctx.db.patch(thread._id, { updatedAtMs: now }));
      return { ok: true as const, featureRequestId: existing.featureRequestId, existed: true as const, updatedAtMs: now };
    }

    const featureRequestId = `fr_${newId()}`;
    yield* tryPromise(() =>
      ctx.db.insert("autopilotFeatureRequests", {
        featureRequestId,
        threadId: args.threadId,
        runId,
        messageId,
        userText,
        capabilityKey,
        capabilityLabel,
        summary,
        confidence,
        notifyWhenAvailable: Boolean(args.notifyWhenAvailable),
        source,
        status: "open",
        createdAtMs: now,
        updatedAtMs: now,
      }),
    );

    yield* tryPromise(() => ctx.db.patch(thread._id, { updatedAtMs: now }));
    return { ok: true as const, featureRequestId, existed: false as const, updatedAtMs: now };
  });

export const recordFeatureRequest = effectMutation({
  args: {
    threadId: v.string(),
    anonKey: v.optional(v.string()),
    runId: v.string(),
    messageId: v.string(),
    userText: v.string(),
    capabilityKey: v.string(),
    capabilityLabel: v.string(),
    summary: v.string(),
    confidence: v.number(),
    notifyWhenAvailable: v.boolean(),
    source: v.object({
      signatureId: v.string(),
      compiled_id: v.optional(v.string()),
      receiptId: v.optional(v.string()),
      modelId: v.optional(v.string()),
      provider: v.optional(v.string()),
      route: v.optional(v.string()),
      fallbackModelId: v.optional(v.string()),
    }),
  },
  returns: v.object({
    ok: v.boolean(),
    featureRequestId: v.string(),
    existed: v.boolean(),
    updatedAtMs: v.number(),
  }),
  handler: recordFeatureRequestImpl,
});

export const listFeatureRequestsForThreadImpl = (
  ctx: EffectQueryCtx,
  args: { readonly threadId: string; readonly anonKey?: string | undefined; readonly limit?: number | undefined },
) =>
  Effect.gen(function* () {
    yield* assertThreadAccess(ctx, args);

    const limit =
      typeof args.limit === "number" && Number.isFinite(args.limit)
        ? Math.max(1, Math.min(100, Math.floor(args.limit)))
        : 30;

    const rows = yield* tryPromise(() =>
      ctx.db
        .query("autopilotFeatureRequests")
        .withIndex("by_threadId_createdAtMs", (q) => q.eq("threadId", args.threadId))
        .order("desc")
        .take(limit),
    );

    return {
      ok: true as const,
      featureRequests: rows.map((row) => ({
        featureRequestId: row.featureRequestId,
        threadId: row.threadId,
        runId: row.runId,
        messageId: row.messageId,
        userText: row.userText,
        capabilityKey: row.capabilityKey,
        capabilityLabel: row.capabilityLabel,
        summary: row.summary,
        confidence: row.confidence,
        notifyWhenAvailable: row.notifyWhenAvailable,
        status: row.status,
        source: row.source,
        createdAtMs: row.createdAtMs,
        updatedAtMs: row.updatedAtMs,
      })),
    };
  });

export const listFeatureRequestsForThread = effectQuery({
  args: {
    threadId: v.string(),
    anonKey: v.optional(v.string()),
    limit: v.optional(v.number()),
  },
  returns: v.object({
    ok: v.boolean(),
    featureRequests: v.array(v.any()),
  }),
  handler: listFeatureRequestsForThreadImpl,
});

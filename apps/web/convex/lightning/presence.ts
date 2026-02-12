import { v } from "convex/values";
import { Effect } from "effect";

import type { EffectMutationCtx, EffectQueryCtx } from "../effect/ctx";
import { effectMutation, effectQuery } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

import { getSubject } from "../autopilot/access";

const nowMs = () => Date.now();

const normalizeOptionalString = (value: string | undefined, maxLen: number): string | undefined => {
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  if (!trimmed) return undefined;
  return trimmed.slice(0, maxLen);
};

const normalizeDeviceId = (value: string): string | null => {
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  if (!trimmed) return null;
  // Device ids are not secrets, but keep them bounded and printable.
  return trimmed.slice(0, 128);
};

const normalizeCapabilities = (
  raw: ReadonlyArray<string> | undefined,
): Array<string> | undefined => {
  if (!raw) return undefined;
  const caps = raw
    .filter((c) => typeof c === "string")
    .map((c) => c.trim().slice(0, 120))
    .filter((c) => c.length > 0)
    .slice(0, 64);
  return caps.length > 0 ? caps : undefined;
};

const requireSubject = (ctx: EffectQueryCtx | EffectMutationCtx): Effect.Effect<string, Error> =>
  getSubject(ctx).pipe(
    Effect.flatMap((subject) => (subject ? Effect.succeed(subject) : Effect.fail(new Error("unauthorized")))),
  );

const presenceValidator = v.object({
  ownerId: v.string(),
  deviceId: v.string(),
  lastSeenAtMs: v.number(),
  version: v.optional(v.string()),
  capabilities: v.optional(v.array(v.string())),
  createdAtMs: v.number(),
  updatedAtMs: v.number(),
});

type PresenceDoc = {
  readonly _id: any;
  readonly ownerId: string;
  readonly deviceId: string;
  readonly lastSeenAtMs: number;
  readonly version?: string;
  readonly capabilities?: Array<string>;
  readonly createdAtMs: number;
  readonly updatedAtMs: number;
};

const toPresence = (row: PresenceDoc) => ({
  ownerId: row.ownerId,
  deviceId: row.deviceId,
  lastSeenAtMs: row.lastSeenAtMs,
  version: row.version,
  capabilities: row.capabilities,
  createdAtMs: row.createdAtMs,
  updatedAtMs: row.updatedAtMs,
});

const loadPresenceByOwnerDevice = (
  ctx: EffectQueryCtx | EffectMutationCtx,
  input: { readonly ownerId: string; readonly deviceId: string },
): Effect.Effect<PresenceDoc | null, Error> =>
  tryPromise(() =>
    ctx.db
      .query("lightningExecutorPresence")
      .withIndex("by_ownerId_deviceId", (q) => q.eq("ownerId", input.ownerId).eq("deviceId", input.deviceId))
      .unique(),
  ).pipe(Effect.mapError((error) => (error instanceof Error ? error : new Error(String(error))))) as Effect.Effect<
    PresenceDoc | null,
    Error
  >;

export const upsertExecutorPresenceImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly deviceId: string;
    readonly version?: string | undefined;
    readonly capabilities?: Array<string> | undefined;
  },
) =>
  Effect.gen(function* () {
    const ownerId = yield* requireSubject(ctx);
    const deviceId = normalizeDeviceId(args.deviceId);
    if (!deviceId) return yield* Effect.fail(new Error("invalid_input"));

    const version = normalizeOptionalString(args.version, 200);
    const capabilities = normalizeCapabilities(args.capabilities);
    const now = nowMs();

    const existing = yield* loadPresenceByOwnerDevice(ctx, { ownerId, deviceId });
    if (existing) {
      yield* tryPromise(() =>
        ctx.db.patch(existing._id, {
          lastSeenAtMs: now,
          version,
          capabilities,
          updatedAtMs: now,
        }),
      );
    } else {
      yield* tryPromise(() =>
        ctx.db.insert("lightningExecutorPresence", {
          ownerId,
          deviceId,
          lastSeenAtMs: now,
          version,
          capabilities,
          createdAtMs: now,
          updatedAtMs: now,
        }),
      );
    }

    const refreshed = yield* loadPresenceByOwnerDevice(ctx, { ownerId, deviceId });
    if (!refreshed) return yield* Effect.fail(new Error("presence_missing_after_upsert"));

    return { ok: true, presence: toPresence(refreshed) };
  });

export const upsertExecutorPresence = effectMutation({
  args: {
    deviceId: v.string(),
    version: v.optional(v.string()),
    capabilities: v.optional(v.array(v.string())),
  },
  returns: v.object({
    ok: v.boolean(),
    presence: presenceValidator,
  }),
  handler: upsertExecutorPresenceImpl,
});

export const getLatestExecutorPresenceImpl = (
  ctx: EffectQueryCtx,
  args: { readonly deviceId?: string | undefined },
) =>
  Effect.gen(function* () {
    const ownerId = yield* requireSubject(ctx);
    const deviceId = normalizeOptionalString(args.deviceId, 128);

    const row: PresenceDoc | null = yield* (deviceId
      ? loadPresenceByOwnerDevice(ctx, { ownerId, deviceId })
      : (tryPromise(() =>
          ctx.db
            .query("lightningExecutorPresence")
            .withIndex("by_ownerId_updatedAtMs", (q) => q.eq("ownerId", ownerId))
            .order("desc")
            .first(),
        ).pipe(Effect.mapError((error) => (error instanceof Error ? error : new Error(String(error))))) as Effect.Effect<
          PresenceDoc | null,
          Error
        >));

    return {
      ok: true,
      presence: row ? toPresence(row) : null,
    };
  });

export const getLatestExecutorPresence = effectQuery({
  args: {
    deviceId: v.optional(v.string()),
  },
  returns: v.object({
    ok: v.boolean(),
    presence: v.union(v.null(), presenceValidator),
  }),
  handler: getLatestExecutorPresenceImpl,
});

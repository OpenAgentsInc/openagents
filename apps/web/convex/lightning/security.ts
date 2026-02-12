import { v } from "convex/values";
import { Effect } from "effect";

import type { EffectMutationCtx, EffectQueryCtx } from "../effect/ctx";
import { effectMutation, effectQuery } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

import { getSubject } from "../autopilot/access";

const nowMs = () => Date.now();
const GLOBAL_STATE_ID = "global";

const credentialRoleValidator = v.union(
  v.literal("gateway_invoice"),
  v.literal("settlement_read"),
  v.literal("operator_admin"),
);
const credentialStatusValidator = v.union(v.literal("active"), v.literal("rotating"), v.literal("revoked"));
const denyReasonCodeValidator = v.union(v.literal("global_pause_active"), v.literal("owner_kill_switch_active"));

const globalControlValidator = v.object({
  stateId: v.string(),
  globalPause: v.boolean(),
  denyReasonCode: v.optional(v.literal("global_pause_active")),
  denyReason: v.optional(v.string()),
  updatedBy: v.optional(v.string()),
  updatedAtMs: v.number(),
});

const ownerControlValidator = v.object({
  ownerId: v.string(),
  killSwitch: v.boolean(),
  denyReasonCode: v.optional(v.literal("owner_kill_switch_active")),
  denyReason: v.optional(v.string()),
  updatedBy: v.optional(v.string()),
  updatedAtMs: v.number(),
});

const credentialRoleStateValidator = v.object({
  role: credentialRoleValidator,
  status: credentialStatusValidator,
  version: v.number(),
  fingerprint: v.optional(v.string()),
  note: v.optional(v.string()),
  updatedAtMs: v.number(),
  lastRotatedAtMs: v.optional(v.number()),
  revokedAtMs: v.optional(v.number()),
});

const securityGateValidator = v.object({
  allowed: v.boolean(),
  denyReasonCode: v.optional(denyReasonCodeValidator),
  denyReason: v.optional(v.string()),
});

type CredentialRole = "gateway_invoice" | "settlement_read" | "operator_admin";
type CredentialStatus = "active" | "rotating" | "revoked";
type SecurityDenyReasonCode = "global_pause_active" | "owner_kill_switch_active";

type GlobalControlDoc = {
  readonly _id: any;
  readonly stateId: string;
  readonly globalPause: boolean;
  readonly denyReasonCode?: "global_pause_active";
  readonly denyReason?: string;
  readonly updatedBy?: string;
  readonly updatedAtMs: number;
};

type OwnerControlDoc = {
  readonly _id: any;
  readonly ownerId: string;
  readonly killSwitch: boolean;
  readonly denyReasonCode?: "owner_kill_switch_active";
  readonly denyReason?: string;
  readonly updatedBy?: string;
  readonly updatedAtMs: number;
};

type CredentialRoleDoc = {
  readonly _id: any;
  readonly role: CredentialRole;
  readonly status: CredentialStatus;
  readonly version: number;
  readonly fingerprint?: string;
  readonly note?: string;
  readonly updatedAtMs: number;
  readonly lastRotatedAtMs?: number;
  readonly revokedAtMs?: number;
};

export type OwnerSecurityGateDecision =
  | Readonly<{ allowed: true }>
  | Readonly<{
      allowed: false;
      denyReasonCode: SecurityDenyReasonCode;
      denyReason: string;
    }>;

const assertOpsSecret = (secret: string): Effect.Effect<void, Error> =>
  Effect.sync(() => {
    const expected = process.env.OA_LIGHTNING_OPS_SECRET;
    if (!expected) throw new Error("ops_disabled");
    if (secret !== expected) throw new Error("forbidden");
  });

const normalizeOptionalString = (value: unknown, maxLen: number): string | undefined => {
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  if (!trimmed) return undefined;
  return trimmed.slice(0, maxLen);
};

const requireSubject = (ctx: EffectQueryCtx | EffectMutationCtx): Effect.Effect<string, Error> =>
  getSubject(ctx).pipe(
    Effect.flatMap((subject) => (subject ? Effect.succeed(subject) : Effect.fail(new Error("unauthorized")))),
  );

const loadGlobalControl = (
  ctx: EffectQueryCtx | EffectMutationCtx,
): Effect.Effect<GlobalControlDoc | null, Error> =>
  tryPromise(() =>
    ctx.db
      .query("l402SecurityGlobal")
      .withIndex("by_stateId", (q) => q.eq("stateId", GLOBAL_STATE_ID))
      .unique(),
  ) as Effect.Effect<GlobalControlDoc | null, Error>;

const loadOwnerControl = (
  ctx: EffectQueryCtx | EffectMutationCtx,
  ownerId: string,
): Effect.Effect<OwnerControlDoc | null, Error> =>
  tryPromise(() =>
    ctx.db
      .query("l402OwnerSecurityControls")
      .withIndex("by_ownerId", (q) => q.eq("ownerId", ownerId))
      .unique(),
  ) as Effect.Effect<OwnerControlDoc | null, Error>;

const listOwnerControls = (ctx: EffectQueryCtx): Effect.Effect<ReadonlyArray<OwnerControlDoc>, Error> =>
  tryPromise(() =>
    ctx.db
      .query("l402OwnerSecurityControls")
      .withIndex("by_killSwitch_updatedAtMs", (q) => q.eq("killSwitch", true))
      .order("desc")
      .collect(),
  ) as Effect.Effect<ReadonlyArray<OwnerControlDoc>, Error>;

const listCredentialRoles = (
  ctx: EffectQueryCtx | EffectMutationCtx,
): Effect.Effect<ReadonlyArray<CredentialRoleDoc>, Error> =>
  tryPromise(() => ctx.db.query("l402CredentialRoles").collect()).pipe(
    Effect.map((rows) =>
      (rows as ReadonlyArray<CredentialRoleDoc>).slice().sort((a, b) => a.role.localeCompare(b.role)),
    ),
  );

const toGlobalControl = (row: GlobalControlDoc | null) =>
  row
    ? {
        stateId: row.stateId,
        globalPause: row.globalPause,
        denyReasonCode: row.denyReasonCode,
        denyReason: row.denyReason,
        updatedBy: row.updatedBy,
        updatedAtMs: row.updatedAtMs,
      }
    : {
        stateId: GLOBAL_STATE_ID,
        globalPause: false as const,
        updatedAtMs: 0,
      };

const toOwnerControl = (row: OwnerControlDoc) => ({
  ownerId: row.ownerId,
  killSwitch: row.killSwitch,
  denyReasonCode: row.denyReasonCode,
  denyReason: row.denyReason,
  updatedBy: row.updatedBy,
  updatedAtMs: row.updatedAtMs,
});

const toCredentialRole = (row: CredentialRoleDoc) => ({
  role: row.role,
  status: row.status,
  version: row.version,
  fingerprint: row.fingerprint,
  note: row.note,
  updatedAtMs: row.updatedAtMs,
  lastRotatedAtMs: row.lastRotatedAtMs,
  revokedAtMs: row.revokedAtMs,
});

export const evaluateOwnerSecurityGate = (
  ctx: EffectQueryCtx | EffectMutationCtx,
  ownerId: string,
): Effect.Effect<OwnerSecurityGateDecision, Error> =>
  Effect.gen(function* () {
    const global = yield* loadGlobalControl(ctx);
    if (global?.globalPause) {
      return {
        allowed: false as const,
        denyReasonCode: "global_pause_active" as const,
        denyReason: global.denyReason ?? "Global paywall pause is active",
      };
    }

    const ownerControl = yield* loadOwnerControl(ctx, ownerId);
    if (ownerControl?.killSwitch) {
      return {
        allowed: false as const,
        denyReasonCode: "owner_kill_switch_active" as const,
        denyReason: ownerControl.denyReason ?? "Owner kill switch is active",
      };
    }

    return { allowed: true as const };
  });

export const getOwnerSecurityStateImpl = (ctx: EffectQueryCtx) =>
  Effect.gen(function* () {
    const ownerId = yield* requireSubject(ctx);
    const global = yield* loadGlobalControl(ctx);
    const ownerControl = yield* loadOwnerControl(ctx, ownerId);
    const gate = yield* evaluateOwnerSecurityGate(ctx, ownerId);
    const credentialRoles = yield* listCredentialRoles(ctx);

    return {
      ok: true as const,
      ownerId,
      global: toGlobalControl(global),
      ownerControl: ownerControl ? toOwnerControl(ownerControl) : null,
      gate,
      credentialRoles: credentialRoles.map(toCredentialRole),
    };
  });

export const getOwnerSecurityState = effectQuery({
  args: {},
  returns: v.object({
    ok: v.boolean(),
    ownerId: v.string(),
    global: globalControlValidator,
    ownerControl: v.union(ownerControlValidator, v.null()),
    gate: securityGateValidator,
    credentialRoles: v.array(credentialRoleStateValidator),
  }),
  handler: getOwnerSecurityStateImpl,
});

export const getControlPlaneSecurityStateImpl = (
  ctx: EffectQueryCtx,
  args: { readonly secret: string },
) =>
  Effect.gen(function* () {
    yield* assertOpsSecret(args.secret);
    const global = yield* loadGlobalControl(ctx);
    const ownerControls = yield* listOwnerControls(ctx);
    const credentialRoles = yield* listCredentialRoles(ctx);

    return {
      ok: true as const,
      global: toGlobalControl(global),
      ownerControls: ownerControls.map(toOwnerControl),
      credentialRoles: credentialRoles.map(toCredentialRole),
    };
  });

export const getControlPlaneSecurityState = effectQuery({
  args: { secret: v.string() },
  returns: v.object({
    ok: v.boolean(),
    global: globalControlValidator,
    ownerControls: v.array(ownerControlValidator),
    credentialRoles: v.array(credentialRoleStateValidator),
  }),
  handler: getControlPlaneSecurityStateImpl,
});

export const setGlobalPauseImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly secret: string;
    readonly active: boolean;
    readonly reason?: string;
    readonly updatedBy?: string;
  },
) =>
  Effect.gen(function* () {
    yield* assertOpsSecret(args.secret);
    const now = nowMs();
    const reason = normalizeOptionalString(args.reason, 500);
    const updatedBy = normalizeOptionalString(args.updatedBy, 120);

    const existing = yield* loadGlobalControl(ctx);
    const payload = {
      stateId: GLOBAL_STATE_ID,
      globalPause: args.active,
      denyReasonCode: args.active ? ("global_pause_active" as const) : undefined,
      denyReason: args.active ? reason ?? "Global paywall pause is active" : undefined,
      updatedBy,
      updatedAtMs: now,
    };

    if (existing) {
      yield* tryPromise(() => ctx.db.patch(existing._id, payload));
    } else {
      yield* tryPromise(() => ctx.db.insert("l402SecurityGlobal", payload));
    }

    const row = yield* loadGlobalControl(ctx);
    if (!row) return yield* Effect.fail(new Error("security_state_not_found"));
    return { ok: true as const, global: toGlobalControl(row) };
  });

export const setGlobalPause = effectMutation({
  args: {
    secret: v.string(),
    active: v.boolean(),
    reason: v.optional(v.string()),
    updatedBy: v.optional(v.string()),
  },
  returns: v.object({
    ok: v.boolean(),
    global: globalControlValidator,
  }),
  handler: setGlobalPauseImpl,
});

export const setOwnerKillSwitchImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly secret: string;
    readonly ownerId: string;
    readonly active: boolean;
    readonly reason?: string;
    readonly updatedBy?: string;
  },
) =>
  Effect.gen(function* () {
    yield* assertOpsSecret(args.secret);
    const ownerId = normalizeOptionalString(args.ownerId, 160);
    if (!ownerId) return yield* Effect.fail(new Error("invalid_input"));

    const now = nowMs();
    const reason = normalizeOptionalString(args.reason, 500);
    const updatedBy = normalizeOptionalString(args.updatedBy, 120);

    const existing = yield* loadOwnerControl(ctx, ownerId);
    const payload = {
      ownerId,
      killSwitch: args.active,
      denyReasonCode: args.active ? ("owner_kill_switch_active" as const) : undefined,
      denyReason: args.active ? reason ?? "Owner kill switch is active" : undefined,
      updatedBy,
      updatedAtMs: now,
    };

    if (existing) {
      yield* tryPromise(() => ctx.db.patch(existing._id, payload));
    } else {
      yield* tryPromise(() => ctx.db.insert("l402OwnerSecurityControls", payload));
    }

    const row = yield* loadOwnerControl(ctx, ownerId);
    if (!row) return yield* Effect.fail(new Error("security_state_not_found"));
    return { ok: true as const, ownerControl: toOwnerControl(row) };
  });

export const setOwnerKillSwitch = effectMutation({
  args: {
    secret: v.string(),
    ownerId: v.string(),
    active: v.boolean(),
    reason: v.optional(v.string()),
    updatedBy: v.optional(v.string()),
  },
  returns: v.object({
    ok: v.boolean(),
    ownerControl: ownerControlValidator,
  }),
  handler: setOwnerKillSwitchImpl,
});

const upsertCredentialRole = (
  ctx: EffectMutationCtx,
  input: {
    readonly role: CredentialRole;
    readonly status: CredentialStatus;
    readonly fingerprint?: string;
    readonly note?: string;
    readonly versionResolver: (existing: CredentialRoleDoc | null) => number;
    readonly rotatedAtMs?: number;
    readonly revokedAtMs?: number;
  },
) =>
  Effect.gen(function* () {
    const now = nowMs();
    const existing = (yield* tryPromise(() =>
      ctx.db
        .query("l402CredentialRoles")
        .withIndex("by_role", (q) => q.eq("role", input.role))
        .unique(),
    )) as CredentialRoleDoc | null;
    const version = input.versionResolver(existing);

    const payload = {
      role: input.role,
      status: input.status,
      version,
      fingerprint: input.fingerprint,
      note: input.note,
      updatedAtMs: now,
      lastRotatedAtMs: input.rotatedAtMs,
      revokedAtMs: input.revokedAtMs,
    };

    if (existing) {
      yield* tryPromise(() => ctx.db.patch(existing._id, payload));
    } else {
      yield* tryPromise(() => ctx.db.insert("l402CredentialRoles", payload));
    }

    const row = (yield* tryPromise(() =>
      ctx.db
        .query("l402CredentialRoles")
        .withIndex("by_role", (q) => q.eq("role", input.role))
        .unique(),
    )) as CredentialRoleDoc | null;
    if (!row) return yield* Effect.fail(new Error("credential_role_not_found"));
    return toCredentialRole(row);
  });

export const rotateCredentialRoleImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly secret: string;
    readonly role: CredentialRole;
    readonly fingerprint?: string;
    readonly note?: string;
  },
) =>
  Effect.gen(function* () {
    yield* assertOpsSecret(args.secret);
    const fingerprint = normalizeOptionalString(args.fingerprint, 256);
    const note = normalizeOptionalString(args.note, 500);
    const rotatedAtMs = nowMs();
    const role = yield* upsertCredentialRole(ctx, {
      role: args.role,
      status: "rotating",
      fingerprint,
      note,
      versionResolver: (existing) => Math.max(1, (existing?.version ?? 0) + 1),
      rotatedAtMs,
    });
    return { ok: true as const, role };
  });

export const rotateCredentialRole = effectMutation({
  args: {
    secret: v.string(),
    role: credentialRoleValidator,
    fingerprint: v.optional(v.string()),
    note: v.optional(v.string()),
  },
  returns: v.object({
    ok: v.boolean(),
    role: credentialRoleStateValidator,
  }),
  handler: rotateCredentialRoleImpl,
});

export const activateCredentialRoleImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly secret: string;
    readonly role: CredentialRole;
    readonly fingerprint?: string;
    readonly note?: string;
  },
) =>
  Effect.gen(function* () {
    yield* assertOpsSecret(args.secret);
    const fingerprint = normalizeOptionalString(args.fingerprint, 256);
    const note = normalizeOptionalString(args.note, 500);
    const role = yield* upsertCredentialRole(ctx, {
      role: args.role,
      status: "active",
      fingerprint,
      note,
      versionResolver: (existing) => {
        if (!existing) return 1;
        if (existing.status === "rotating") return existing.version;
        return Math.max(1, existing.version + 1);
      },
      rotatedAtMs: nowMs(),
    });
    return { ok: true as const, role };
  });

export const activateCredentialRole = effectMutation({
  args: {
    secret: v.string(),
    role: credentialRoleValidator,
    fingerprint: v.optional(v.string()),
    note: v.optional(v.string()),
  },
  returns: v.object({
    ok: v.boolean(),
    role: credentialRoleStateValidator,
  }),
  handler: activateCredentialRoleImpl,
});

export const revokeCredentialRoleImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly secret: string;
    readonly role: CredentialRole;
    readonly note?: string;
  },
) =>
  Effect.gen(function* () {
    yield* assertOpsSecret(args.secret);
    const note = normalizeOptionalString(args.note, 500);
    const revokedAtMs = nowMs();
    const role = yield* upsertCredentialRole(ctx, {
      role: args.role,
      status: "revoked",
      note,
      versionResolver: (existing) => Math.max(1, existing?.version ?? 1),
      revokedAtMs,
    });
    return { ok: true as const, role };
  });

export const revokeCredentialRole = effectMutation({
  args: {
    secret: v.string(),
    role: credentialRoleValidator,
    note: v.optional(v.string()),
  },
  returns: v.object({
    ok: v.boolean(),
    role: credentialRoleStateValidator,
  }),
  handler: revokeCredentialRoleImpl,
});

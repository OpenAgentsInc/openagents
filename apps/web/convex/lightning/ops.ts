import { v } from "convex/values";
import { Effect } from "effect";

import type { EffectMutationCtx, EffectQueryCtx } from "../effect/ctx";
import { effectMutation, effectQuery } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

const nowMs = () => Date.now();
const newId = () => crypto.randomUUID();

const paywallStatusValidator = v.union(v.literal("active"), v.literal("paused"), v.literal("archived"));
const deploymentStatusValidator = v.union(
  v.literal("pending"),
  v.literal("applied"),
  v.literal("failed"),
  v.literal("rolled_back"),
);

const paywallPolicyValidator = v.object({
  paywallId: v.string(),
  ownerId: v.string(),
  pricingMode: v.literal("fixed"),
  fixedAmountMsats: v.number(),
  maxPerRequestMsats: v.optional(v.number()),
  allowedHosts: v.optional(v.array(v.string())),
  blockedHosts: v.optional(v.array(v.string())),
  quotaPerMinute: v.optional(v.number()),
  quotaPerDay: v.optional(v.number()),
  killSwitch: v.boolean(),
  createdAtMs: v.number(),
  updatedAtMs: v.number(),
});

const paywallRouteValidator = v.object({
  routeId: v.string(),
  paywallId: v.string(),
  ownerId: v.string(),
  hostPattern: v.string(),
  pathPattern: v.string(),
  upstreamUrl: v.string(),
  protocol: v.union(v.literal("http"), v.literal("https")),
  timeoutMs: v.number(),
  priority: v.number(),
  createdAtMs: v.number(),
  updatedAtMs: v.number(),
});

const paywallControlPlaneValidator = v.object({
  paywallId: v.string(),
  ownerId: v.string(),
  name: v.string(),
  description: v.optional(v.string()),
  status: paywallStatusValidator,
  requestId: v.optional(v.string()),
  metadata: v.optional(v.any()),
  createdAtMs: v.number(),
  updatedAtMs: v.number(),
  policy: paywallPolicyValidator,
  routes: v.array(paywallRouteValidator),
});

const deploymentValidator = v.object({
  deploymentId: v.string(),
  paywallId: v.optional(v.string()),
  ownerId: v.optional(v.string()),
  configHash: v.string(),
  imageDigest: v.optional(v.string()),
  status: deploymentStatusValidator,
  diagnostics: v.optional(v.any()),
  appliedAtMs: v.optional(v.number()),
  rolledBackFrom: v.optional(v.string()),
  createdAtMs: v.number(),
  updatedAtMs: v.number(),
});

const gatewayEventValidator = v.object({
  eventId: v.string(),
  paywallId: v.string(),
  ownerId: v.string(),
  eventType: v.string(),
  level: v.union(v.literal("info"), v.literal("warn"), v.literal("error")),
  requestId: v.optional(v.string()),
  metadata: v.optional(v.any()),
  createdAtMs: v.number(),
});

const assertOpsSecret = (secret: string): Effect.Effect<void, Error> =>
  Effect.sync(() => {
    const expected = process.env.OA_LIGHTNING_OPS_SECRET;
    if (!expected) throw new Error("ops_disabled");
    if (secret !== expected) throw new Error("forbidden");
  });

type PaywallDoc = {
  readonly _id: any;
  readonly paywallId: string;
  readonly ownerId: string;
  readonly name: string;
  readonly description?: string;
  readonly status: "active" | "paused" | "archived";
  readonly requestId?: string;
  readonly metadata?: unknown;
  readonly createdAtMs: number;
  readonly updatedAtMs: number;
};

type PaywallPolicyDoc = {
  readonly paywallId: string;
  readonly ownerId: string;
  readonly pricingMode: "fixed";
  readonly fixedAmountMsats: number;
  readonly maxPerRequestMsats?: number;
  readonly allowedHosts?: Array<string>;
  readonly blockedHosts?: Array<string>;
  readonly quotaPerMinute?: number;
  readonly quotaPerDay?: number;
  readonly killSwitch: boolean;
  readonly createdAtMs: number;
  readonly updatedAtMs: number;
};

type PaywallRouteDoc = {
  readonly routeId: string;
  readonly paywallId: string;
  readonly ownerId: string;
  readonly hostPattern: string;
  readonly pathPattern: string;
  readonly upstreamUrl: string;
  readonly protocol: "http" | "https";
  readonly timeoutMs: number;
  readonly priority: number;
  readonly createdAtMs: number;
  readonly updatedAtMs: number;
};

const loadPolicy = (
  ctx: EffectQueryCtx,
  paywallId: string,
): Effect.Effect<PaywallPolicyDoc | null, Error> =>
  tryPromise(() =>
    ctx.db
      .query("l402PaywallPolicies")
      .withIndex("by_paywallId", (q) => q.eq("paywallId", paywallId))
      .unique(),
  ) as Effect.Effect<PaywallPolicyDoc | null, Error>;

const loadRoutes = (
  ctx: EffectQueryCtx,
  paywallId: string,
): Effect.Effect<ReadonlyArray<PaywallRouteDoc>, Error> =>
  tryPromise(() =>
    ctx.db
      .query("l402PaywallRoutes")
      .withIndex("by_paywallId_priority", (q) => q.eq("paywallId", paywallId))
      .order("asc")
      .collect(),
  ) as Effect.Effect<ReadonlyArray<PaywallRouteDoc>, Error>;

export const listPaywallControlPlaneStateImpl = (
  ctx: EffectQueryCtx,
  args: {
    readonly secret: string;
    readonly statuses?: ReadonlyArray<"active" | "paused" | "archived">;
  },
) =>
  Effect.gen(function* () {
    yield* assertOpsSecret(args.secret);

    const rows = (yield* tryPromise(() => ctx.db.query("l402Paywalls").collect())) as ReadonlyArray<PaywallDoc>;

    const statuses =
      args.statuses && args.statuses.length > 0 ? new Set(args.statuses) : new Set(["active", "paused"] as const);

    const filtered = rows
      .filter((row) => statuses.has(row.status))
      .sort((a, b) => a.paywallId.localeCompare(b.paywallId));

    const paywalls = yield* Effect.forEach(filtered, (paywall) =>
      Effect.gen(function* () {
        const policy = yield* loadPolicy(ctx, paywall.paywallId);
        if (!policy) return yield* Effect.fail(new Error("policy_missing"));
        const routes = yield* loadRoutes(ctx, paywall.paywallId);
        return {
          paywallId: paywall.paywallId,
          ownerId: paywall.ownerId,
          name: paywall.name,
          description: paywall.description,
          status: paywall.status,
          requestId: paywall.requestId,
          metadata: paywall.metadata,
          createdAtMs: paywall.createdAtMs,
          updatedAtMs: paywall.updatedAtMs,
          policy: {
            paywallId: policy.paywallId,
            ownerId: policy.ownerId,
            pricingMode: policy.pricingMode,
            fixedAmountMsats: policy.fixedAmountMsats,
            maxPerRequestMsats: policy.maxPerRequestMsats,
            allowedHosts: policy.allowedHosts,
            blockedHosts: policy.blockedHosts,
            quotaPerMinute: policy.quotaPerMinute,
            quotaPerDay: policy.quotaPerDay,
            killSwitch: policy.killSwitch,
            createdAtMs: policy.createdAtMs,
            updatedAtMs: policy.updatedAtMs,
          },
          routes: routes.map((route) => ({
            routeId: route.routeId,
            paywallId: route.paywallId,
            ownerId: route.ownerId,
            hostPattern: route.hostPattern,
            pathPattern: route.pathPattern,
            upstreamUrl: route.upstreamUrl,
            protocol: route.protocol,
            timeoutMs: route.timeoutMs,
            priority: route.priority,
            createdAtMs: route.createdAtMs,
            updatedAtMs: route.updatedAtMs,
          })),
        };
      }),
    );

    return { ok: true as const, paywalls };
  });

export const listPaywallControlPlaneState = effectQuery({
  args: {
    secret: v.string(),
    statuses: v.optional(v.array(paywallStatusValidator)),
  },
  returns: v.object({
    ok: v.boolean(),
    paywalls: v.array(paywallControlPlaneValidator),
  }),
  handler: listPaywallControlPlaneStateImpl,
});

const normalizeOptionalString = (value: unknown, maxLen: number): string | undefined => {
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  if (!trimmed) return undefined;
  return trimmed.slice(0, maxLen);
};

type DeploymentDoc = {
  readonly _id: any;
  readonly deploymentId: string;
  readonly paywallId?: string;
  readonly ownerId?: string;
  readonly configHash: string;
  readonly imageDigest?: string;
  readonly status: "pending" | "applied" | "failed" | "rolled_back";
  readonly diagnostics?: unknown;
  readonly appliedAtMs?: number;
  readonly rolledBackFrom?: string;
  readonly createdAtMs: number;
  readonly updatedAtMs: number;
};

type GatewayEventDoc = {
  readonly _id: any;
  readonly eventId: string;
  readonly paywallId: string;
  readonly ownerId: string;
  readonly eventType: string;
  readonly level: "info" | "warn" | "error";
  readonly requestId?: string;
  readonly metadata?: unknown;
  readonly createdAtMs: number;
};

export const recordGatewayCompileIntentImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly secret: string;
    readonly deploymentId?: string;
    readonly paywallId?: string;
    readonly ownerId?: string;
    readonly configHash: string;
    readonly imageDigest?: string;
    readonly status: "pending" | "applied" | "failed" | "rolled_back";
    readonly diagnostics?: unknown;
    readonly requestId?: string;
    readonly metadata?: unknown;
    readonly rolledBackFrom?: string;
    readonly appliedAtMs?: number;
  },
) =>
  Effect.gen(function* () {
    yield* assertOpsSecret(args.secret);

    const configHash = normalizeOptionalString(args.configHash, 256);
    if (!configHash) return yield* Effect.fail(new Error("invalid_input"));

    const deploymentId = normalizeOptionalString(args.deploymentId, 128) ?? `dep_${newId()}`;
    const paywallId = normalizeOptionalString(args.paywallId, 128);
    const ownerId = normalizeOptionalString(args.ownerId, 128);
    const imageDigest = normalizeOptionalString(args.imageDigest, 256);
    const rolledBackFrom = normalizeOptionalString(args.rolledBackFrom, 256);
    const requestId = normalizeOptionalString(args.requestId, 160);

    const existing = (yield* tryPromise(() =>
      ctx.db
        .query("l402GatewayDeployments")
        .withIndex("by_deploymentId", (q) => q.eq("deploymentId", deploymentId))
        .unique(),
    )) as DeploymentDoc | null;

    const now = nowMs();
    const diagnostics = {
      diagnostics: args.diagnostics,
      metadata: args.metadata,
      requestId,
      recordedAtMs: now,
    };

    const payload = {
      deploymentId,
      paywallId,
      ownerId,
      configHash,
      imageDigest,
      status: args.status,
      diagnostics,
      appliedAtMs: args.appliedAtMs,
      rolledBackFrom,
      updatedAtMs: now,
    } as const;

    if (existing) {
      yield* tryPromise(() => ctx.db.patch(existing._id, payload));
    } else {
      yield* tryPromise(() =>
        ctx.db.insert("l402GatewayDeployments", {
          ...payload,
          createdAtMs: now,
        }),
      );
    }

    if (paywallId && ownerId) {
      yield* tryPromise(() =>
        ctx.db.insert("l402GatewayEvents", {
          eventId: `evt_${newId()}`,
          paywallId,
          ownerId,
          eventType: `gateway_compile_intent_${args.status}`,
          level: args.status === "failed" ? "error" : "info",
          requestId,
          metadata: diagnostics,
          createdAtMs: now,
        }),
      );
    }

    const row = (yield* tryPromise(() =>
      ctx.db
        .query("l402GatewayDeployments")
        .withIndex("by_deploymentId", (q) => q.eq("deploymentId", deploymentId))
        .unique(),
    )) as DeploymentDoc | null;

    if (!row) return yield* Effect.fail(new Error("deployment_not_found"));

    return {
      ok: true as const,
      deployment: {
        deploymentId: row.deploymentId,
        paywallId: row.paywallId,
        ownerId: row.ownerId,
        configHash: row.configHash,
        imageDigest: row.imageDigest,
        status: row.status,
        diagnostics: row.diagnostics,
        appliedAtMs: row.appliedAtMs,
        rolledBackFrom: row.rolledBackFrom,
        createdAtMs: row.createdAtMs,
        updatedAtMs: row.updatedAtMs,
      },
    };
  });

export const recordGatewayCompileIntent = effectMutation({
  args: {
    secret: v.string(),
    deploymentId: v.optional(v.string()),
    paywallId: v.optional(v.string()),
    ownerId: v.optional(v.string()),
    configHash: v.string(),
    imageDigest: v.optional(v.string()),
    status: deploymentStatusValidator,
    diagnostics: v.optional(v.any()),
    requestId: v.optional(v.string()),
    metadata: v.optional(v.any()),
    rolledBackFrom: v.optional(v.string()),
    appliedAtMs: v.optional(v.number()),
  },
  returns: v.object({
    ok: v.boolean(),
    deployment: deploymentValidator,
  }),
  handler: recordGatewayCompileIntentImpl,
});

export const recordGatewayDeploymentEventImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly secret: string;
    readonly paywallId: string;
    readonly ownerId: string;
    readonly eventType: string;
    readonly level: "info" | "warn" | "error";
    readonly requestId?: string;
    readonly deploymentId?: string;
    readonly configHash?: string;
    readonly executionPath?: "hosted-node" | "local-node";
    readonly metadata?: unknown;
  },
) =>
  Effect.gen(function* () {
    yield* assertOpsSecret(args.secret);

    const paywallId = normalizeOptionalString(args.paywallId, 128);
    const ownerId = normalizeOptionalString(args.ownerId, 128);
    const eventType = normalizeOptionalString(args.eventType, 160);
    if (!paywallId || !ownerId || !eventType) return yield* Effect.fail(new Error("invalid_input"));

    const requestId = normalizeOptionalString(args.requestId, 160);
    const deploymentId = normalizeOptionalString(args.deploymentId, 128);
    const configHash = normalizeOptionalString(args.configHash, 256);
    const executionPath = args.executionPath ?? "hosted-node";
    const now = nowMs();
    const eventId = `evt_${newId()}`;

    yield* tryPromise(() =>
      ctx.db.insert("l402GatewayEvents", {
        eventId,
        paywallId,
        ownerId,
        eventType,
        level: args.level,
        requestId,
        metadata: {
          executionPath,
          deploymentId,
          configHash,
          details: args.metadata,
        },
        createdAtMs: now,
      }),
    );

    const row = (yield* tryPromise(() =>
      ctx.db
        .query("l402GatewayEvents")
        .withIndex("by_eventId", (q) => q.eq("eventId", eventId))
        .unique(),
    )) as GatewayEventDoc | null;

    if (!row) return yield* Effect.fail(new Error("event_not_found"));

    return {
      ok: true as const,
      event: {
        eventId: row.eventId,
        paywallId: row.paywallId,
        ownerId: row.ownerId,
        eventType: row.eventType,
        level: row.level,
        requestId: row.requestId,
        metadata: row.metadata,
        createdAtMs: row.createdAtMs,
      },
    };
  });

export const recordGatewayDeploymentEvent = effectMutation({
  args: {
    secret: v.string(),
    paywallId: v.string(),
    ownerId: v.string(),
    eventType: v.string(),
    level: v.union(v.literal("info"), v.literal("warn"), v.literal("error")),
    requestId: v.optional(v.string()),
    deploymentId: v.optional(v.string()),
    configHash: v.optional(v.string()),
    executionPath: v.optional(v.union(v.literal("hosted-node"), v.literal("local-node"))),
    metadata: v.optional(v.any()),
  },
  returns: v.object({
    ok: v.boolean(),
    event: gatewayEventValidator,
  }),
  handler: recordGatewayDeploymentEventImpl,
});

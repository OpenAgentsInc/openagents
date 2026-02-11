import { v } from "convex/values";
import { Effect } from "effect";

import type { EffectMutationCtx, EffectQueryCtx } from "../effect/ctx";
import { effectMutation, effectQuery } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

import { getSubject } from "../autopilot/access";

const nowMs = () => Date.now();
const newId = () => crypto.randomUUID();

export type L402PaywallStatus = "active" | "paused" | "archived";
export type L402PricingMode = "fixed";

const paywallStatusValidator = v.union(v.literal("active"), v.literal("paused"), v.literal("archived"));
const pricingModeValidator = v.literal("fixed");
const protocolValidator = v.union(v.literal("http"), v.literal("https"));

const paywallPolicyInputValidator = v.object({
  pricingMode: pricingModeValidator,
  fixedAmountMsats: v.number(),
  maxPerRequestMsats: v.optional(v.number()),
  allowedHosts: v.optional(v.array(v.string())),
  blockedHosts: v.optional(v.array(v.string())),
  quotaPerMinute: v.optional(v.number()),
  quotaPerDay: v.optional(v.number()),
  killSwitch: v.optional(v.boolean()),
});

const paywallRouteInputValidator = v.object({
  hostPattern: v.string(),
  pathPattern: v.string(),
  upstreamUrl: v.string(),
  protocol: v.optional(protocolValidator),
  timeoutMs: v.optional(v.number()),
  priority: v.optional(v.number()),
});

const paywallPolicyValidator = v.object({
  paywallId: v.string(),
  ownerId: v.string(),
  pricingMode: pricingModeValidator,
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
  protocol: protocolValidator,
  timeoutMs: v.number(),
  priority: v.number(),
  createdAtMs: v.number(),
  updatedAtMs: v.number(),
});

const paywallValidator = v.object({
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

type PaywallDoc = {
  readonly _id: any;
  readonly paywallId: string;
  readonly ownerId: string;
  readonly name: string;
  readonly description?: string;
  readonly status: L402PaywallStatus;
  readonly requestId?: string;
  readonly metadata?: unknown;
  readonly createdAtMs: number;
  readonly updatedAtMs: number;
};

type PaywallPolicyDoc = {
  readonly _id: any;
  readonly paywallId: string;
  readonly ownerId: string;
  readonly pricingMode: L402PricingMode;
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
  readonly _id: any;
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

type NormalizedPolicy = {
  readonly pricingMode: L402PricingMode;
  readonly fixedAmountMsats: number;
  readonly maxPerRequestMsats?: number;
  readonly allowedHosts?: Array<string>;
  readonly blockedHosts?: Array<string>;
  readonly quotaPerMinute?: number;
  readonly quotaPerDay?: number;
  readonly killSwitch: boolean;
};

type NormalizedRoute = {
  readonly hostPattern: string;
  readonly pathPattern: string;
  readonly upstreamUrl: string;
  readonly upstreamHost: string;
  readonly protocol: "http" | "https";
  readonly timeoutMs: number;
  readonly priority: number;
};

const normalizeOptionalString = (value: unknown, maxLen: number): string | undefined => {
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  if (!trimmed) return undefined;
  return trimmed.slice(0, maxLen);
};

const normalizeHost = (value: string): string | undefined => {
  const trimmed = value.trim().toLowerCase();
  if (!trimmed) return undefined;
  const noProtocol = trimmed.replace(/^https?:\/\//, "");
  const noTrailing = noProtocol.replace(/\/+$/, "");
  if (!noTrailing) return undefined;
  return noTrailing;
};

const normalizeHostList = (value: ReadonlyArray<string> | undefined): Array<string> | undefined => {
  if (!value || value.length === 0) return undefined;
  const out = [...new Set(value.map(normalizeHost).filter((v): v is string => Boolean(v)))].sort((a, b) =>
    a.localeCompare(b),
  );
  return out.length > 0 ? out : undefined;
};

const normalizePositiveInt = (value: number | undefined): number | undefined => {
  if (typeof value !== "number" || !Number.isFinite(value)) return undefined;
  const normalized = Math.floor(value);
  if (normalized <= 0) return undefined;
  return normalized;
};

const requireSubject = (ctx: EffectQueryCtx | EffectMutationCtx): Effect.Effect<string, Error> =>
  getSubject(ctx).pipe(
    Effect.flatMap((subject) => (subject ? Effect.succeed(subject) : Effect.fail(new Error("unauthorized")))),
  );

const normalizePolicy = (input: {
  readonly pricingMode: L402PricingMode;
  readonly fixedAmountMsats: number;
  readonly maxPerRequestMsats?: number;
  readonly allowedHosts?: ReadonlyArray<string>;
  readonly blockedHosts?: ReadonlyArray<string>;
  readonly quotaPerMinute?: number;
  readonly quotaPerDay?: number;
  readonly killSwitch?: boolean;
}): Effect.Effect<NormalizedPolicy, Error> =>
  Effect.gen(function* () {
    const fixedAmountMsats = Math.floor(input.fixedAmountMsats);
    if (!Number.isFinite(fixedAmountMsats) || fixedAmountMsats < 0) {
      return yield* Effect.fail(new Error("invalid_input"));
    }

    const maxPerRequestMsats =
      typeof input.maxPerRequestMsats === "number" && Number.isFinite(input.maxPerRequestMsats)
        ? Math.floor(input.maxPerRequestMsats)
        : undefined;

    if (maxPerRequestMsats !== undefined && maxPerRequestMsats < 0) {
      return yield* Effect.fail(new Error("invalid_input"));
    }

    if (maxPerRequestMsats !== undefined && fixedAmountMsats > maxPerRequestMsats) {
      return yield* Effect.fail(new Error("policy_violation"));
    }

    const allowedHosts = normalizeHostList(input.allowedHosts);
    const blockedHosts = normalizeHostList(input.blockedHosts);
    if (allowedHosts && blockedHosts) {
      const blockedSet = new Set(blockedHosts);
      if (allowedHosts.some((host) => blockedSet.has(host))) {
        return yield* Effect.fail(new Error("policy_violation"));
      }
    }

    const quotaPerMinute = normalizePositiveInt(input.quotaPerMinute);
    const quotaPerDay = normalizePositiveInt(input.quotaPerDay);
    if ((input.quotaPerMinute !== undefined && quotaPerMinute === undefined) || (input.quotaPerDay !== undefined && quotaPerDay === undefined)) {
      return yield* Effect.fail(new Error("invalid_input"));
    }

    return {
      pricingMode: "fixed",
      fixedAmountMsats,
      maxPerRequestMsats,
      allowedHosts,
      blockedHosts,
      quotaPerMinute,
      quotaPerDay,
      killSwitch: input.killSwitch === true,
    };
  });

const normalizeRoute = (
  input: {
    readonly hostPattern: string;
    readonly pathPattern: string;
    readonly upstreamUrl: string;
    readonly protocol?: "http" | "https";
    readonly timeoutMs?: number;
    readonly priority?: number;
  },
  index: number,
): Effect.Effect<NormalizedRoute, Error> =>
  Effect.gen(function* () {
    const hostPattern = normalizeOptionalString(input.hostPattern, 400);
    const pathPatternRaw = normalizeOptionalString(input.pathPattern, 400);
    const upstreamUrl = normalizeOptionalString(input.upstreamUrl, 4_000);
    if (!hostPattern || !pathPatternRaw || !upstreamUrl) {
      return yield* Effect.fail(new Error("invalid_input"));
    }

    const pathPattern = pathPatternRaw.startsWith("/") ? pathPatternRaw : `/${pathPatternRaw}`;
    const parsed = yield* Effect.try({
      try: () => new URL(upstreamUrl),
      catch: () => new Error("policy_violation"),
    });

    const parsedProtocol = parsed.protocol === "http:" ? "http" : parsed.protocol === "https:" ? "https" : null;
    if (!parsedProtocol) return yield* Effect.fail(new Error("policy_violation"));
    const protocol = input.protocol ?? parsedProtocol;
    if (protocol !== parsedProtocol) return yield* Effect.fail(new Error("policy_violation"));

    const upstreamHost = normalizeHost(parsed.host);
    if (!upstreamHost) return yield* Effect.fail(new Error("policy_violation"));

    const timeoutMs =
      typeof input.timeoutMs === "number" && Number.isFinite(input.timeoutMs) ? Math.floor(input.timeoutMs) : 15_000;
    if (timeoutMs < 100 || timeoutMs > 120_000) return yield* Effect.fail(new Error("invalid_input"));

    const priority =
      typeof input.priority === "number" && Number.isFinite(input.priority) ? Math.floor(input.priority) : index + 1;

    return {
      hostPattern,
      pathPattern,
      upstreamUrl,
      upstreamHost,
      protocol,
      timeoutMs,
      priority,
    };
  });

const assertPolicyAllowsRoutes = (
  policy: NormalizedPolicy,
  routes: ReadonlyArray<NormalizedRoute>,
): Effect.Effect<void, Error> =>
  Effect.gen(function* () {
    const keySet = new Set<string>();
    const allowedSet = policy.allowedHosts ? new Set(policy.allowedHosts) : null;
    const blockedSet = policy.blockedHosts ? new Set(policy.blockedHosts) : null;

    for (const route of routes) {
      const key = `${route.hostPattern}::${route.pathPattern}`;
      if (keySet.has(key)) return yield* Effect.fail(new Error("route_conflict"));
      keySet.add(key);

      if (allowedSet && !allowedSet.has(route.upstreamHost)) {
        return yield* Effect.fail(new Error("policy_violation"));
      }

      if (blockedSet && blockedSet.has(route.upstreamHost)) {
        return yield* Effect.fail(new Error("policy_violation"));
      }
    }
  });

const loadPaywallById = (
  ctx: EffectQueryCtx | EffectMutationCtx,
  paywallId: string,
): Effect.Effect<PaywallDoc | null, Error> =>
  tryPromise(() =>
    ctx.db
      .query("l402Paywalls")
      .withIndex("by_paywallId", (q) => q.eq("paywallId", paywallId))
      .unique(),
  ) as Effect.Effect<PaywallDoc | null, Error>;

const loadPolicyByPaywall = (
  ctx: EffectQueryCtx | EffectMutationCtx,
  paywallId: string,
): Effect.Effect<PaywallPolicyDoc | null, Error> =>
  tryPromise(() =>
    ctx.db
      .query("l402PaywallPolicies")
      .withIndex("by_paywallId", (q) => q.eq("paywallId", paywallId))
      .unique(),
  ) as Effect.Effect<PaywallPolicyDoc | null, Error>;

const loadRoutesByPaywall = (
  ctx: EffectQueryCtx | EffectMutationCtx,
  paywallId: string,
): Effect.Effect<ReadonlyArray<PaywallRouteDoc>, Error> =>
  tryPromise(() =>
    ctx.db
      .query("l402PaywallRoutes")
      .withIndex("by_paywallId_priority", (q) => q.eq("paywallId", paywallId))
      .order("asc")
      .collect(),
  ) as Effect.Effect<ReadonlyArray<PaywallRouteDoc>, Error>;

const assertPaywallAccess = (
  ctx: EffectQueryCtx | EffectMutationCtx,
  args: { readonly paywallId: string; readonly ownerId: string },
): Effect.Effect<PaywallDoc, Error> =>
  Effect.gen(function* () {
    const paywall = yield* loadPaywallById(ctx, args.paywallId);
    if (!paywall) return yield* Effect.fail(new Error("paywall_not_found"));
    if (paywall.ownerId !== args.ownerId) return yield* Effect.fail(new Error("forbidden"));
    return paywall;
  });

const assertNoRouteConflicts = (
  ctx: EffectQueryCtx | EffectMutationCtx,
  routes: ReadonlyArray<NormalizedRoute>,
  currentPaywallId?: string,
): Effect.Effect<void, Error> =>
  Effect.gen(function* () {
    for (const route of routes) {
      const rows = (yield* tryPromise(() =>
        ctx.db
          .query("l402PaywallRoutes")
          .withIndex("by_hostPattern_pathPattern", (q) =>
            q.eq("hostPattern", route.hostPattern).eq("pathPattern", route.pathPattern),
          )
          .collect(),
      )) as ReadonlyArray<PaywallRouteDoc>;

      const conflict = rows.some((row) => row.paywallId !== currentPaywallId);
      if (conflict) return yield* Effect.fail(new Error("route_conflict"));
    }
  });

const toPaywallView = (paywall: PaywallDoc, policy: PaywallPolicyDoc, routes: ReadonlyArray<PaywallRouteDoc>) => ({
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
});

const loadPaywallView = (
  ctx: EffectQueryCtx | EffectMutationCtx,
  paywall: PaywallDoc,
): Effect.Effect<ReturnType<typeof toPaywallView>, Error> =>
  Effect.gen(function* () {
    const policy = yield* loadPolicyByPaywall(ctx, paywall.paywallId);
    if (!policy) return yield* Effect.fail(new Error("policy_missing"));
    const routes = yield* loadRoutesByPaywall(ctx, paywall.paywallId);
    return toPaywallView(paywall, policy, routes);
  });

const insertGatewayEvent = (
  ctx: EffectMutationCtx,
  input: {
    readonly paywallId: string;
    readonly ownerId: string;
    readonly eventType: string;
    readonly level: "info" | "warn" | "error";
    readonly requestId?: string;
    readonly metadata?: unknown;
    readonly createdAtMs: number;
  },
): Effect.Effect<void, Error> =>
  tryPromise(() =>
    ctx.db.insert("l402GatewayEvents", {
      eventId: `evt_${newId()}`,
      paywallId: input.paywallId,
      ownerId: input.ownerId,
      eventType: input.eventType,
      level: input.level,
      requestId: input.requestId,
      metadata: input.metadata,
      createdAtMs: input.createdAtMs,
    }),
  ).pipe(Effect.asVoid);

export const createPaywallImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly name: string;
    readonly description?: string;
    readonly status?: Exclude<L402PaywallStatus, "archived">;
    readonly policy: {
      readonly pricingMode: L402PricingMode;
      readonly fixedAmountMsats: number;
      readonly maxPerRequestMsats?: number;
      readonly allowedHosts?: ReadonlyArray<string>;
      readonly blockedHosts?: ReadonlyArray<string>;
      readonly quotaPerMinute?: number;
      readonly quotaPerDay?: number;
      readonly killSwitch?: boolean;
    };
    readonly routes: ReadonlyArray<{
      readonly hostPattern: string;
      readonly pathPattern: string;
      readonly upstreamUrl: string;
      readonly protocol?: "http" | "https";
      readonly timeoutMs?: number;
      readonly priority?: number;
    }>;
    readonly requestId?: string;
    readonly metadata?: unknown;
  },
) =>
  Effect.gen(function* () {
    const ownerId = yield* requireSubject(ctx);
    const name = normalizeOptionalString(args.name, 160);
    if (!name) return yield* Effect.fail(new Error("invalid_input"));
    const description = normalizeOptionalString(args.description, 2_000);
    const requestId = normalizeOptionalString(args.requestId, 160);

    const policy = yield* normalizePolicy(args.policy);
    const routes = yield* Effect.forEach(args.routes, (route, index) => normalizeRoute(route, index));
    if (routes.length === 0) return yield* Effect.fail(new Error("invalid_input"));
    yield* assertPolicyAllowsRoutes(policy, routes);
    yield* assertNoRouteConflicts(ctx, routes);

    const requestedStatus = args.status ?? "active";
    const status: L402PaywallStatus = policy.killSwitch ? "paused" : requestedStatus;

    const now = nowMs();
    const paywallId = `pw_${newId()}`;

    yield* tryPromise(() =>
      ctx.db.insert("l402Paywalls", {
        paywallId,
        ownerId,
        name,
        description,
        status,
        requestId,
        metadata: args.metadata,
        createdAtMs: now,
        updatedAtMs: now,
      }),
    );

    yield* tryPromise(() =>
      ctx.db.insert("l402PaywallPolicies", {
        paywallId,
        ownerId,
        pricingMode: policy.pricingMode,
        fixedAmountMsats: policy.fixedAmountMsats,
        maxPerRequestMsats: policy.maxPerRequestMsats,
        allowedHosts: policy.allowedHosts,
        blockedHosts: policy.blockedHosts,
        quotaPerMinute: policy.quotaPerMinute,
        quotaPerDay: policy.quotaPerDay,
        killSwitch: policy.killSwitch,
        createdAtMs: now,
        updatedAtMs: now,
      }),
    );

    for (const route of routes) {
      yield* tryPromise(() =>
        ctx.db.insert("l402PaywallRoutes", {
          routeId: `route_${newId()}`,
          paywallId,
          ownerId,
          hostPattern: route.hostPattern,
          pathPattern: route.pathPattern,
          upstreamUrl: route.upstreamUrl,
          protocol: route.protocol,
          timeoutMs: route.timeoutMs,
          priority: route.priority,
          createdAtMs: now,
          updatedAtMs: now,
        }),
      );
    }

    yield* insertGatewayEvent(ctx, {
      paywallId,
      ownerId,
      eventType: "paywall_created",
      level: "info",
      requestId,
      metadata: args.metadata,
      createdAtMs: now,
    });

    const paywall = yield* assertPaywallAccess(ctx, { paywallId, ownerId });
    const view = yield* loadPaywallView(ctx, paywall);
    return { ok: true as const, paywall: view };
  });

export const createPaywall = effectMutation({
  args: {
    name: v.string(),
    description: v.optional(v.string()),
    status: v.optional(v.union(v.literal("active"), v.literal("paused"))),
    policy: paywallPolicyInputValidator,
    routes: v.array(paywallRouteInputValidator),
    requestId: v.optional(v.string()),
    metadata: v.optional(v.any()),
  },
  returns: v.object({
    ok: v.boolean(),
    paywall: paywallValidator,
  }),
  handler: createPaywallImpl,
});

export const updatePaywallImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly paywallId: string;
    readonly name?: string;
    readonly description?: string;
    readonly policy?: {
      readonly pricingMode: L402PricingMode;
      readonly fixedAmountMsats: number;
      readonly maxPerRequestMsats?: number;
      readonly allowedHosts?: ReadonlyArray<string>;
      readonly blockedHosts?: ReadonlyArray<string>;
      readonly quotaPerMinute?: number;
      readonly quotaPerDay?: number;
      readonly killSwitch?: boolean;
    };
    readonly routes?: ReadonlyArray<{
      readonly hostPattern: string;
      readonly pathPattern: string;
      readonly upstreamUrl: string;
      readonly protocol?: "http" | "https";
      readonly timeoutMs?: number;
      readonly priority?: number;
    }>;
    readonly requestId?: string;
    readonly metadata?: unknown;
  },
) =>
  Effect.gen(function* () {
    const ownerId = yield* requireSubject(ctx);
    const paywall = yield* assertPaywallAccess(ctx, { paywallId: args.paywallId, ownerId });
    if (paywall.status === "archived") return yield* Effect.fail(new Error("invalid_transition"));

    const currentPolicy = yield* loadPolicyByPaywall(ctx, paywall.paywallId);
    if (!currentPolicy) return yield* Effect.fail(new Error("policy_missing"));
    const currentRoutes = yield* loadRoutesByPaywall(ctx, paywall.paywallId);

    const normalizedPolicy = args.policy
      ? yield* normalizePolicy(args.policy)
      : ({
          pricingMode: currentPolicy.pricingMode,
          fixedAmountMsats: currentPolicy.fixedAmountMsats,
          maxPerRequestMsats: currentPolicy.maxPerRequestMsats,
          allowedHosts: currentPolicy.allowedHosts,
          blockedHosts: currentPolicy.blockedHosts,
          quotaPerMinute: currentPolicy.quotaPerMinute,
          quotaPerDay: currentPolicy.quotaPerDay,
          killSwitch: currentPolicy.killSwitch,
        } satisfies NormalizedPolicy);

    const normalizedRoutes = args.routes
      ? yield* Effect.forEach(args.routes, (route, index) => normalizeRoute(route, index))
      : yield* Effect.forEach(currentRoutes, (route) => normalizeRoute(route, route.priority - 1));

    if (normalizedRoutes.length === 0) return yield* Effect.fail(new Error("invalid_input"));
    yield* assertPolicyAllowsRoutes(normalizedPolicy, normalizedRoutes);
    if (args.routes) yield* assertNoRouteConflicts(ctx, normalizedRoutes, paywall.paywallId);

    const requestId = normalizeOptionalString(args.requestId, 160);
    const name = args.name === undefined ? paywall.name : normalizeOptionalString(args.name, 160);
    if (!name) return yield* Effect.fail(new Error("invalid_input"));
    const description = args.description === undefined ? paywall.description : normalizeOptionalString(args.description, 2_000);

    const now = nowMs();
    const nextStatus =
      normalizedPolicy.killSwitch && paywall.status === "active" ? ("paused" as L402PaywallStatus) : paywall.status;

    yield* tryPromise(() =>
      ctx.db.patch(paywall._id, {
        name,
        description,
        status: nextStatus,
        requestId: requestId ?? paywall.requestId,
        metadata: args.metadata ?? paywall.metadata,
        updatedAtMs: now,
      }),
    );

    if (args.policy) {
      yield* tryPromise(() =>
        ctx.db.patch(currentPolicy._id, {
          pricingMode: normalizedPolicy.pricingMode,
          fixedAmountMsats: normalizedPolicy.fixedAmountMsats,
          maxPerRequestMsats: normalizedPolicy.maxPerRequestMsats,
          allowedHosts: normalizedPolicy.allowedHosts,
          blockedHosts: normalizedPolicy.blockedHosts,
          quotaPerMinute: normalizedPolicy.quotaPerMinute,
          quotaPerDay: normalizedPolicy.quotaPerDay,
          killSwitch: normalizedPolicy.killSwitch,
          updatedAtMs: now,
        }),
      );
    }

    if (args.routes) {
      for (const route of currentRoutes) {
        yield* tryPromise(() => ctx.db.delete(route._id));
      }
      for (const route of normalizedRoutes) {
        yield* tryPromise(() =>
          ctx.db.insert("l402PaywallRoutes", {
            routeId: `route_${newId()}`,
            paywallId: paywall.paywallId,
            ownerId,
            hostPattern: route.hostPattern,
            pathPattern: route.pathPattern,
            upstreamUrl: route.upstreamUrl,
            protocol: route.protocol,
            timeoutMs: route.timeoutMs,
            priority: route.priority,
            createdAtMs: now,
            updatedAtMs: now,
          }),
        );
      }
    }

    yield* insertGatewayEvent(ctx, {
      paywallId: paywall.paywallId,
      ownerId,
      eventType: normalizedPolicy.killSwitch ? "paywall_updated_kill_switch" : "paywall_updated",
      level: "info",
      requestId,
      metadata: args.metadata,
      createdAtMs: now,
    });

    const updated = yield* assertPaywallAccess(ctx, { paywallId: paywall.paywallId, ownerId });
    const view = yield* loadPaywallView(ctx, updated);
    return { ok: true as const, paywall: view };
  });

export const updatePaywall = effectMutation({
  args: {
    paywallId: v.string(),
    name: v.optional(v.string()),
    description: v.optional(v.string()),
    policy: v.optional(paywallPolicyInputValidator),
    routes: v.optional(v.array(paywallRouteInputValidator)),
    requestId: v.optional(v.string()),
    metadata: v.optional(v.any()),
  },
  returns: v.object({
    ok: v.boolean(),
    paywall: paywallValidator,
  }),
  handler: updatePaywallImpl,
});

const transitionPaywallStatus = (
  ctx: EffectMutationCtx,
  args: { readonly paywallId: string; readonly toStatus: "active" | "paused"; readonly requestId?: string; readonly reason?: string },
) =>
  Effect.gen(function* () {
    const ownerId = yield* requireSubject(ctx);
    const paywall = yield* assertPaywallAccess(ctx, { paywallId: args.paywallId, ownerId });
    const policy = yield* loadPolicyByPaywall(ctx, paywall.paywallId);
    if (!policy) return yield* Effect.fail(new Error("policy_missing"));
    if (paywall.status === "archived") return yield* Effect.fail(new Error("invalid_transition"));

    if (args.toStatus === "active" && policy.killSwitch) {
      return yield* Effect.fail(new Error("policy_violation"));
    }

    if (paywall.status === args.toStatus) {
      const view = yield* loadPaywallView(ctx, paywall);
      return { ok: true as const, changed: false as const, paywall: view };
    }

    if (args.toStatus === "paused" && paywall.status !== "active") {
      return yield* Effect.fail(new Error("invalid_transition"));
    }
    if (args.toStatus === "active" && paywall.status !== "paused") {
      return yield* Effect.fail(new Error("invalid_transition"));
    }

    const now = nowMs();
    yield* tryPromise(() =>
      ctx.db.patch(paywall._id, {
        status: args.toStatus,
        updatedAtMs: now,
      }),
    );

    yield* insertGatewayEvent(ctx, {
      paywallId: paywall.paywallId,
      ownerId,
      eventType: args.toStatus === "paused" ? "paywall_paused" : "paywall_resumed",
      level: "info",
      requestId: normalizeOptionalString(args.requestId, 160),
      metadata: args.reason ? { reason: normalizeOptionalString(args.reason, 500) } : undefined,
      createdAtMs: now,
    });

    const updated = yield* assertPaywallAccess(ctx, { paywallId: paywall.paywallId, ownerId });
    const view = yield* loadPaywallView(ctx, updated);
    return { ok: true as const, changed: true as const, paywall: view };
  });

export const pausePaywallImpl = (
  ctx: EffectMutationCtx,
  args: { readonly paywallId: string; readonly requestId?: string; readonly reason?: string },
) => transitionPaywallStatus(ctx, { paywallId: args.paywallId, toStatus: "paused", requestId: args.requestId, reason: args.reason });

export const pausePaywall = effectMutation({
  args: {
    paywallId: v.string(),
    requestId: v.optional(v.string()),
    reason: v.optional(v.string()),
  },
  returns: v.object({
    ok: v.boolean(),
    changed: v.boolean(),
    paywall: paywallValidator,
  }),
  handler: pausePaywallImpl,
});

export const resumePaywallImpl = (
  ctx: EffectMutationCtx,
  args: { readonly paywallId: string; readonly requestId?: string; readonly reason?: string },
) => transitionPaywallStatus(ctx, { paywallId: args.paywallId, toStatus: "active", requestId: args.requestId, reason: args.reason });

export const resumePaywall = effectMutation({
  args: {
    paywallId: v.string(),
    requestId: v.optional(v.string()),
    reason: v.optional(v.string()),
  },
  returns: v.object({
    ok: v.boolean(),
    changed: v.boolean(),
    paywall: paywallValidator,
  }),
  handler: resumePaywallImpl,
});

export const getPaywallImpl = (ctx: EffectQueryCtx, args: { readonly paywallId: string }) =>
  Effect.gen(function* () {
    const ownerId = yield* requireSubject(ctx);
    const paywall = yield* assertPaywallAccess(ctx, { paywallId: args.paywallId, ownerId });
    const view = yield* loadPaywallView(ctx, paywall);
    return { ok: true as const, paywall: view };
  });

export const getPaywall = effectQuery({
  args: {
    paywallId: v.string(),
  },
  returns: v.object({
    ok: v.boolean(),
    paywall: paywallValidator,
  }),
  handler: getPaywallImpl,
});

export const listPaywallsImpl = (
  ctx: EffectQueryCtx,
  args: { readonly status?: L402PaywallStatus; readonly limit?: number },
) =>
  Effect.gen(function* () {
    const ownerId = yield* requireSubject(ctx);
    const limit =
      typeof args.limit === "number" && Number.isFinite(args.limit) ? Math.max(1, Math.min(200, Math.floor(args.limit))) : 50;

    const rows = (yield* (args.status
      ? tryPromise(() =>
          ctx.db
            .query("l402Paywalls")
            .withIndex("by_ownerId_status_updatedAtMs", (q) => q.eq("ownerId", ownerId).eq("status", args.status!))
            .order("desc")
            .take(limit),
        )
      : tryPromise(() =>
          ctx.db
            .query("l402Paywalls")
            .withIndex("by_ownerId_updatedAtMs", (q) => q.eq("ownerId", ownerId))
            .order("desc")
            .take(limit),
        ))) as ReadonlyArray<PaywallDoc>;

    const views = yield* Effect.forEach(rows, (paywall) => loadPaywallView(ctx, paywall));
    return {
      ok: true as const,
      paywalls: views,
    };
  });

export const listPaywalls = effectQuery({
  args: {
    status: v.optional(paywallStatusValidator),
    limit: v.optional(v.number()),
  },
  returns: v.object({
    ok: v.boolean(),
    paywalls: v.array(paywallValidator),
  }),
  handler: listPaywallsImpl,
});

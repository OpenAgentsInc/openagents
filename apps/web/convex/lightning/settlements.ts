import { v } from "convex/values";
import { Effect } from "effect";

import type { EffectMutationCtx, EffectQueryCtx } from "../effect/ctx";
import { effectMutation, effectQuery } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

import { getSubject } from "../autopilot/access";

const nowMs = () => Date.now();

const invoiceStatusValidator = v.union(v.literal("open"), v.literal("settled"), v.literal("canceled"), v.literal("expired"));
const paymentProofTypeValidator = v.literal("lightning_preimage");

const invoiceValidator = v.object({
  invoiceId: v.string(),
  paywallId: v.string(),
  ownerId: v.string(),
  amountMsats: v.number(),
  status: invoiceStatusValidator,
  paymentHash: v.optional(v.string()),
  paymentRequest: v.optional(v.string()),
  paymentProofRef: v.optional(v.string()),
  requestId: v.optional(v.string()),
  createdAtMs: v.number(),
  updatedAtMs: v.number(),
  settledAtMs: v.optional(v.number()),
});

const settlementValidator = v.object({
  settlementId: v.string(),
  paywallId: v.string(),
  ownerId: v.string(),
  invoiceId: v.optional(v.string()),
  amountMsats: v.number(),
  paymentProofRef: v.string(),
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

const normalizeOptionalString = (value: unknown, maxLen: number): string | undefined => {
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  if (!trimmed) return undefined;
  return trimmed.slice(0, maxLen);
};

const normalizePositiveInt = (value: unknown): number | undefined => {
  if (typeof value !== "number" || !Number.isFinite(value)) return undefined;
  const normalized = Math.floor(value);
  if (normalized <= 0) return undefined;
  return normalized;
};

const parseLimit = (value: number | undefined, max = 200, fallback = 50): number => {
  if (typeof value !== "number" || !Number.isFinite(value)) return fallback;
  return Math.max(1, Math.min(max, Math.floor(value)));
};

const parseCursor = (value: unknown): number | undefined => {
  if (typeof value !== "number" || !Number.isFinite(value)) return undefined;
  return Math.floor(value);
};

const requireSubject = (ctx: EffectQueryCtx | EffectMutationCtx): Effect.Effect<string, Error> =>
  getSubject(ctx).pipe(
    Effect.flatMap((subject) => (subject ? Effect.succeed(subject) : Effect.fail(new Error("unauthorized")))),
  );

const isLowerHex = (value: string): boolean => /^[0-9a-f]+$/.test(value);

const normalizePreimage = (value: string): string | null => {
  const normalized = value.trim().toLowerCase();
  if (!normalized || !isLowerHex(normalized)) return null;
  return normalized;
};

export const formatPaymentProofReference = (preimageHex: string): string => {
  const normalized = normalizePreimage(preimageHex) ?? preimageHex.trim().toLowerCase();
  return `lightning_preimage:${normalized.slice(0, 24)}`;
};

const invoiceRank: Record<"open" | "settled" | "canceled" | "expired", number> = {
  open: 0,
  canceled: 1,
  expired: 1,
  settled: 2,
};

const chooseInvoiceStatus = (
  current: "open" | "settled" | "canceled" | "expired",
  incoming: "open" | "settled" | "canceled" | "expired",
): "open" | "settled" | "canceled" | "expired" => {
  const currentRank = invoiceRank[current];
  const incomingRank = invoiceRank[incoming];
  if (incomingRank > currentRank) return incoming;
  return current;
};

type InvoiceDoc = {
  readonly _id: any;
  readonly invoiceId: string;
  readonly paywallId: string;
  readonly ownerId: string;
  readonly amountMsats: number;
  readonly status: "open" | "settled" | "canceled" | "expired";
  readonly paymentHash?: string;
  readonly paymentRequest?: string;
  readonly paymentProofRef?: string;
  readonly requestId?: string;
  readonly createdAtMs: number;
  readonly updatedAtMs: number;
  readonly settledAtMs?: number;
};

type SettlementDoc = {
  readonly _id: any;
  readonly settlementId: string;
  readonly paywallId: string;
  readonly ownerId: string;
  readonly invoiceId?: string;
  readonly amountMsats: number;
  readonly paymentProofRef: string;
  readonly requestId?: string;
  readonly metadata?: unknown;
  readonly createdAtMs: number;
};

type PaywallDoc = {
  readonly _id: any;
  readonly paywallId: string;
  readonly ownerId: string;
};

const loadInvoiceByInvoiceId = (
  ctx: EffectQueryCtx | EffectMutationCtx,
  invoiceId: string,
): Effect.Effect<InvoiceDoc | null, Error> =>
  tryPromise(() =>
    ctx.db
      .query("l402Invoices")
      .withIndex("by_invoiceId", (q) => q.eq("invoiceId", invoiceId))
      .unique(),
  ) as Effect.Effect<InvoiceDoc | null, Error>;

const loadSettlementBySettlementId = (
  ctx: EffectQueryCtx | EffectMutationCtx,
  settlementId: string,
): Effect.Effect<SettlementDoc | null, Error> =>
  tryPromise(() =>
    ctx.db
      .query("l402Settlements")
      .withIndex("by_settlementId", (q) => q.eq("settlementId", settlementId))
      .unique(),
  ) as Effect.Effect<SettlementDoc | null, Error>;

const loadPaywallByPaywallId = (
  ctx: EffectQueryCtx | EffectMutationCtx,
  paywallId: string,
): Effect.Effect<PaywallDoc | null, Error> =>
  tryPromise(() =>
    ctx.db
      .query("l402Paywalls")
      .withIndex("by_paywallId", (q) => q.eq("paywallId", paywallId))
      .unique(),
  ) as Effect.Effect<PaywallDoc | null, Error>;

const toInvoice = (invoice: InvoiceDoc) => ({
  invoiceId: invoice.invoiceId,
  paywallId: invoice.paywallId,
  ownerId: invoice.ownerId,
  amountMsats: invoice.amountMsats,
  status: invoice.status,
  paymentHash: invoice.paymentHash,
  paymentRequest: invoice.paymentRequest,
  paymentProofRef: invoice.paymentProofRef,
  requestId: invoice.requestId,
  createdAtMs: invoice.createdAtMs,
  updatedAtMs: invoice.updatedAtMs,
  settledAtMs: invoice.settledAtMs,
});

const toSettlement = (settlement: SettlementDoc) => ({
  settlementId: settlement.settlementId,
  paywallId: settlement.paywallId,
  ownerId: settlement.ownerId,
  invoiceId: settlement.invoiceId,
  amountMsats: settlement.amountMsats,
  paymentProofRef: settlement.paymentProofRef,
  requestId: settlement.requestId,
  metadata: settlement.metadata,
  createdAtMs: settlement.createdAtMs,
});

const receiptMetadata = (input: {
  readonly rail: "lightning";
  readonly assetId: "BTC_LN";
  readonly amountMsats: number;
  readonly paymentHash?: string;
  readonly paymentProofValue: string;
  readonly paymentProofRef: string;
  readonly requestId?: string;
  readonly taskId?: string;
  readonly routeId?: string;
  readonly paywallId: string;
  readonly ownerId: string;
  readonly invoiceId?: string;
}) => ({
  rail: input.rail,
  asset_id: input.assetId,
  amount_msats: input.amountMsats,
  payment_proof: {
    type: "lightning_preimage" as const,
    value: input.paymentProofValue,
  },
  payment_hash: input.paymentHash,
  payment_proof_ref: input.paymentProofRef,
  correlation: {
    request_id: input.requestId,
    task_id: input.taskId,
    route_id: input.routeId,
    paywall_id: input.paywallId,
    owner_id: input.ownerId,
    invoice_id: input.invoiceId,
    payment_hash: input.paymentHash,
  },
});

const upsertInvoiceLifecycle = (
  ctx: EffectMutationCtx,
  input: {
    readonly invoiceId: string;
    readonly paywallId: string;
    readonly ownerId: string;
    readonly amountMsats: number;
    readonly status: "open" | "settled" | "canceled" | "expired";
    readonly paymentHash?: string;
    readonly paymentRequest?: string;
    readonly paymentProofRef?: string;
    readonly requestId?: string;
    readonly settledAtMs?: number;
  },
): Effect.Effect<{ readonly changed: boolean; readonly invoice: ReturnType<typeof toInvoice> }, Error> =>
  Effect.gen(function* () {
    const existing = yield* loadInvoiceByInvoiceId(ctx, input.invoiceId);
    const now = nowMs();

    if (!existing) {
      yield* tryPromise(() =>
        ctx.db.insert("l402Invoices", {
          invoiceId: input.invoiceId,
          paywallId: input.paywallId,
          ownerId: input.ownerId,
          amountMsats: input.amountMsats,
          status: input.status,
          paymentHash: input.paymentHash,
          paymentRequest: input.paymentRequest,
          paymentProofRef: input.paymentProofRef,
          requestId: input.requestId,
          createdAtMs: now,
          updatedAtMs: now,
          settledAtMs: input.status === "settled" ? (input.settledAtMs ?? now) : undefined,
        }),
      );

      const inserted = yield* loadInvoiceByInvoiceId(ctx, input.invoiceId);
      if (!inserted) return yield* Effect.fail(new Error("invoice_not_found"));
      return { changed: true as const, invoice: toInvoice(inserted) };
    }

    if (existing.paywallId !== input.paywallId || existing.ownerId !== input.ownerId) {
      return yield* Effect.fail(new Error("invoice_scope_mismatch"));
    }

    const nextStatus = chooseInvoiceStatus(existing.status, input.status);
    const nextPaymentHash = existing.paymentHash ?? input.paymentHash;
    const nextPaymentRequest = existing.paymentRequest ?? input.paymentRequest;
    const nextPaymentProofRef = existing.paymentProofRef ?? input.paymentProofRef;
    const nextRequestId = existing.requestId ?? input.requestId;
    const nextSettledAtMs =
      nextStatus === "settled"
        ? existing.settledAtMs ?? input.settledAtMs ?? now
        : existing.settledAtMs;

    const changed =
      existing.status !== nextStatus ||
      existing.amountMsats !== input.amountMsats ||
      existing.paymentHash !== nextPaymentHash ||
      existing.paymentRequest !== nextPaymentRequest ||
      existing.paymentProofRef !== nextPaymentProofRef ||
      existing.requestId !== nextRequestId ||
      existing.settledAtMs !== nextSettledAtMs;

    if (!changed) {
      return { changed: false as const, invoice: toInvoice(existing) };
    }

    yield* tryPromise(() =>
      ctx.db.patch(existing._id, {
        amountMsats: input.amountMsats,
        status: nextStatus,
        paymentHash: nextPaymentHash,
        paymentRequest: nextPaymentRequest,
        paymentProofRef: nextPaymentProofRef,
        requestId: nextRequestId,
        settledAtMs: nextSettledAtMs,
        updatedAtMs: now,
      }),
    );

    const updated = yield* loadInvoiceByInvoiceId(ctx, input.invoiceId);
    if (!updated) return yield* Effect.fail(new Error("invoice_not_found"));
    return {
      changed: true as const,
      invoice: toInvoice(updated),
    };
  });

export const ingestInvoiceLifecycleImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly secret: string;
    readonly invoiceId: string;
    readonly paywallId: string;
    readonly ownerId: string;
    readonly amountMsats: number;
    readonly status: "open" | "settled" | "canceled" | "expired";
    readonly paymentHash?: string;
    readonly paymentRequest?: string;
    readonly paymentProofRef?: string;
    readonly requestId?: string;
    readonly settledAtMs?: number;
  },
) =>
  Effect.gen(function* () {
    yield* assertOpsSecret(args.secret);

    const invoiceId = normalizeOptionalString(args.invoiceId, 160);
    const paywallId = normalizeOptionalString(args.paywallId, 160);
    const ownerId = normalizeOptionalString(args.ownerId, 160);
    const amountMsats = normalizePositiveInt(args.amountMsats);
    if (!invoiceId || !paywallId || !ownerId || amountMsats === undefined) {
      return yield* Effect.fail(new Error("invalid_input"));
    }

    const paymentHash = normalizeOptionalString(args.paymentHash, 256);
    const paymentRequest = normalizeOptionalString(args.paymentRequest, 4_096);
    const paymentProofRef = normalizeOptionalString(args.paymentProofRef, 256);
    const requestId = normalizeOptionalString(args.requestId, 180);
    const settledAtMs = parseCursor(args.settledAtMs);

    return yield* upsertInvoiceLifecycle(ctx, {
      invoiceId,
      paywallId,
      ownerId,
      amountMsats,
      status: args.status,
      paymentHash,
      paymentRequest,
      paymentProofRef,
      requestId,
      settledAtMs,
    }).pipe(
      Effect.map((result) => ({
        ok: true as const,
        changed: result.changed,
        invoice: result.invoice,
      })),
    );
  });

export const ingestInvoiceLifecycle = effectMutation({
  args: {
    secret: v.string(),
    invoiceId: v.string(),
    paywallId: v.string(),
    ownerId: v.string(),
    amountMsats: v.number(),
    status: invoiceStatusValidator,
    paymentHash: v.optional(v.string()),
    paymentRequest: v.optional(v.string()),
    paymentProofRef: v.optional(v.string()),
    requestId: v.optional(v.string()),
    settledAtMs: v.optional(v.number()),
  },
  returns: v.object({
    ok: v.boolean(),
    changed: v.boolean(),
    invoice: invoiceValidator,
  }),
  handler: ingestInvoiceLifecycleImpl,
});

export const ingestSettlementImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly secret: string;
    readonly settlementId: string;
    readonly paywallId: string;
    readonly ownerId: string;
    readonly invoiceId?: string;
    readonly amountMsats: number;
    readonly paymentHash?: string;
    readonly paymentProofType: "lightning_preimage";
    readonly paymentProofValue: string;
    readonly requestId?: string;
    readonly taskId?: string;
    readonly routeId?: string;
    readonly metadata?: unknown;
  },
) =>
  Effect.gen(function* () {
    yield* assertOpsSecret(args.secret);

    const settlementId = normalizeOptionalString(args.settlementId, 180);
    const paywallId = normalizeOptionalString(args.paywallId, 160);
    const ownerId = normalizeOptionalString(args.ownerId, 160);
    const invoiceId = normalizeOptionalString(args.invoiceId, 160);
    const paymentHash = normalizeOptionalString(args.paymentHash, 256);
    const requestId = normalizeOptionalString(args.requestId, 180);
    const taskId = normalizeOptionalString(args.taskId, 180);
    const routeId = normalizeOptionalString(args.routeId, 180);
    const amountMsats = normalizePositiveInt(args.amountMsats);

    if (!settlementId || !paywallId || !ownerId || amountMsats === undefined) {
      return yield* Effect.fail(new Error("invalid_input"));
    }

    if (args.paymentProofType !== "lightning_preimage") {
      return yield* Effect.fail(new Error("invalid_input"));
    }

    const preimageHex = normalizePreimage(args.paymentProofValue);
    if (!preimageHex) {
      return yield* Effect.fail(new Error("invalid_input"));
    }

    const paymentProofRef = formatPaymentProofReference(preimageHex);
    const now = nowMs();

    const existing = yield* loadSettlementBySettlementId(ctx, settlementId);
    if (existing) {
      const invoice = invoiceId
        ? yield* upsertInvoiceLifecycle(ctx, {
            invoiceId,
            paywallId,
            ownerId,
            amountMsats,
            status: "settled",
            paymentHash,
            paymentProofRef,
            requestId,
            settledAtMs: now,
          }).pipe(Effect.map((row) => row.invoice))
        : undefined;

      return {
        ok: true as const,
        existed: true as const,
        settlement: toSettlement(existing),
        invoice,
      };
    }

    const receipt = receiptMetadata({
      rail: "lightning",
      assetId: "BTC_LN",
      amountMsats,
      paymentHash,
      paymentProofValue: preimageHex,
      paymentProofRef,
      requestId,
      taskId,
      routeId,
      paywallId,
      ownerId,
      invoiceId,
    });

    yield* tryPromise(() =>
      ctx.db.insert("l402Settlements", {
        settlementId,
        paywallId,
        ownerId,
        invoiceId,
        amountMsats,
        paymentProofRef,
        requestId,
        metadata: {
          receipt,
          taskId,
          routeId,
          sourceMetadata: args.metadata,
        },
        createdAtMs: now,
      }),
    );

    const row = yield* loadSettlementBySettlementId(ctx, settlementId);
    if (!row) return yield* Effect.fail(new Error("settlement_not_found"));

    const invoice = invoiceId
      ? yield* upsertInvoiceLifecycle(ctx, {
          invoiceId,
          paywallId,
          ownerId,
          amountMsats,
          status: "settled",
          paymentHash,
          paymentProofRef,
          requestId,
          settledAtMs: now,
        }).pipe(Effect.map((updated) => updated.invoice))
      : undefined;

    return {
      ok: true as const,
      existed: false as const,
      settlement: toSettlement(row),
      invoice,
    };
  });

export const ingestSettlement = effectMutation({
  args: {
    secret: v.string(),
    settlementId: v.string(),
    paywallId: v.string(),
    ownerId: v.string(),
    invoiceId: v.optional(v.string()),
    amountMsats: v.number(),
    paymentHash: v.optional(v.string()),
    paymentProofType: paymentProofTypeValidator,
    paymentProofValue: v.string(),
    requestId: v.optional(v.string()),
    taskId: v.optional(v.string()),
    routeId: v.optional(v.string()),
    metadata: v.optional(v.any()),
  },
  returns: v.object({
    ok: v.boolean(),
    existed: v.boolean(),
    settlement: settlementValidator,
    invoice: v.optional(invoiceValidator),
  }),
  handler: ingestSettlementImpl,
});

export const listOwnerSettlementsImpl = (
  ctx: EffectQueryCtx,
  args: {
    readonly limit?: number;
    readonly beforeCreatedAtMs?: number;
  },
) =>
  Effect.gen(function* () {
    const ownerId = yield* requireSubject(ctx);
    const limit = parseLimit(args.limit);
    const beforeCreatedAtMs = parseCursor(args.beforeCreatedAtMs);

    const rows = (yield* tryPromise(() =>
      ctx.db
        .query("l402Settlements")
        .withIndex("by_ownerId_createdAtMs", (q) => q.eq("ownerId", ownerId))
        .order("desc")
        .collect(),
    )) as ReadonlyArray<SettlementDoc>;

    const filtered = rows.filter((row) =>
      beforeCreatedAtMs !== undefined ? row.createdAtMs < beforeCreatedAtMs : true,
    );
    const page = filtered.slice(0, limit);
    const nextCursor = page.length === limit ? (page[page.length - 1]?.createdAtMs ?? null) : null;

    return {
      ok: true as const,
      settlements: page.map(toSettlement),
      nextCursor,
    };
  });

export const listOwnerSettlements = effectQuery({
  args: {
    limit: v.optional(v.number()),
    beforeCreatedAtMs: v.optional(v.number()),
  },
  returns: v.object({
    ok: v.boolean(),
    settlements: v.array(settlementValidator),
    nextCursor: v.union(v.number(), v.null()),
  }),
  handler: listOwnerSettlementsImpl,
});

export const listPaywallSettlementsImpl = (
  ctx: EffectQueryCtx,
  args: {
    readonly paywallId: string;
    readonly limit?: number;
    readonly beforeCreatedAtMs?: number;
  },
) =>
  Effect.gen(function* () {
    const ownerId = yield* requireSubject(ctx);
    const paywall = yield* loadPaywallByPaywallId(ctx, args.paywallId);
    if (!paywall) return yield* Effect.fail(new Error("paywall_not_found"));
    if (paywall.ownerId !== ownerId) return yield* Effect.fail(new Error("forbidden"));

    const limit = parseLimit(args.limit);
    const beforeCreatedAtMs = parseCursor(args.beforeCreatedAtMs);

    const rows = (yield* tryPromise(() =>
      ctx.db
        .query("l402Settlements")
        .withIndex("by_paywallId_createdAtMs", (q) => q.eq("paywallId", args.paywallId))
        .order("desc")
        .collect(),
    )) as ReadonlyArray<SettlementDoc>;

    const filtered = rows
      .filter((row) => row.ownerId === ownerId)
      .filter((row) => (beforeCreatedAtMs !== undefined ? row.createdAtMs < beforeCreatedAtMs : true));

    const page = filtered.slice(0, limit);
    const nextCursor = page.length === limit ? (page[page.length - 1]?.createdAtMs ?? null) : null;

    return {
      ok: true as const,
      settlements: page.map(toSettlement),
      nextCursor,
    };
  });

export const listPaywallSettlements = effectQuery({
  args: {
    paywallId: v.string(),
    limit: v.optional(v.number()),
    beforeCreatedAtMs: v.optional(v.number()),
  },
  returns: v.object({
    ok: v.boolean(),
    settlements: v.array(settlementValidator),
    nextCursor: v.union(v.number(), v.null()),
  }),
  handler: listPaywallSettlementsImpl,
});

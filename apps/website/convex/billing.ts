import { v } from "convex/values";
import { internalMutation, internalQuery, type MutationCtx, type QueryCtx } from "./_generated/server";
import { fail } from "./lib/errors";

const roundUsd = (value: number): number => {
  return Math.round((value + Number.EPSILON) * 100) / 100;
};

const requirePositive = (value: number, label: string): number => {
  if (!Number.isFinite(value) || value <= 0) {
    fail("BAD_REQUEST", `${label} must be greater than zero`);
  }
  return value;
};

const computeBalance = async (
  ctx: QueryCtx | MutationCtx,
  user_id: string,
): Promise<number> => {
  const entries = await ctx.db
    .query("credit_ledger")
    .withIndex("by_user_id", (q) => q.eq("user_id", user_id))
    .collect();
  const total = entries.reduce((sum, entry) => sum + entry.amount_usd, 0);
  return roundUsd(total);
};

export const getCreditBalance = internalQuery({
  args: {
    user_id: v.string(),
  },
  handler: async (ctx, args) => {
    return {
      user_id: args.user_id,
      balance_usd: await computeBalance(ctx, args.user_id),
    };
  },
});

export const grantMonthlyCredits = internalMutation({
  args: {
    user_id: v.string(),
    amount_usd: v.number(),
    meta: v.optional(v.any()),
  },
  handler: async (ctx, args) => {
    const amount = roundUsd(requirePositive(args.amount_usd, "amount_usd"));
    await ctx.db.insert("credit_ledger", {
      user_id: args.user_id,
      kind: "grant",
      amount_usd: amount,
      meta: args.meta,
      created_at: Date.now(),
    });
    return {
      user_id: args.user_id,
      balance_usd: await computeBalance(ctx, args.user_id),
    };
  },
});

export const burnCredits = internalMutation({
  args: {
    user_id: v.string(),
    amount_usd: v.number(),
    meta: v.optional(v.any()),
  },
  handler: async (ctx, args) => {
    const amount = roundUsd(requirePositive(args.amount_usd, "amount_usd"));
    await ctx.db.insert("credit_ledger", {
      user_id: args.user_id,
      kind: "burn",
      amount_usd: -amount,
      meta: args.meta,
      created_at: Date.now(),
    });
    return {
      user_id: args.user_id,
      balance_usd: await computeBalance(ctx, args.user_id),
    };
  },
});

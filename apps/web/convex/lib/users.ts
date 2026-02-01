import type { Doc } from "../_generated/dataModel";
import type { MutationCtx, QueryCtx } from "../_generated/server";
import { fail, requireFound } from "./errors";

type UserDoc = Doc<"users">;

type Identity = {
  subject: string;
  email?: string | null;
  name?: string | null;
  pictureUrl?: string | null;
};

const normalizeIdentity = (identity: unknown): Identity | null => {
  if (!identity || typeof identity !== "object") {
    return null;
  }
  const record = identity as {
    subject?: string;
    email?: string | null;
    name?: string | null;
    pictureUrl?: string | null;
  };
  if (!record.subject) {
    return null;
  }
  return {
    subject: record.subject,
    email: record.email ?? undefined,
    name: record.name ?? undefined,
    pictureUrl: record.pictureUrl ?? undefined,
  };
};

export const getUser = async (
  ctx: QueryCtx | MutationCtx,
): Promise<UserDoc | null> => {
  const identity = normalizeIdentity(await ctx.auth.getUserIdentity());
  if (!identity) {
    return null;
  }
  return (
    (await ctx.db
      .query("users")
      .withIndex("by_user_id", (q) => q.eq("user_id", identity.subject))
      .first()) ?? null
  );
};

export const requireUser = async (ctx: MutationCtx): Promise<UserDoc> => {
  const identity = normalizeIdentity(await ctx.auth.getUserIdentity());
  if (!identity) {
    fail("UNAUTHORIZED", "Not authenticated");
  }
  const identityValue = identity as Identity;

  const existing = await ctx.db
    .query("users")
    .withIndex("by_user_id", (q) => q.eq("user_id", identityValue.subject))
    .first();

  if (existing) {
    const updates: Partial<UserDoc> = {};
    if (identityValue.email && identityValue.email !== existing.email) {
      updates.email = identityValue.email;
    }
    if (identityValue.name && identityValue.name !== existing.name) {
      updates.name = identityValue.name;
    }
    if (identityValue.pictureUrl && identityValue.pictureUrl !== existing.image) {
      updates.image = identityValue.pictureUrl;
    }
    if (Object.keys(updates).length > 0) {
      await ctx.db.patch(existing._id, updates);
    }
    return (await ctx.db.get(existing._id)) ?? existing;
  }

  const now = Date.now();
  const newUser: {
    user_id: string;
    created_at: number;
    credits: number;
    plan: string;
    email?: string;
    name?: string;
    image?: string;
  } = {
    user_id: identityValue.subject,
    created_at: now,
    credits: 100,
    plan: "free",
  };

  if (identityValue.email) {
    newUser.email = identityValue.email;
  }
  if (identityValue.name) {
    newUser.name = identityValue.name;
  }
  if (identityValue.pictureUrl) {
    newUser.image = identityValue.pictureUrl;
  }

  const userId = await ctx.db.insert("users", newUser);
  const created = await ctx.db.get(userId);
  return requireFound(created, "NOT_FOUND", "User not found after creation");
};

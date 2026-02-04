import { v } from 'convex/values';
import { mutation, query } from './_generated/server';
import { requireFound } from './lib/errors';
import { requireAdmin } from './lib/admin';

type Identity = {
  subject: string;
  email?: string | null;
};

/** When no env admin list is set, this userId is treated as admin (e.g. chris@openagents.com). */
const DEFAULT_ADMIN_USER_ID = 'user_01KF7DWP1TE4S9S770HX6XPMYC';

function parseList(value: string | undefined | null): Set<string> {
  if (!value) return new Set();
  return new Set(
    value
      .split(/[\n,]+/g)
      .map((entry) => entry.trim().toLowerCase())
      .filter(Boolean),
  );
}

function normalizeIdentity(identity: unknown): Identity | null {
  if (!identity || typeof identity !== 'object') return null;
  const record = identity as { subject?: string; email?: string | null };
  if (!record.subject) return null;
  return { subject: record.subject, email: record.email ?? null };
}

function normalizeEmail(value: string): string {
  return value.trim().toLowerCase();
}

export const getAdminStatus = query({
  args: {},
  handler: async (ctx) => {
    const identity = normalizeIdentity(await ctx.auth.getUserIdentity());
    if (!identity) {
      return { isAdmin: false, userId: null, email: null };
    }

    const configuredEmails = parseList(process.env.OA_ADMIN_EMAILS ?? process.env.OA_APEX_ADMINS);
    let adminUserIds = parseList(process.env.OA_ADMIN_USER_IDS);
    const useDefaults = configuredEmails.size === 0 && adminUserIds.size === 0;
    const adminEmails = useDefaults ? new Set(['chris@openagents.com']) : configuredEmails;
    if (useDefaults && DEFAULT_ADMIN_USER_ID) {
      adminUserIds = new Set([...adminUserIds, DEFAULT_ADMIN_USER_ID]);
    }
    let email = identity.email?.toLowerCase() ?? null;
    if (!email) {
      const user = await ctx.db
        .query('users')
        .withIndex('by_user_id', (q) => q.eq('user_id', identity.subject))
        .first();
      email = user?.email?.toLowerCase() ?? null;
    }

    const isAdmin =
      (email !== null && adminEmails.has(email)) || adminUserIds.has(identity.subject);

    return {
      isAdmin,
      userId: identity.subject,
      email,
    };
  },
});

export const listUsers = query({
  args: {},
  handler: async (ctx) => {
    await requireAdmin(ctx);
    const rows = await ctx.db.query('users').collect();
    return rows
      .map((user) => ({
        _id: user._id,
        user_id: user.user_id,
        kind: user.kind,
        name: user.name ?? null,
        email: user.email ?? null,
        created_at: user.created_at,
        updated_at: user.updated_at,
        access_enabled: user.access_enabled ?? false,
        access_updated_at: user.access_updated_at ?? null,
        access_updated_by: user.access_updated_by ?? null,
      }))
      .sort((a, b) => b.created_at - a.created_at);
  },
});

export const listWaitlist = query({
  args: {},
  handler: async (ctx) => {
    await requireAdmin(ctx);
    const rows = await ctx.db.query('waitlist').withIndex('by_created_at').collect();
    return rows.map((entry) => ({
      _id: entry._id,
      email: entry.email,
      source: entry.source ?? null,
      created_at: entry.created_at,
      approved: entry.approved ?? false,
      approved_at: entry.approved_at ?? null,
      approved_by: entry.approved_by ?? null,
    }));
  },
});

export const setUserAccess = mutation({
  args: {
    user_id: v.string(),
    enabled: v.boolean(),
  },
  handler: async (ctx, args) => {
    const admin = await requireAdmin(ctx);
    const user = await ctx.db
      .query('users')
      .withIndex('by_user_id', (q) => q.eq('user_id', args.user_id))
      .first();
    const found = requireFound(user, 'NOT_FOUND', 'User not found');

    await ctx.db.patch(found._id, {
      access_enabled: args.enabled,
      access_updated_at: Date.now(),
      access_updated_by: admin.userId,
    });

    return {
      user_id: found.user_id,
      access_enabled: args.enabled,
    };
  },
});

export const setWaitlistApproval = mutation({
  args: {
    email: v.string(),
    approved: v.boolean(),
  },
  handler: async (ctx, args) => {
    const admin = await requireAdmin(ctx);
    const email = normalizeEmail(args.email);
    const entry = await ctx.db
      .query('waitlist')
      .withIndex('by_email', (q) => q.eq('email', email))
      .first();
    const found = requireFound(entry, 'NOT_FOUND', 'Waitlist entry not found');

    await ctx.db.patch(found._id, {
      approved: args.approved,
      approved_at: args.approved ? Date.now() : undefined,
      approved_by: args.approved ? admin.userId : undefined,
    });

    return {
      email,
      approved: args.approved,
    };
  },
});

import type { MutationCtx, QueryCtx } from '../_generated/server';
import { fail } from './errors';

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

export async function requireAdmin(ctx: QueryCtx | MutationCtx) {
  const identity = normalizeIdentity(await ctx.auth.getUserIdentity());
  if (!identity) {
    fail('UNAUTHORIZED', 'Not authenticated');
  }
  const safeIdentity = identity as Identity;

  const configuredEmails = parseList(process.env.OA_ADMIN_EMAILS ?? process.env.OA_APEX_ADMINS);
  let adminUserIds = parseList(process.env.OA_ADMIN_USER_IDS);
  const useDefaults = configuredEmails.size === 0 && adminUserIds.size === 0;
  const adminEmails = useDefaults ? new Set(['chris@openagents.com']) : configuredEmails;
  if (useDefaults && DEFAULT_ADMIN_USER_ID) {
    adminUserIds = new Set([...adminUserIds, DEFAULT_ADMIN_USER_ID]);
  }
  let email = safeIdentity.email?.toLowerCase() ?? null;
  if (!email) {
    const user = await ctx.db
      .query('users')
      .withIndex('by_user_id', (q) => q.eq('user_id', safeIdentity.subject))
      .first();
    email = user?.email?.toLowerCase() ?? null;
  }

  const isAdmin =
    (email !== null && adminEmails.has(email)) || adminUserIds.has(safeIdentity.subject);

  if (!isAdmin) {
    fail('FORBIDDEN', 'Not authorized');
  }

  return {
    userId: safeIdentity.subject,
    email,
  };
}

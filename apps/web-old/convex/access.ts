import { query } from './_generated/server';

type AccessStatus = {
  allowed: boolean;
  reason?: 'unauthenticated' | 'no_access';
  userAccess: boolean;
  waitlistApproved: boolean;
  user: { user_id: string; email: string | null } | null;
  waitlistEntry: {
    email: string;
    source: string | null;
    created_at: number;
    approved: boolean;
    approved_at: number | null;
  } | null;
};

function normalizeEmail(value: string | null | undefined): string | null {
  if (!value) return null;
  const trimmed = value.trim().toLowerCase();
  return trimmed.length > 0 ? trimmed : null;
}

export const getStatus = query({
  args: {},
  handler: async (ctx): Promise<AccessStatus> => {
    const identity = (await ctx.auth.getUserIdentity()) as
      | { subject?: string; email?: string | null }
      | null;
    if (!identity?.subject) {
      return {
        allowed: false,
        reason: 'unauthenticated',
        userAccess: false,
        waitlistApproved: false,
        user: null,
        waitlistEntry: null,
      };
    }

    const userId = identity.subject;
    const email = normalizeEmail(identity.email ?? null);
    const user = await ctx.db
      .query('users')
      .withIndex('by_user_id', (q) => q.eq('user_id', userId))
      .first();

    const userAccess = user?.access_enabled === true;
    let waitlistEntry = null;
    if (email) {
      waitlistEntry = await ctx.db
        .query('waitlist')
        .withIndex('by_email', (q) => q.eq('email', email))
        .first();
    }

    const waitlistApproved = waitlistEntry?.approved === true;
    const allowed = userAccess || waitlistApproved;

    return {
      allowed,
      reason: allowed ? undefined : 'no_access',
      userAccess,
      waitlistApproved,
      user: {
        user_id: userId,
        email,
      },
      waitlistEntry: waitlistEntry
        ? {
            email: waitlistEntry.email,
            source: waitlistEntry.source ?? null,
            created_at: waitlistEntry.created_at,
            approved: waitlistApproved,
            approved_at: waitlistEntry.approved_at ?? null,
          }
        : null,
    };
  },
});

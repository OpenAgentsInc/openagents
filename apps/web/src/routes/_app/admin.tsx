import { useEffect, useMemo, useState } from 'react';
import { createFileRoute, redirect, useNavigate } from '@tanstack/react-router';
import { getAuth } from '@workos/authkit-tanstack-react-start';
import { useAccessToken } from '@workos/authkit-tanstack-react-start/client';
import { useMutation, useQuery } from 'convex/react';
import { api } from '../../../convex/_generated/api';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';

type AdminUser = {
  email: string | null;
  id: string;
};

function parseList(value: string | undefined | null): Set<string> {
  if (!value) return new Set();
  return new Set(
    value
      .split(/[\n,]+/g)
      .map((entry) => entry.trim().toLowerCase())
      .filter(Boolean),
  );
}

function isAdmin(user: { id: string; email?: string | null } | null): boolean {
  if (!user) return false;
  const configuredEmails = parseList(process.env.OA_ADMIN_EMAILS ?? process.env.OA_APEX_ADMINS);
  const adminUserIds = parseList(process.env.OA_ADMIN_USER_IDS);
  const adminEmails =
    configuredEmails.size === 0 && adminUserIds.size === 0
      ? new Set(['chris@openagents.com'])
      : configuredEmails;
  const email = user.email?.toLowerCase() ?? null;
  return (email !== null && adminEmails.has(email)) || adminUserIds.has(user.id);
}

export const Route = createFileRoute('/_app/admin')({
  ssr: false,
  loader: async ({ location }) => {
    const auth = await getAuth().catch(() => null);
    const user = auth?.user ?? null;
    if (!user) {
      throw redirect({ to: '/login', search: { redirect: location.pathname } });
    }
    if (!isAdmin(user)) {
      throw redirect({ to: '/' });
    }
    return {
      admin: {
        id: user.id,
        email: user.email ?? null,
      } satisfies AdminUser,
    };
  },
  component: AdminPage,
});

function AdminPage() {
  const { admin } = Route.useLoaderData();
  const navigate = useNavigate();
  const [isClient, setIsClient] = useState(false);
  useEffect(() => setIsClient(true), []);
  const { accessToken, loading: tokenLoading } = useAccessToken();
  const adminStatus = useQuery(api.admin.getAdminStatus);
  const tokenReady = !!accessToken && !tokenLoading;
  const canQuery = isClient && tokenReady && adminStatus?.isAdmin === true;

  // Client-side: redirect to / if Convex says not admin (e.g. different admin list)
  useEffect(() => {
    if (!isClient || tokenLoading) return;
    if (tokenReady && adminStatus && !adminStatus.isAdmin) {
      navigate({ to: '/' });
    }
  }, [isClient, tokenLoading, tokenReady, adminStatus, navigate]);
  const users = useQuery(api.admin.listUsers, canQuery ? {} : 'skip') ?? [];
  const waitlist = useQuery(api.admin.listWaitlist, canQuery ? {} : 'skip') ?? [];
  const setUserAccess = useMutation(api.admin.setUserAccess);
  const setWaitlistApproval = useMutation(api.admin.setWaitlistApproval);
  const [pendingUser, setPendingUser] = useState<string | null>(null);
  const [pendingWaitlist, setPendingWaitlist] = useState<string | null>(null);

  const stats = useMemo(() => {
    const totalUsers = users.length;
    const accessEnabled = users.filter((u) => u.access_enabled).length;
    const waitlistTotal = waitlist.length;
    const waitlistApproved = waitlist.filter((w) => w.approved).length;
    return { totalUsers, accessEnabled, waitlistTotal, waitlistApproved };
  }, [users, waitlist]);

  // Defer full content until after mount so server and client render identical HTML (avoids hydration mismatch with Radix/cache).
  if (!isClient) {
    return (
      <div className="flex min-h-0 flex-1 flex-col gap-6 p-6">
        <Card>
          <CardContent className="p-6">
            <div className="text-xs uppercase tracking-[0.3em] text-muted-foreground">
              Admin Console
            </div>
            <div className="text-2xl font-semibold text-foreground">Access & Waitlist</div>
            <div className="text-sm text-muted-foreground">Signed in as {admin.email ?? admin.id}</div>
            <div className="text-sm text-muted-foreground pt-4">Loading…</div>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-6 p-6">
      <Card>
        <CardContent className="flex flex-col gap-2 p-6">
          <div className="text-xs uppercase tracking-[0.3em] text-muted-foreground">
            Admin Console
          </div>
          <div className="text-2xl font-semibold text-foreground">Access & Waitlist</div>
          <div className="text-sm text-muted-foreground">
            Signed in as {admin.email ?? admin.id}
          </div>
          <div role="none" className="shrink-0 h-px w-full bg-border" />
          <div className="flex flex-wrap gap-6 text-sm text-muted-foreground">
            <div>
              <span className="text-foreground font-medium">{stats.totalUsers}</span> users
            </div>
            <div>
              <span className="text-foreground font-medium">{stats.accessEnabled}</span> access enabled
            </div>
            <div>
              <span className="text-foreground font-medium">{stats.waitlistTotal}</span> waitlist entries
            </div>
            <div>
              <span className="text-foreground font-medium">{stats.waitlistApproved}</span> approved
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardContent className="flex flex-col gap-4 p-6">
          <div className="text-lg font-semibold text-foreground">Users</div>
          <div className="overflow-x-auto">
            <table className="w-full text-left text-sm">
              <thead>
                <tr className="border-b border-border text-xs uppercase tracking-wide text-muted-foreground">
                  <th className="py-2 pr-4">User</th>
                  <th className="py-2 pr-4">Email</th>
                  <th className="py-2 pr-4">Created</th>
                  <th className="py-2 pr-4">Access</th>
                  <th className="py-2 pr-4">Actions</th>
                </tr>
              </thead>
              <tbody>
                {users.map((user) => {
                  const created = new Date(user.created_at).toLocaleString();
                  const access = user.access_enabled;
                  const isPending = pendingUser === user.user_id;
                  return (
                    <tr key={user._id} className="border-b border-border/60">
                      <td className="py-3 pr-4">
                        <div className="font-medium text-foreground">{user.name ?? user.user_id}</div>
                        <div className="text-xs text-muted-foreground">{user.kind}</div>
                      </td>
                      <td className="py-3 pr-4 text-muted-foreground">{user.email ?? '—'}</td>
                      <td className="py-3 pr-4 text-muted-foreground">{created}</td>
                      <td className="py-3 pr-4">
                        <span
                          className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs ${
                            access ? 'bg-emerald-500/15 text-emerald-200' : 'bg-muted text-muted-foreground'
                          }`}
                        >
                          {access ? 'enabled' : 'disabled'}
                        </span>
                      </td>
                      <td className="py-3 pr-4">
                        <Button
                          size="sm"
                          variant={access ? 'secondary' : 'default'}
                          disabled={isPending}
                          onClick={async () => {
                            setPendingUser(user.user_id);
                            try {
                              await setUserAccess({
                                user_id: user.user_id,
                                enabled: !access,
                              });
                            } finally {
                              setPendingUser(null);
                            }
                          }}
                        >
                          {access ? 'Disable' : 'Enable'}
                        </Button>
                      </td>
                    </tr>
                  );
                })}
                {users.length === 0 && (
                  <tr>
                    <td className="py-4 text-muted-foreground" colSpan={5}>
                      {canQuery ? 'No users yet.' : 'Admin access required.'}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardContent className="flex flex-col gap-4 p-6">
          <div className="text-lg font-semibold text-foreground">Waitlist</div>
          <div className="overflow-x-auto">
            <table className="w-full text-left text-sm">
              <thead>
                <tr className="border-b border-border text-xs uppercase tracking-wide text-muted-foreground">
                  <th className="py-2 pr-4">Email</th>
                  <th className="py-2 pr-4">Source</th>
                  <th className="py-2 pr-4">Requested</th>
                  <th className="py-2 pr-4">Status</th>
                  <th className="py-2 pr-4">Actions</th>
                </tr>
              </thead>
              <tbody>
                {waitlist.map((entry) => {
                  const created = new Date(entry.created_at).toLocaleString();
                  const approved = entry.approved;
                  const isPending = pendingWaitlist === entry.email;
                  return (
                    <tr key={entry._id} className="border-b border-border/60">
                      <td className="py-3 pr-4 font-medium text-foreground">{entry.email}</td>
                      <td className="py-3 pr-4 text-muted-foreground">{entry.source ?? '—'}</td>
                      <td className="py-3 pr-4 text-muted-foreground">{created}</td>
                      <td className="py-3 pr-4">
                        <span
                          className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs ${
                            approved ? 'bg-emerald-500/15 text-emerald-200' : 'bg-amber-500/15 text-amber-200'
                          }`}
                        >
                          {approved ? 'approved' : 'pending'}
                        </span>
                      </td>
                      <td className="py-3 pr-4">
                        <Button
                          size="sm"
                          variant={approved ? 'secondary' : 'default'}
                          disabled={isPending}
                          onClick={async () => {
                            setPendingWaitlist(entry.email);
                            try {
                              await setWaitlistApproval({
                                email: entry.email,
                                approved: !approved,
                              });
                            } finally {
                              setPendingWaitlist(null);
                            }
                          }}
                        >
                          {approved ? 'Revoke' : 'Approve'}
                        </Button>
                      </td>
                    </tr>
                  );
                })}
                {waitlist.length === 0 && (
                  <tr>
                    <td className="py-4 text-muted-foreground" colSpan={5}>
                      {canQuery ? 'No waitlist entries yet.' : 'Admin access required.'}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

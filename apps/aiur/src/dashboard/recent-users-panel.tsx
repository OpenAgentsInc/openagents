import { useEffect, useState } from 'react'

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { fetchCreditsUsers, type CreditsUserSummary } from '@/credits/credits-api-client'

const usdCentsToDisplay = (cents: number): string => `$${(cents / 100).toFixed(2)}`

/**
 * Recent signups, most-recent-first, with $10 signup-credit status and
 * current balance — backed by the same `GET /api/admin/credits/users` route
 * as the credits console's search (`ORDER BY users.created_at DESC` on the
 * main Worker, see `admin-credits-routes.ts`), just called with no query so
 * it returns the default recent page instead of a filtered search. Extracted
 * from `ops-console.tsx`'s `UsersPanel` (AIUR-3, #8501) so the SAME panel
 * renders on both the owner homepage and the ops page rather than forking
 * two copies of the same list.
 */
export function RecentUsersPanel() {
  const [users, setUsers] = useState<ReadonlyArray<CreditsUserSummary>>([])
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading')

  useEffect(() => {
    let cancelled = false
    void (async () => {
      const result = await fetchCreditsUsers()
      if (cancelled) return
      if (!result.ok) {
        setStatus('error')
        return
      }
      setUsers(result.value.users)
      setStatus('ready')
    })()
    return () => {
      cancelled = true
    }
  }, [])

  return (
    <Card data-testid="recent-users-panel">
      <CardHeader>
        <CardTitle>Recent signups</CardTitle>
        <CardDescription>
          Who signed up, $10 signup-credit status, and current balance. Click a
          row to open their credits page.
        </CardDescription>
      </CardHeader>
      <CardContent>
        {status === 'loading' && <p className="text-sm text-khala-text-faint">Loading...</p>}
        {status === 'error' && <p className="text-sm text-khala-danger">Failed to load.</p>}
        <ul className="grid gap-1" data-testid="recent-users-list">
          {status === 'ready' && users.length === 0 && (
            <li className="text-sm text-khala-text-faint">No signups yet.</li>
          )}
          {users.map(user => (
            <li key={user.userId}>
              <a
                className="khala-focus flex w-full items-center justify-between border border-khala-border bg-khala-surface-raised px-3 py-2 text-left font-mono text-sm text-khala-text no-underline hover:bg-white/5"
                data-testid={`recent-users-row-${user.userId}`}
                href={`/credits?userId=${encodeURIComponent(user.userId)}`}
              >
                <span>
                  {user.displayName}
                  {user.githubLogin !== null ? ` (@${user.githubLogin})` : ''}
                  <span className="ml-2 text-xs text-khala-text-faint">
                    {user.hasSignupCreditGrant ? 'signup credit granted' : 'no signup credit'}
                  </span>
                </span>
                <span>{usdCentsToDisplay(user.balanceUsdCents)}</span>
              </a>
            </li>
          ))}
        </ul>
      </CardContent>
    </Card>
  )
}

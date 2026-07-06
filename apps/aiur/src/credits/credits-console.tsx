import { useEffect, useReducer, useState } from 'react'

import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'

import { creditsActionReducer, initialCreditsActionState } from './credits-action-state'
import {
  type CreditsBalance,
  type CreditsHistory,
  type CreditsUserSummary,
  type RecentGrant,
  fetchCreditsBalance,
  fetchCreditsHistory,
  fetchCreditsUsers,
  fetchRecentGrants,
  mintCreditsActionRef,
  submitCreditsClawback,
  submitCreditsGrant,
} from './credits-api-client'

const usdCentsToDisplay = (cents: number): string => `$${(cents / 100).toFixed(2)}`

type SelectedTarget = Readonly<{ userId: string; githubLogin: string | null; displayName: string }>

function UserSearch({
  onSelect,
}: {
  onSelect: (target: SelectedTarget) => void
}) {
  const [query, setQuery] = useState('')
  const [results, setResults] = useState<ReadonlyArray<CreditsUserSummary>>([])
  const [status, setStatus] = useState<'idle' | 'loading' | 'error'>('idle')

  const runSearch = async () => {
    setStatus('loading')
    const result = await fetchCreditsUsers(query)
    if (!result.ok) {
      setStatus('error')
      return
    }
    setResults(result.value.users)
    setStatus('idle')
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Find a user</CardTitle>
        <CardDescription>Search by GitHub login, user id, or display name.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-3">
        <div className="flex gap-2">
          <input
            className="min-h-10 flex-1 border border-khala-border bg-black px-3 py-2 font-mono text-sm text-khala-text outline-none placeholder:text-khala-text-faint"
            data-testid="credits-search-input"
            onChange={event => setQuery(event.target.value)}
            onKeyDown={event => {
              if (event.key === 'Enter') void runSearch()
            }}
            placeholder="octocat, user_abc123, ..."
            value={query}
          />
          <Button data-testid="credits-search-button" onClick={() => void runSearch()}>
            Search
          </Button>
        </div>
        {status === 'error' && (
          <p className="text-sm text-khala-danger">Search failed. Try again.</p>
        )}
        <ul className="grid gap-1" data-testid="credits-search-results">
          {results.map(user => (
            <li key={user.userId}>
              <button
                className="khala-focus flex w-full items-center justify-between border border-khala-border bg-khala-surface-raised px-3 py-2 text-left font-mono text-sm text-khala-text hover:bg-white/5"
                data-testid={`credits-search-result-${user.userId}`}
                onClick={() =>
                  onSelect({
                    displayName: user.displayName,
                    githubLogin: user.githubLogin,
                    userId: user.userId,
                  })
                }
                type="button"
              >
                <span>
                  {user.displayName}
                  {user.githubLogin !== null ? ` (@${user.githubLogin})` : ''}
                </span>
                <span className="text-xs text-khala-text-faint">
                  {user.hasSignupCreditGrant ? 'signup credit ' : ''}
                  {user.hasAdminCreditGrant ? 'admin grant' : ''}
                </span>
              </button>
            </li>
          ))}
        </ul>
      </CardContent>
    </Card>
  )
}

function GrantForm({
  target,
  onGranted,
}: {
  target: SelectedTarget
  onGranted: () => void
}) {
  const [amountUsd, setAmountUsd] = useState('10.00')
  const [reason, setReason] = useState('')
  const [action, dispatch] = useReducer(creditsActionReducer, initialCreditsActionState)

  const amountUsdCents = Math.round(Number.parseFloat(amountUsd || '0') * 100)
  const canSubmit = Number.isFinite(amountUsdCents) && amountUsdCents > 0 && reason.trim().length > 0

  const confirmGrant = async (actionRef: string) => {
    dispatch({ type: 'submit' })
    const result = await submitCreditsGrant({
      amountUsdCents,
      githubLogin: target.githubLogin ?? undefined,
      grantRef: actionRef,
      reason: reason.trim(),
      userId: target.userId,
    })
    if (!result.ok) {
      dispatch({ messageSafe: result.messageSafe, type: 'fail' })
      return
    }
    dispatch({ messageSafe: 'Granted.', type: 'succeed' })
    onGranted()
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Grant credit</CardTitle>
        <CardDescription>
          Every grant is receipted and requires a reason. Manual grants replace
          IAP for the first MVP build.
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-3">
        <label className="grid gap-1 text-sm text-khala-text-muted">
          <span>Amount (USD)</span>
          <input
            className="min-h-10 border border-khala-border bg-black px-3 py-2 font-mono text-sm text-khala-text outline-none"
            data-testid="grant-amount-input"
            inputMode="decimal"
            onChange={event => setAmountUsd(event.target.value)}
            value={amountUsd}
          />
        </label>
        <label className="grid gap-1 text-sm text-khala-text-muted">
          <span>Reason</span>
          <input
            className="min-h-10 border border-khala-border bg-black px-3 py-2 font-mono text-sm text-khala-text outline-none"
            data-testid="grant-reason-input"
            onChange={event => setReason(event.target.value)}
            placeholder="Beta tester welcome credit"
            value={reason}
          />
        </label>
        {action.status === 'idle' && (
          <Button
            data-testid="grant-start-button"
            disabled={!canSubmit}
            onClick={() =>
              dispatch({ actionRef: mintCreditsActionRef(), type: 'start_confirm' })
            }
          >
            Grant {usdCentsToDisplay(Number.isFinite(amountUsdCents) ? amountUsdCents : 0)}
          </Button>
        )}
        {(action.status === 'confirming' ||
          action.status === 'submitting' ||
          action.status === 'error') && (
          <div className="grid gap-2 border border-khala-warning/45 bg-khala-surface-raised p-3">
            <p className="text-sm text-khala-text">
              Confirm: grant {usdCentsToDisplay(amountUsdCents)} to {target.displayName} —
              {' '}
              &ldquo;{reason.trim()}&rdquo;
            </p>
            <div className="flex gap-2">
              <Button
                data-testid="grant-confirm-button"
                disabled={action.status === 'submitting'}
                onClick={() => {
                  // A retry from 'error' must transition error -> confirming
                  // BEFORE 'submit' is dispatched (submit only fires from
                  // 'confirming') — see credits-action-state.ts.
                  if (action.status === 'error') dispatch({ type: 'retry' })
                  void confirmGrant(action.actionRef)
                }}
              >
                {action.status === 'submitting'
                  ? 'Granting...'
                  : action.status === 'error'
                    ? 'Retry grant'
                    : 'Confirm grant'}
              </Button>
              <Button
                data-testid="grant-cancel-button"
                onClick={() => dispatch({ type: 'cancel' })}
                variant="secondary"
              >
                Cancel
              </Button>
            </div>
          </div>
        )}
        {action.status === 'error' && (
          <p className="text-sm text-khala-danger" data-testid="grant-error">
            {action.messageSafe}
          </p>
        )}
        {action.status === 'success' && (
          <p className="text-sm text-khala-success" data-testid="grant-success">
            {action.messageSafe}
          </p>
        )}
      </CardContent>
    </Card>
  )
}

function ClawbackForm({ target }: { target: SelectedTarget }) {
  const [amountUsd, setAmountUsd] = useState('')
  const [reason, setReason] = useState('')
  const [action, dispatch] = useReducer(creditsActionReducer, initialCreditsActionState)

  const amountUsdCents = Math.round(Number.parseFloat(amountUsd || '0') * 100)
  const canSubmit = Number.isFinite(amountUsdCents) && amountUsdCents > 0 && reason.trim().length > 0

  const confirmClawback = async (actionRef: string) => {
    dispatch({ type: 'submit' })
    const result = await submitCreditsClawback({
      amountUsdCents,
      clawbackRef: actionRef,
      githubLogin: target.githubLogin ?? undefined,
      reason: reason.trim(),
      userId: target.userId,
    })
    if (!result.ok) {
      dispatch({ messageSafe: result.messageSafe, type: 'fail' })
      return
    }
    dispatch({
      messageSafe: result.value.insufficientBalance
        ? 'Balance too low to fully claw back.'
        : 'Clawed back.',
      type: 'succeed',
    })
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Claw back credit</CardTitle>
        <CardDescription>Never goes balance-negative; refuses over-clawback.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-3">
        <label className="grid gap-1 text-sm text-khala-text-muted">
          <span>Amount (USD)</span>
          <input
            className="min-h-10 border border-khala-border bg-black px-3 py-2 font-mono text-sm text-khala-text outline-none"
            data-testid="clawback-amount-input"
            inputMode="decimal"
            onChange={event => setAmountUsd(event.target.value)}
            value={amountUsd}
          />
        </label>
        <label className="grid gap-1 text-sm text-khala-text-muted">
          <span>Reason</span>
          <input
            className="min-h-10 border border-khala-border bg-black px-3 py-2 font-mono text-sm text-khala-text outline-none"
            data-testid="clawback-reason-input"
            onChange={event => setReason(event.target.value)}
            placeholder="Chargeback / refund"
            value={reason}
          />
        </label>
        {action.status === 'idle' && (
          <Button
            data-testid="clawback-start-button"
            disabled={!canSubmit}
            onClick={() =>
              dispatch({ actionRef: mintCreditsActionRef(), type: 'start_confirm' })
            }
            variant="destructive"
          >
            Claw back
          </Button>
        )}
        {(action.status === 'confirming' ||
          action.status === 'submitting' ||
          action.status === 'error') && (
          <div className="grid gap-2 border border-khala-danger/45 bg-khala-surface-raised p-3">
            <p className="text-sm text-khala-text">
              Confirm: claw back {usdCentsToDisplay(amountUsdCents)} from {target.displayName}
            </p>
            <div className="flex gap-2">
              <Button
                data-testid="clawback-confirm-button"
                disabled={action.status === 'submitting'}
                onClick={() => {
                  // Same retry-then-submit ordering as GrantForm — see
                  // credits-action-state.ts.
                  if (action.status === 'error') dispatch({ type: 'retry' })
                  void confirmClawback(action.actionRef)
                }}
                variant="destructive"
              >
                {action.status === 'submitting'
                  ? 'Clawing back...'
                  : action.status === 'error'
                    ? 'Retry clawback'
                    : 'Confirm clawback'}
              </Button>
              <Button
                data-testid="clawback-cancel-button"
                onClick={() => dispatch({ type: 'cancel' })}
                variant="secondary"
              >
                Cancel
              </Button>
            </div>
          </div>
        )}
        {action.status === 'error' && (
          <p className="text-sm text-khala-danger" data-testid="clawback-error">
            {action.messageSafe}
          </p>
        )}
        {action.status === 'success' && (
          <p className="text-sm text-khala-success" data-testid="clawback-success">
            {action.messageSafe}
          </p>
        )}
      </CardContent>
    </Card>
  )
}

function BalanceAndHistory({
  target,
  refreshKey,
}: {
  target: SelectedTarget
  refreshKey: number
}) {
  const [balance, setBalance] = useState<CreditsBalance | undefined>(undefined)
  const [history, setHistory] = useState<CreditsHistory | undefined>(undefined)
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading')

  // Re-fetches whenever the target user OR refreshKey changes (a new
  // grant/clawback landed).
  useEffect(() => {
    let cancelled = false
    setStatus('loading')
    void (async () => {
      const [balanceResult, historyResult] = await Promise.all([
        fetchCreditsBalance({ userId: target.userId }),
        fetchCreditsHistory({ userId: target.userId }),
      ])
      if (cancelled) return
      if (!balanceResult.ok || !historyResult.ok) {
        setStatus('error')
        return
      }
      setBalance(balanceResult.value)
      setHistory(historyResult.value)
      setStatus('ready')
    })()
    return () => {
      cancelled = true
    }
  }, [target.userId, refreshKey])

  return (
    <Card>
      <CardHeader>
        <CardTitle>{target.displayName}&rsquo;s balance</CardTitle>
        <CardDescription>
          {target.githubLogin !== null ? `@${target.githubLogin} — ` : ''}
          {target.userId}
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        {status === 'loading' && <p className="text-sm text-khala-text-faint">Loading...</p>}
        {status === 'error' && <p className="text-sm text-khala-danger">Failed to load.</p>}
        {balance !== undefined && (
          <p className="font-mono text-2xl font-semibold text-khala-text" data-testid="credits-balance-value">
            {usdCentsToDisplay(balance.balance.balanceUsdCents)}
          </p>
        )}
        {history !== undefined && (
          <ul className="grid gap-1" data-testid="credits-history-list">
            {history.history.length === 0 && (
              <li className="text-sm text-khala-text-faint">No grant history yet.</li>
            )}
            {history.history.map(entry => (
              <li
                className="flex items-center justify-between border-b border-khala-border py-1 text-sm text-khala-text"
                key={entry.receiptRef}
              >
                <span>
                  {entry.reason}{' '}
                  <span className="text-khala-text-faint">({entry.kind})</span>
                </span>
                <span>{usdCentsToDisplay(entry.amountUsdCents)}</span>
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  )
}

function RecentGrantsLedger({ refreshKey }: { refreshKey: number }) {
  const [grants, setGrants] = useState<ReadonlyArray<RecentGrant>>([])

  useEffect(() => {
    let cancelled = false
    void (async () => {
      const result = await fetchRecentGrants()
      if (!cancelled && result.ok) setGrants(result.value.grants)
    })()
    return () => {
      cancelled = true
    }
  }, [refreshKey])

  return (
    <Card>
      <CardHeader>
        <CardTitle>Recent grants</CardTitle>
        <CardDescription>Ledger view across all users.</CardDescription>
      </CardHeader>
      <CardContent>
        <ul className="grid gap-1" data-testid="recent-grants-list">
          {grants.length === 0 && (
            <li className="text-sm text-khala-text-faint">No grants yet.</li>
          )}
          {grants.map(grant => (
            <li
              className="flex items-center justify-between border-b border-khala-border py-1 text-sm text-khala-text"
              key={grant.grantRef}
            >
              <span>
                {grant.userId} — {grant.reason}
              </span>
              <span>{usdCentsToDisplay(grant.amountUsdCents)}</span>
            </li>
          ))}
        </ul>
      </CardContent>
    </Card>
  )
}

export function CreditsConsole() {
  const [target, setTarget] = useState<SelectedTarget | undefined>(undefined)
  const [refreshKey, setRefreshKey] = useState(0)

  return (
    <div className="grid gap-6">
      <UserSearch onSelect={setTarget} />
      {target !== undefined && (
        <>
          <BalanceAndHistory key={target.userId} refreshKey={refreshKey} target={target} />
          <GrantForm
            key={`grant-${target.userId}`}
            onGranted={() => setRefreshKey(k => k + 1)}
            target={target}
          />
          <ClawbackForm key={`clawback-${target.userId}`} target={target} />
        </>
      )}
      <RecentGrantsLedger refreshKey={refreshKey} />
    </div>
  )
}

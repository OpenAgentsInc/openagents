import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'

const columnHeaders = [
  'Account',
  'Provider',
  'State',
  'Hourly',
  'Weekly',
  'Reset',
] as const

const eyebrowClass =
  'm-0 font-mono text-xs font-semibold uppercase leading-none tracking-wide text-khala-energy-soft'
const bodyClass = 'm-0 max-w-3xl text-sm/6 text-khala-text-muted'
const codeClass =
  'break-all bg-white/[0.06] px-1.5 py-0.5 font-mono text-[0.85em] text-khala-text'

export function ArtanisAccountsPage() {
  return (
    <main
      aria-label="Operator account observability"
      className="min-h-dvh bg-black text-khala-text"
      data-route="artanis-accounts"
    >
      <div className="mx-auto grid min-h-dvh w-full max-w-6xl gap-6 px-4 py-6 font-mono sm:px-6 lg:px-8">
        <Card className="grid gap-5 p-5 sm:p-6">
          <p className={eyebrowClass}>Artanis / accounts</p>
          <h1 className="m-0 max-w-3xl text-3xl font-semibold leading-tight tracking-normal text-white sm:text-4xl">
            Operator account observability
          </h1>
          <p className={bodyClass}>
            Owner-only status for Codex and Claude coding accounts: live
            cooldowns, usage windows, and manual reset controls.
          </p>
        </Card>

        <Card
          aria-label="Operator account observability dashboard"
          className="grid gap-4 p-4 shadow-xl shadow-black/20 sm:p-5"
        >
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant="warning">Unauthorized</Badge>
            <span className="text-sm/5 text-khala-text-muted">
              This owner-only account dashboard is not available without an
              owner session.
            </span>
          </div>
          <div
            className="grid grid-cols-2 gap-2 text-xs font-semibold uppercase leading-none tracking-wide text-khala-text-faint sm:grid-cols-6"
            role="row"
          >
            {columnHeaders.map(header => (
              <span key={header}>{header}</span>
            ))}
          </div>
          <p className="m-0 text-sm/5 text-khala-text-muted">
            No operator account rows are available.
          </p>
        </Card>

        <Card className="grid gap-3 p-4 sm:p-5">
          <p className={eyebrowClass}>Live surface</p>
          <p className={bodyClass}>
            Live status, refresh, and manual reset controls remain on the
            operator API and the existing operator page until this route
            carries real owner-session auth.
          </p>
          <div className="grid gap-3 border border-khala-border bg-black/25 p-3 text-sm/5 text-khala-text-muted sm:grid-cols-2">
            <div>
              <span className="block text-khala-text-faint">Status</span>
              <code className={codeClass}>
                /api/operator/accounts/status
              </code>
            </div>
            <div>
              <span className="block text-khala-text-faint">Reset</span>
              <code className={codeClass}>/api/operator/accounts/reset</code>
            </div>
          </div>
        </Card>

        <Card className="grid gap-2 p-4 sm:p-5">
          <p className="m-0 max-w-[76ch] text-xs leading-5 text-khala-text-faint">
            This surface is operator evidence and control only. It does not
            grant dispatch, spend, settlement, provider-account ownership
            transfer, or cross-owner routing authority.
          </p>
        </Card>
      </div>
    </main>
  )
}

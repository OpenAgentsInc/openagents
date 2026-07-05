import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'

const eyebrowClass =
  'm-0 font-mono text-xs font-semibold uppercase leading-none tracking-wide text-khala-text-faint'
const panelTitleClass =
  'm-0 font-mono text-sm font-semibold uppercase leading-none text-khala-text'
const panelMetaClass = 'm-0 mt-1 font-mono text-xs leading-4 text-khala-text-faint'
const rowClass =
  'grid grid-cols-[minmax(0,1fr)_auto] gap-3 border-t border-khala-border/60 py-2'
const rowLabelClass = 'min-w-0 font-mono text-xs font-medium leading-4 text-khala-text'
const rowDetailClass = 'mt-1 font-mono text-[0.66rem] leading-4 text-khala-text-faint'
const rowValueClass = 'text-right font-mono text-xs leading-4 tabular-nums text-khala-text'
const navLinkClass =
  'khala-focus font-mono text-xs font-semibold uppercase leading-none text-khala-text-faint hover:text-khala-text'
const codeClass =
  'break-all bg-white/[0.06] px-1.5 py-0.5 font-mono text-[0.85em] text-khala-text'

function PanelHeader({
  meta,
  status,
  title,
}: Readonly<{ meta?: string; status?: string; title: string }>) {
  return (
    <div className="mb-2 flex min-w-0 flex-wrap items-start justify-between gap-2">
      <div className="min-w-0">
        <h2 className={panelTitleClass}>{title}</h2>
        {meta === undefined ? null : <p className={panelMetaClass}>{meta}</p>}
      </div>
      {status === undefined ? null : <Badge>{status}</Badge>}
    </div>
  )
}

function MetricRow({
  detail,
  label,
  value,
}: Readonly<{ detail: string; label: string; value: string }>) {
  return (
    <div className={rowClass}>
      <div className="min-w-0">
        <div className={rowLabelClass}>{label}</div>
        <div className={rowDetailClass}>{detail}</div>
      </div>
      <div className={rowValueClass}>{value}</div>
    </div>
  )
}

const stateRows = [
  { label: 'GREEN', detail: 'Waiting for live registry.' },
  { label: 'YELLOW', detail: 'Waiting for live registry.' },
  { label: 'RED', detail: 'Waiting for live registry.' },
  { label: 'DEGRADED', detail: 'Waiting for live registry.' },
  { label: 'PLANNED', detail: 'Waiting for live registry.' },
  { label: 'WITHDRAWN', detail: 'Waiting for live registry.' },
] as const

// `openagents.com/promises` — the human-readable product-promise ledger
// (docs/promises/). The Foldkit original
// (apps/web/src/page/loggedOut/page/promises.ts) drives two client-fetched
// models: `PublicProductPromisesModel` against /api/public/product-promises
// and `PublicPromiseTransitionsModel` (the claim-upgrade-receipt audit panel)
// against /api/public/product-promises/transitions. Every prior TS-6 Start
// route has stayed static/SSR-only, so this port keeps that posture: the
// always-static hero, nav, and panel structure is preserved, and every panel
// renders the exact honest "nothing fetched yet" copy the Foldkit view
// already shows when both models are null — Version/Promises "Loading",
// state-map rows "Waiting for live registry.", "None listed." for blocker
// refs, and so on — rather than fabricating registry rows, receipts, or
// wiring a first live fetch on a standalone page.
export function PromisesPage() {
  return (
    <main className="min-h-dvh bg-black text-khala-text" data-route="promises">
      <div className="mx-auto w-full max-w-7xl px-4 py-4 font-mono sm:px-5 lg:px-6">
        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-khala-border pb-3">
          <a
            className="khala-focus font-mono text-xs font-semibold uppercase tracking-normal text-khala-text"
            href="/"
          >
            OpenAgents
          </a>
          <nav className="flex flex-wrap items-center gap-4">
            <a className={navLinkClass} href="/docs/product-promises">
              Docs
            </a>
            <a className={navLinkClass} href="/api/public/product-promises">
              JSON
            </a>
            <a className={navLinkClass} href="/forum/f/product-promises">
              Forum
            </a>
          </nav>
        </div>

        <div className="grid min-w-0 gap-4 pt-8 lg:grid-cols-[minmax(0,1.3fr)_minmax(300px,0.7fr)]">
          <div className="min-w-0">
            <p className={eyebrowClass}>Human-readable promise ledger</p>
            <h1 className="m-0 mt-4 max-w-3xl text-[clamp(2.25rem,5vw,4.75rem)] font-semibold leading-[0.96] tracking-normal text-white">
              Product promises
            </h1>
            <p className="m-0 mt-5 max-w-2xl text-[0.92rem] leading-6 text-khala-text-muted">
              A visual map of what OpenAgents says it does, what is live, what
              is gated, and what should be reported when reality does not
              match the claim.
            </p>
          </div>
          <Card
            className="min-w-0 border-khala-border bg-khala-surface p-3 text-left"
            data-promises-registry-status=""
          >
            <PanelHeader
              meta="Backed by /api/public/product-promises."
              status="Idle"
              title="Registry Status"
            />
            <MetricRow detail="Versioned public JSON." label="Version" value="Loading" />
            <MetricRow
              detail="Current records in the live registry."
              label="Promises"
              value="Loading"
            />
            <MetricRow
              detail="Default path for loose reports and stale-copy notes."
              label="Report path"
              value="Forum"
            />
          </Card>
        </div>
      </div>

      <div className="mx-auto grid min-w-0 w-full max-w-7xl gap-3 px-4 py-4 font-mono sm:px-5 lg:grid-cols-3 lg:px-6">
        <Card className="min-w-0 border-khala-border bg-khala-surface p-3 text-left">
          <PanelHeader
            meta="Green is live. Yellow is scoped. Red is blocked. Withdrawn is historical."
            title="State Map"
          />
          {stateRows.map(row => (
            <MetricRow detail={row.detail} key={row.label} label={row.label} value="-" />
          ))}
        </Card>
        <Card className="min-w-0 border-khala-border bg-khala-surface p-3 text-left">
          <PanelHeader meta="Grouped by current product area." title="Product Areas" />
          <MetricRow
            detail="Waiting for /api/public/product-promises."
            label="Loading"
            value="-"
          />
        </Card>
        <Card className="min-w-0 border-khala-border bg-khala-surface p-3 text-left">
          <PanelHeader status="Loading" title="Current Caveats" />
          <p className={`${panelMetaClass} border-t border-khala-border/60 pt-2`}>
            Waiting for the live caveat list.
          </p>
        </Card>
      </div>

      <div className="mx-auto w-full max-w-7xl px-4 py-4 font-mono sm:px-5 lg:px-6">
        <Card className="min-w-0 border-khala-border bg-khala-surface p-3 text-left">
          <PanelHeader
            meta="At-a-glance blocker and evidence inventory from the live JSON."
            status="Loading"
            title="Blockers And Evidence"
          />
          <MetricRow detail="Yellow, red, degraded, or planned records." label="Not green" value="-" />
          <MetricRow
            detail="Unique blocker refs across the registry."
            label="Unique blockers"
            value="-"
          />
          <MetricRow
            detail="Evidence refs attached to promise records."
            label="Evidence refs"
            value="-"
          />
          <div className="border-t border-khala-border/60 pt-3">
            <p className="m-0 font-mono text-[0.62rem] font-semibold uppercase leading-none text-khala-text-faint">
              Top blocker refs
            </p>
            <p className="m-0 mt-2 font-mono text-[0.68rem] leading-5 text-khala-text-faint">
              None listed.
            </p>
          </div>
        </Card>
      </div>

      <div className="mx-auto w-full max-w-7xl px-4 py-4 font-mono sm:px-5 lg:px-6">
        <div className="mb-3 flex min-w-0 flex-wrap items-end justify-between gap-3 border-t border-khala-border pt-4">
          <div className="min-w-0">
            <p className="m-0 font-mono text-xs font-semibold uppercase leading-none text-khala-text-faint">
              proof.claim_upgrade_receipts.v1
            </p>
            <h2 className="m-0 mt-2 text-xl font-semibold leading-6 text-white">
              Claim-upgrade audit panel
            </h2>
            <p className={`${panelMetaClass} mt-1 max-w-3xl`}>
              Every promise state change traces to a dereferenceable,
              registry-versioned transition receipt: the before/after state,
              the mechanical checks it passed, and the owner signoff. A
              passing receipt is evidence for a flip, never the flip itself.
            </p>
          </div>
          <Badge>Idle</Badge>
        </div>

        <Card className="mb-3 min-w-0 border-khala-border bg-khala-surface p-3 text-left">
          <PanelHeader
            meta="Receipt-backing tally over the live transition receipt feed."
            status="Idle"
            title="Audit Summary"
          />
          <MetricRow
            detail="Every state change is recorded as one receipt."
            label="Transition receipts"
            value="-"
          />
          <MetricRow detail="Receipts whose target state is green." label="Green flips" value="-" />
          <MetricRow
            detail="Green flips backed by a passing or owner-signed receipt."
            label="Green flips receipt-backed"
            value="-"
          />
          <MetricRow
            detail="Owner-authorized policy-exception receipts."
            label="Owner-signed exceptions"
            value="-"
          />
        </Card>

        <div className="mb-3 flex flex-wrap items-center gap-4">
          <a className={navLinkClass} href="/api/public/product-promises/transitions">
            Receipt feed JSON
          </a>
          <a className={navLinkClass} href="/api/public/product-promises/audit">
            Enterprise audit projection JSON
          </a>
        </div>

        <Card className="min-w-0 border-khala-border bg-khala-surface p-3 text-left">
          <PanelHeader
            meta="Waiting for /api/public/product-promises/transitions."
            status="Idle"
            title="Transition Receipts"
          />
        </Card>
      </div>

      <div className="mx-auto w-full max-w-7xl px-4 py-4 pb-10 font-mono sm:px-5 lg:px-6">
        <div className="mb-3 flex min-w-0 flex-wrap items-end justify-between gap-3 border-t border-khala-border pt-4">
          <div className="min-w-0">
            <h2 className="m-0 text-lg font-semibold leading-6 text-white">
              Promise records
            </h2>
            <p className={panelMetaClass}>Cards render from the live public endpoint.</p>
          </div>
          <Badge>Idle</Badge>
        </div>
        <Card className="min-w-0 border-khala-border bg-khala-surface p-3 text-left">
          <PanelHeader
            meta="The browser is waiting for /api/public/product-promises."
            status="Idle"
            title="Promise Records"
          />
        </Card>
      </div>

      <div className="mx-auto w-full max-w-7xl px-4 pb-8 font-mono sm:px-5 lg:px-6">
        <Card className="grid gap-3 border-khala-border bg-khala-surface p-4 sm:p-5">
          <p className={eyebrowClass}>Live surface</p>
          <p className="m-0 max-w-3xl text-sm/6 text-khala-text-muted">
            Live registry rows, state counts, and receipt-backed audit history
            stay on the existing Foldkit page until this route carries a real
            fetch against the endpoints below.
          </p>
          <div className="grid gap-3 border border-khala-border bg-black/25 p-3 text-sm/5 text-khala-text-muted sm:grid-cols-2">
            <div>
              <span className="block text-khala-text-faint">Promises</span>
              <code className={codeClass}>/api/public/product-promises</code>
            </div>
            <div>
              <span className="block text-khala-text-faint">Transitions</span>
              <code className={codeClass}>
                /api/public/product-promises/transitions
              </code>
            </div>
            <div>
              <span className="block text-khala-text-faint">Audit projection</span>
              <code className={codeClass}>/api/public/product-promises/audit</code>
            </div>
            <div>
              <span className="block text-khala-text-faint">Docs</span>
              <code className={codeClass}>/docs/product-promises</code>
            </div>
          </div>
        </Card>
      </div>
    </main>
  )
}

import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'

const eyebrowClass =
  'm-0 font-mono text-[0.72rem] font-semibold uppercase leading-none text-khala-text-faint'
const panelTitleClass =
  'm-0 font-mono text-[0.72rem] font-semibold uppercase leading-none text-khala-text'
const panelMetaClass = 'm-0 font-mono text-[0.68rem] leading-4 text-khala-text-faint'

function PanelHeader({
  meta,
  status,
  title,
}: Readonly<{ meta?: string; status?: string; title: string }>) {
  return (
    <div className="mb-3 flex items-start justify-between gap-3">
      <div>
        <h2 className={panelTitleClass}>{title}</h2>
        {meta === undefined ? null : <p className={panelMetaClass}>{meta}</p>}
      </div>
      {status === undefined ? null : <Badge>{status}</Badge>}
    </div>
  )
}

// `openagents.com/training/runs` — the public CS336 run-state listing. The
// Foldkit original (apps/web/src/page/loggedOut/page/trainingRuns.ts) drives
// a `PublicTrainingRunsModel` union (`Idle` / `Loading` / `Loaded` / `Failed`)
// fed by a client fetch against `/api/training/runs`. Every prior TS-6 Start
// route has stayed static/SSR-only, so this port keeps that posture and
// renders the model's own `PublicTrainingRunsIdle` state honestly — the same
// "No Worker-authoritative training runs are recorded yet" empty copy the
// Foldkit view already shows before its fetch resolves — rather than
// fabricating run rows or being first to wire live fetch on a standalone
// page. The `/training/runs/$runId` run-detail route (and each run's Real
// Gradient / windows / receipts panels) stays on the existing Foldkit page
// until this route carries live data.
export function TrainingRunsPage() {
  return (
    <main className="min-h-dvh bg-black text-khala-text" data-route="training-runs">
      <div className="mx-auto w-full max-w-7xl px-4 py-4 sm:px-5 lg:px-6">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div>
            <h1 className="m-0 text-2xl font-semibold leading-tight text-khala-text">
              Training Runs
            </h1>
            <p className={panelMetaClass}>
              Public CS336 run state, verification, and settlement
              projection.
            </p>
          </div>
          <Badge>Idle</Badge>
        </div>
        <p className={`${panelMetaClass} mt-1`}>Backed by /api/training/runs.</p>
      </div>
      <div className="mx-auto grid w-full max-w-7xl grid-cols-1 gap-3 px-4 pb-6 sm:px-5 lg:grid-cols-[20rem_minmax(0,1fr)] lg:px-6">
        <Card className="min-w-0 p-3 text-left">
          <PanelHeader
            title="Public Runs"
            meta="No Worker-authoritative training runs are recorded yet."
            status="0 runs"
          />
        </Card>
        <Card className="min-w-0 p-3 text-left">
          <PanelHeader
            title="Run Detail"
            meta="No run projection is available for this route."
            status="Idle"
          />
        </Card>
      </div>
      <div className="mx-auto w-full max-w-7xl px-4 pb-8 sm:px-5 lg:px-6">
        <Card className="grid gap-2 p-4 sm:p-5">
          <p className={eyebrowClass}>Live surface</p>
          <p className="m-0 max-w-3xl text-sm/6 text-khala-text-muted">
            Live run rows, leaderboards, and the per-run Real Gradient status
            stay on the existing operator-facing Foldkit page until this
            route carries a real fetch against the endpoints below.
          </p>
          <div className="grid gap-3 border border-khala-border bg-black/25 p-3 text-sm/5 text-khala-text-muted sm:grid-cols-2">
            <div>
              <span className="block text-khala-text-faint">Runs</span>
              <code className="break-all bg-white/[0.06] px-1.5 py-0.5 font-mono text-[0.85em] text-khala-text">
                /api/training/runs
              </code>
            </div>
            <div>
              <span className="block text-khala-text-faint">Leaderboards</span>
              <code className="break-all bg-white/[0.06] px-1.5 py-0.5 font-mono text-[0.85em] text-khala-text">
                /api/training/leaderboards
              </code>
            </div>
          </div>
        </Card>
      </div>
    </main>
  )
}

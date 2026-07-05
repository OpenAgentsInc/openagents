import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'

const eyebrowClass =
  'm-0 font-mono text-xs font-semibold uppercase leading-none tracking-wide text-khala-energy-soft'
const bodyClass = 'm-0 max-w-3xl text-sm/6 text-khala-text-muted'
const panelTitleClass =
  'm-0 font-mono text-sm font-semibold uppercase leading-none text-khala-text'
const panelMetaClass = 'm-0 mt-1 font-mono text-xs leading-4 text-khala-text-faint'
const codeClass =
  'break-all bg-white/[0.06] px-1.5 py-0.5 font-mono text-[0.85em] text-khala-text'

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

const launchIntentLines = [
  'POST /api/gym/mirrorcode/runs',
  'Authorization: Bearer <admin-token>',
  '',
  '{',
  '  "kind": "launch",',
  '  "taskId": "cal",',
  '  "bucket": "S",',
  '  "language": "python",',
  '  "grade": "smoke"',
  '}',
]

const statusReadLines = [
  'GET /api/gym/mirrorcode/runs/{runId}',
  'GET /api/gym/mirrorcode/runs',
  '',
  'returns:',
  '  status, passRate, tokensTotal,',
  '  exactTokenUsageEventRefs,',
  '  tokenAttributionProofRef',
]

function CodeBlock({
  label,
  lines,
}: Readonly<{ label: string; lines: ReadonlyArray<string> }>) {
  return (
    <figure className="m-0 grid gap-2 border border-khala-border bg-black p-3">
      <figcaption className="font-mono text-xs font-semibold uppercase tracking-wide text-khala-text-faint">
        {label}
      </figcaption>
      <pre className="m-0 overflow-x-auto whitespace-pre font-mono text-xs leading-5 text-khala-text-muted">
        {lines.join('\n')}
      </pre>
    </figure>
  )
}

// `openagents.com/mirrorcode` — "MirrorCode, powered by Khala" (#6378). The
// Foldkit original (apps/web/src/page/loggedOut/page/mirrorcode.ts) drives a
// `MirrorCodeRunsModel` union (`Idle` / `Loading` / `Loaded` / `Failed`) fed by
// a client fetch against /api/gym/mirrorcode/runs. Every prior TS-6 Start
// route has stayed static/SSR-only, so this port keeps that posture: the
// always-static hero and playground-contract copy is preserved in full (it
// does not depend on the fetch), and the three data-dependent panels (Live
// run, Execution visualizer, Leaderboard) render the model's own
// `MirrorCodeRunsIdle` empty-state copy honestly — the same text the Foldkit
// view already shows before its fetch resolves — rather than fabricating run
// rows or wiring a first live fetch on a standalone page.
export function MirrorCodePage() {
  return (
    <main className="min-h-dvh bg-black text-khala-text" data-route="mirrorcode">
      <div className="mx-auto grid min-w-0 w-full max-w-7xl gap-6 px-4 py-8 font-mono sm:px-6 lg:px-8">
        <header className="grid gap-3">
          <div
            className="w-fit border border-khala-energy/30 bg-khala-energy/10 px-3 py-1 text-xs font-semibold uppercase tracking-wide text-khala-energy-soft"
            data-mirrorcode-no-spend-banner=""
          >
            Live data only / public tasks only
          </div>
          <h1 className="m-0 text-3xl font-semibold tracking-tight text-balance text-white sm:text-5xl">
            MirrorCode, powered by Khala
          </h1>
          <p className={bodyClass}>
            Khala (openagents/khala) reimplements real tools from scratch
            inside a sandbox, then a held-out test suite scores the result.
            The benchmark is the Epoch Research MirrorCode set, run here on
            PUBLIC tasks only — the private set is excluded — so every number
            below is reproducible and never reads the held-out answers.
          </p>
        </header>

        <div
          className="border border-khala-border bg-black px-3 py-2 text-xs text-khala-text-muted"
          data-mirrorcode-benchmark-strip=""
        >
          Benchmark: Epoch Research MirrorCode · scope: public tasks only
          (private set excluded) · model: openagents/khala
        </div>

        <Card
          className="grid min-w-0 gap-4 border-khala-border bg-khala-surface p-4"
          data-mirrorcode-playground-panel=""
        >
          <div className="grid gap-2 lg:grid-cols-[minmax(0,1fr)_18rem] lg:items-start">
            <div className="grid gap-2">
              <p className={eyebrowClass}>MirrorCode-as-a-Service playground</p>
              <h2 className="m-0 max-w-[26ch] text-2xl font-semibold tracking-tight text-balance text-white sm:text-3xl">
                Queue a public-task run, then read status by run id
              </h2>
              <p className={bodyClass}>
                The public playground shows the exact API surface without
                opening public dispatch. Launch is owner-gated; status and
                leaderboard reads are public-safe and never expose task
                source, prompts, logs, canary strings, keys, or private-set
                answers.
              </p>
            </div>
            <div
              className="grid gap-1 border border-khala-warning/30 bg-khala-warning/10 p-3 text-xs text-khala-warning"
              data-mirrorcode-owner-gated-launch=""
            >
              <span className="font-semibold uppercase tracking-wide">
                Owner-gated launch
              </span>
              <span>
                POST requires an admin bearer token; public visitors can
                inspect the contract and read results only.
              </span>
            </div>
          </div>
          <div className="grid gap-3 md:grid-cols-3">
            <div
              className="grid content-start gap-2 border border-khala-border bg-black p-3"
              data-mirrorcode-playground-step="target"
            >
              <h3 className="m-0 text-sm font-semibold text-white">
                1. Choose a public target
              </h3>
              <p className="m-0 text-[0.8125rem] leading-5 text-khala-text-faint">
                Select an S, M, or L public MirrorCode task. Private tasks are
                excluded from this service surface.
              </p>
            </div>
            <div
              className="grid content-start gap-2 border border-khala-border bg-black p-3"
              data-mirrorcode-playground-step="launch"
            >
              <h3 className="m-0 text-sm font-semibold text-white">
                2. Queue a launch intent
              </h3>
              <p className="m-0 text-[0.8125rem] leading-5 text-khala-text-faint">
                The owner-operated runner creates a queued row first, then the
                external MirrorCode/Inspect executor updates status and
                result.
              </p>
            </div>
            <div
              className="grid content-start gap-2 border border-khala-border bg-black p-3"
              data-mirrorcode-playground-step="status"
            >
              <h3 className="m-0 text-sm font-semibold text-white">
                3. Poll public status
              </h3>
              <p className="m-0 text-[0.8125rem] leading-5 text-khala-text-faint">
                Read a single run or the leaderboard. The response carries
                public-safe fields only, with exact-token refs when a
                decision-grade result exists.
              </p>
            </div>
          </div>
          <div className="grid gap-3 lg:grid-cols-2">
            <CodeBlock label="Launch intent" lines={launchIntentLines} />
            <CodeBlock label="Status read" lines={statusReadLines} />
          </div>
        </Card>

        <Card
          className="grid min-w-0 gap-4 border-khala-border bg-khala-surface p-4"
          data-mirrorcode-live-panel=""
        >
          <div className="grid gap-2">
            <p className={eyebrowClass}>Live run</p>
            <h2 className="m-0 max-w-[24ch] text-2xl font-semibold tracking-tight text-balance text-white sm:text-3xl">
              The latest Khala run
            </h2>
            <p className={bodyClass}>
              Status, pass-rate over the public scoring suite, total tokens,
              task, bucket, and the run summary — straight from the public
              projection.
            </p>
          </div>
          <div
            className="grid place-items-start gap-2 border border-dashed border-khala-border bg-black p-6"
            data-mirrorcode-live-empty=""
          >
            <p className={eyebrowClass}>Latest run</p>
            <p className="m-0 text-base font-semibold text-white sm:text-lg">
              No runs yet — machinery shipped, awaiting first Phase-0 run
            </p>
            <p className="m-0 max-w-[78ch] text-base text-khala-text-muted sm:text-sm">
              The MirrorCode harness, scorer, and projection are live. As soon
              as the first Khala run lands, its status, pass-rate, token
              total, and summary appear here. Nothing on this page is
              fabricated.
            </p>
          </div>
        </Card>

        <Card
          className="grid min-w-0 gap-4 border-khala-border bg-khala-surface p-4"
          data-mirrorcode-execution-visualizer=""
        >
          <div className="grid gap-2">
            <p className={eyebrowClass}>Live run execution visualizer</p>
            <h2 className="m-0 max-w-[28ch] text-2xl font-semibold tracking-tight text-balance text-white sm:text-3xl">
              Follow MirrorCode tasks from queue to closeout
            </h2>
            <p className={bodyClass}>
              A compact execution rail for the newest public runs. It shows
              lifecycle state, scoring posture, token burn, and finish status
              without exposing prompts, raw logs, private benchmark material,
              or canary strings.
            </p>
          </div>
          <div
            className="grid place-items-start gap-2 border border-dashed border-khala-border bg-black p-6"
            data-mirrorcode-execution-empty=""
          >
            <p className={eyebrowClass}>Execution visualizer</p>
            <p className="m-0 text-base font-semibold text-white sm:text-lg">
              No execution rows to visualize yet
            </p>
            <p className="m-0 max-w-[78ch] text-base text-khala-text-muted sm:text-sm">
              When a MirrorCode run exists, this panel renders the queued,
              implementation, scoring, and closeout phases from public-safe
              status rows only.
            </p>
          </div>
        </Card>

        <Card
          className="grid min-w-0 gap-4 border-khala-border bg-khala-surface p-4"
          data-mirrorcode-leaderboard-panel=""
        >
          <PanelHeader
            meta="Every real Khala run, newest first. Smoke runs are labeled Phase-0 smoke and are not frontier measurements."
            status="Idle"
            title="Khala runs on the public MirrorCode suite"
          />
          <p className="m-0 text-sm/6 text-khala-text-muted">
            No scored runs yet — the leaderboard and paper-reference
            comparators render from the same public projection once the first
            run lands.
          </p>
        </Card>

        <Card className="grid gap-3 p-4 sm:p-5">
          <p className={eyebrowClass}>Live surface</p>
          <p className={bodyClass}>
            The live run feed, execution visualizer, and leaderboard stay on
            the existing Foldkit page until this route carries a real fetch
            against the endpoint below.
          </p>
          <div className="grid gap-3 border border-khala-border bg-black/25 p-3 text-sm/5 text-khala-text-muted sm:grid-cols-2">
            <div>
              <span className="block text-khala-text-faint">Runs</span>
              <code className={codeClass}>/api/gym/mirrorcode/runs</code>
            </div>
            <div>
              <span className="block text-khala-text-faint">Single run</span>
              <code className={codeClass}>
                /api/gym/mirrorcode/runs/{'{runId}'}
              </code>
            </div>
          </div>
        </Card>
      </div>
    </main>
  )
}

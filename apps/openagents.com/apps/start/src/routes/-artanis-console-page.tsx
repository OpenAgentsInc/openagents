import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'

const eyebrowClass =
  'm-0 font-mono text-xs font-semibold uppercase leading-none tracking-wide text-khala-energy-soft'
const linkClass =
  'border border-khala-border px-3 py-2 text-xs text-khala-text-muted underline-offset-4 transition-colors hover:border-khala-border-strong hover:text-khala-text hover:underline'

function StatMetric({
  detail,
  label,
  value,
}: Readonly<{ detail: string; label: string; value: string }>) {
  return (
    <div className="grid min-h-28 min-w-0 content-between gap-3 overflow-hidden border border-khala-border bg-khala-surface p-3">
      <div className="truncate text-[0.6875rem] text-khala-text-faint">
        {label}
      </div>
      <div className="min-w-0 break-words text-2xl font-semibold leading-tight tracking-normal tabular-nums text-khala-text sm:text-3xl">
        {value}
      </div>
      <div className="break-words text-xs text-khala-text-faint">
        {detail}
      </div>
    </div>
  )
}

const fleetMapSlotCount = 8

function FleetMapSlot({ index }: Readonly<{ index: number }>) {
  return (
    <li
      className="grid min-h-24 content-between gap-3 border border-khala-border bg-khala-void p-2 text-khala-text-faint"
      data-fleet-map-slot="empty"
    >
      <div className="flex items-center justify-between gap-2">
        <span className="text-[0.625rem] uppercase tracking-wide text-khala-text-faint">
          {String(index + 1).padStart(2, '0')}
        </span>
        <span className="text-[0.625rem] text-current">empty</span>
      </div>
      <div className="grid min-w-0 gap-1">
        <span className="truncate text-xs font-semibold text-khala-text">
          {`slot ${index + 1}`}
        </span>
        <span className="truncate text-[0.6875rem] text-khala-text-faint">
          no public heartbeat
        </span>
      </div>
    </li>
  )
}

const taskBoardLanes = [
  { detail: 'Slots ready to accept work', label: 'Ready' },
  { detail: 'Work claimed by public Pylon refs', label: 'Claimed' },
  {
    detail: 'Trace and verification work in flight',
    label: 'Verifying',
  },
  {
    detail: 'Verifier outcomes from the public feed',
    label: 'Resolved',
  },
] as const

function TaskBoardLane({
  detail,
  label,
}: Readonly<{ detail: string; label: string }>) {
  return (
    <div className="grid min-h-48 content-start gap-3 border border-khala-border bg-khala-void p-3">
      <div className="grid gap-1">
        <div className="flex items-center justify-between gap-2">
          <span className="text-xs font-semibold text-khala-text">
            {label}
          </span>
          <span className="tabular-nums text-[0.6875rem] text-khala-text-faint">
            0
          </span>
        </div>
        <div className="text-[0.6875rem] text-khala-text-faint">{detail}</div>
      </div>
      <div
        className="border border-khala-border bg-black p-3 text-xs text-khala-text-faint"
        role="status"
      >
        No public rows in this lane.
      </div>
    </div>
  )
}

const virtualMergeQueueLanes = [
  {
    detail: 'Pinned commit from GitHub branch protection',
    label: 'Actual head',
    value: 'origin/main',
  },
  {
    detail: 'Advances after each verified non-conflicting candidate',
    label: 'Virtual head',
    value: 'projection',
  },
  {
    detail: 'New fleet work starts from the projected post-merge tree',
    label: 'Next branch base',
    value: 'virtual head',
  },
  {
    detail: 'Duplicate issue, stale base, closed issue, or path conflict',
    label: 'Conflict lane',
    value: 'blocked',
  },
] as const

const virtualMergeQueueSteps = [
  {
    detail:
      'Issue is open, one PR per issue is preserved, and verification passed.',
    label: '1. Admit',
  },
  {
    detail:
      'Candidate patch becomes the next virtual head before another agent branches.',
    label: '2. Project',
  },
  {
    detail: 'Only the front ready entry moves to the real protected branch.',
    label: '3. Promote',
  },
] as const

const fleetOnboardingCommands = [
  'npm install -g @openagentsinc/khala',
  'khala fleet connect',
  'khala fleet status',
] as const

// Verbatim from the Foldkit original's `campaignObjective` constant.
const campaignObjective =
  'Release the next version of Pylon, connect it deeply to Omega, and route more inference and fine-tuning work to the live Pylon wave using the new Bitcoin infrastructure.'

// `openagents.com/artanis` (and `/agents/artanis`) — the live Artanis fleet
// recruitment console. The Foldkit original
// (apps/web/src/page/loggedOut/page/publicAgent.ts `artanisLoadedView` and
// its sub-views) is driven by five independent live models (the public
// goal, Pylon stats, the public activity timeline, the Khala tokens-served
// history, and — indirectly — the fleet-shipping feed), each fed by its own
// client fetch with no live wiring in Start yet. Every prior TS-6 Start
// route has stayed static/SSR-only, so this port keeps that posture: it
// renders every static, always-shown section in full fidelity (the fleet
// map slot grid, the four-lane task board, the virtual merge queue, and the
// "join the fleet" onboarding panel are all genuinely static — none of them
// take a live-model argument in the Foldkit original), and every
// live-model-driven panel (the header's goal/health strip, the Pulse token
// burn chart, the campaign-objective goal panel, the fleet-shipping feed,
// and the Pylon stats panel) renders the EXACT copy each model's own
// pre-fetch branch already produces in the Foldkit `view()` — e.g.
// `artanisPulseView` on the `PublicKhalaTokensServedHistoryIdle` tag falls
// through to its "Token pace unavailable." placeholder, and
// `fleetShippingView` on that same non-Loaded/non-Failed case renders
// "Loading live fleet activity." — rather than fabricating burn rates,
// Pylon rows, or fleet-shipping events.
export function ArtanisConsolePage() {
  return (
    <main
      aria-label="Artanis fleet recruitment console"
      className="min-h-dvh bg-khala-void text-khala-text"
      data-agent="artanis"
      data-component="public-agent-page"
      data-route="artanis"
    >
      <div className="mx-auto grid min-h-dvh w-full max-w-[96rem] content-start gap-5 px-4 py-5 font-mono sm:px-6 lg:px-8">
        <Card className="grid gap-4 px-4 py-4 sm:px-5">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="flex flex-wrap items-center gap-3">
              <div className="text-lg font-semibold tracking-normal text-khala-text sm:text-xl">
                ARTANIS console
              </div>
              <Badge variant="ready">
                <span className="h-2 w-2 animate-pulse rounded-full bg-khala-success" />
                LIVE
              </Badge>
            </div>
            <a className="text-xs text-khala-text-faint underline-offset-4 hover:text-khala-text hover:underline" href="/">
              Start your own agent
            </a>
          </div>
          <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(13rem,0.32fr)]">
            <div className="grid gap-2">
              <h1 className="m-0 text-3xl font-semibold leading-none tracking-normal text-khala-text sm:text-4xl">
                Artanis
              </h1>
              <div className="flex flex-wrap gap-x-5 gap-y-2 text-xs text-khala-text-muted">
                <span>No public goal</span>
                <span>Active slots loading</span>
                <span>no active public run</span>
              </div>
            </div>
            <div className="grid content-end gap-2">
              <div className="flex items-center justify-between gap-3 text-[0.6875rem] text-khala-text-faint">
                <span>Daily token pace</span>
                <span className="tabular-nums text-khala-text-muted">0%</span>
              </div>
              <div className="h-2 overflow-hidden border border-khala-border bg-black">
                <div className="h-full bg-khala-warning" style={{ width: '0%' }} />
              </div>
            </div>
          </div>
        </Card>

        <Card className="grid gap-3 p-4 sm:p-5">
          <p className={eyebrowClass}>The Pulse</p>
          <div
            className="border border-khala-border bg-khala-surface p-4 text-sm text-khala-text-muted"
            role="status"
          >
            Token pace unavailable.
          </div>
        </Card>

        <Card
          className="grid gap-4 p-4 sm:p-5"
          data-component="artanis-fleet-map-task-board"
        >
          <div className="flex flex-wrap items-end justify-between gap-3">
            <div className="grid gap-2">
              <p className={eyebrowClass}>Fleet map</p>
              <h2 className="m-0 text-lg font-semibold tracking-normal text-khala-text">
                Pylons, slots, active tasks
              </h2>
            </div>
            <div className="flex flex-wrap gap-2 text-xs text-khala-text-muted">
              <span>0 assignment-ready</span>
              <span>0 task rows</span>
            </div>
          </div>
          <div className="grid gap-4 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
            <div className="grid content-start gap-3">
              <ul className="grid grid-cols-2 gap-2 sm:grid-cols-4 lg:grid-cols-6 xl:grid-cols-4">
                {Array.from({ length: fleetMapSlotCount }, (_, index) => (
                  <FleetMapSlot index={index} key={index} />
                ))}
              </ul>
              <p className="m-0 text-xs leading-5 text-khala-text-faint">
                Loading public Pylon slots from the live stats projection.
              </p>
            </div>
            <div className="grid content-start gap-3">
              <div className="grid gap-1">
                <p className={eyebrowClass}>Active Task Board</p>
                <p className="m-0 text-xs text-khala-text-faint">
                  Work and verification lanes from the public activity feed.
                </p>
              </div>
              <div className="grid gap-2 md:grid-cols-2">
                {taskBoardLanes.map(lane => (
                  <TaskBoardLane
                    detail={lane.detail}
                    key={lane.label}
                    label={lane.label}
                  />
                ))}
              </div>
              <p className="m-0 text-xs leading-5 text-khala-text-faint">
                Only public activity rows are shown; prompts, local
                workspaces, private traces, and provider payloads stay out of
                this board.
              </p>
            </div>
          </div>
        </Card>

        <Card
          className="grid gap-4 p-4 sm:p-5"
          data-component="artanis-virtual-merge-queue"
        >
          <div className="flex flex-wrap items-end justify-between gap-3">
            <div className="grid gap-2">
              <p className={eyebrowClass}>Virtual merge queue</p>
              <h2 className="m-0 text-lg font-semibold tracking-normal text-khala-text">
                Projected branch base for parallel agents
              </h2>
            </div>
            <a
              className={linkClass}
              href="/docs/artanis/2026-06-28-gitafter-cloudflare-artifacts-coordination-audit"
            >
              Coordination audit
            </a>
          </div>
          <div className="grid gap-4 xl:grid-cols-[minmax(0,1.2fr)_minmax(18rem,0.8fr)]">
            <div className="grid gap-3">
              <ul
                aria-label="Virtual merge queue projection lanes"
                className="grid gap-2 sm:grid-cols-2 xl:grid-cols-4"
              >
                {virtualMergeQueueLanes.map(lane => (
                  <li
                    className="grid min-h-28 content-between gap-3 border border-khala-border bg-khala-surface-raised p-3 text-khala-text-muted"
                    key={lane.label}
                  >
                    <div className="grid gap-1">
                      <span className="text-[0.6875rem] uppercase tracking-wide text-khala-text-faint">
                        {lane.label}
                      </span>
                      <span className="text-lg font-semibold leading-6 text-khala-text">
                        {lane.value}
                      </span>
                    </div>
                    <p className="m-0 text-xs leading-5">{lane.detail}</p>
                  </li>
                ))}
              </ul>
              <div className="grid gap-2 border border-khala-border bg-khala-surface p-3">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <span className="text-xs font-semibold text-khala-text">
                    Public proof fixture
                  </span>
                  <span className="tabular-nums text-[0.6875rem] text-khala-energy-soft">
                    24 accepted / 0 conflicts
                  </span>
                </div>
                <div className="h-2 overflow-hidden border border-khala-border bg-black">
                  <div className="h-full bg-khala-energy" style={{ width: '100%' }} />
                </div>
                <p className="m-0 text-xs leading-5 text-khala-text-muted">
                  The shipped simulator proves a 20+ item queue can advance
                  one virtual head without opening duplicate work for the
                  same issue.
                </p>
              </div>
            </div>
            <div className="grid content-start gap-3">
              <ol className="grid gap-2">
                {virtualMergeQueueSteps.map(step => (
                  <li
                    className="grid gap-1 border border-khala-border bg-black p-3"
                    key={step.label}
                  >
                    <div className="text-xs font-semibold leading-5 text-khala-text">
                      {step.label}
                    </div>
                    <p className="m-0 text-xs leading-5 text-khala-text-muted">
                      {step.detail}
                    </p>
                  </li>
                ))}
              </ol>
              <p className="m-0 border border-khala-border bg-khala-surface p-3 text-xs leading-5 text-khala-text-muted">
                Public-safe only: no raw patches, local workspace paths,
                provider payloads, or private prompts are exposed on this
                page.
              </p>
            </div>
          </div>
        </Card>

        <div className="grid gap-5 xl:grid-cols-[minmax(17rem,0.75fr)_minmax(0,1.45fr)_minmax(18rem,0.8fr)]">
          <div className="grid content-start gap-5">
            <Card className="grid gap-3 p-4 sm:p-5">
              <p className={eyebrowClass}>Campaign objective</p>
              <div className="grid gap-3">
                <p className="m-0 max-w-4xl whitespace-pre-wrap text-base leading-7 text-khala-text">
                  {campaignObjective}
                </p>
                <div className="flex flex-wrap gap-x-6 gap-y-2 text-xs text-khala-text-muted">
                  <span>Awaiting the first public durable Artanis goal.</span>
                </div>
              </div>
            </Card>
          </div>
          <div className="grid content-start gap-5">
            <Card className="grid gap-4 p-4 sm:p-5">
              <div className="flex flex-wrap items-end justify-between gap-3">
                <div className="grid gap-2">
                  <p className={eyebrowClass}>Fleet shipping</p>
                  <h2 className="m-0 text-lg font-semibold tracking-normal text-khala-text">
                    What the fleet is doing now
                  </h2>
                </div>
                <Badge variant="outline">Loading</Badge>
              </div>
              <div
                className="border border-khala-border bg-khala-surface p-4 text-sm text-khala-text-muted"
                role="status"
              >
                Loading live fleet activity.
              </div>
            </Card>
          </div>
          <div className="grid content-start gap-5">
            <Card className="grid gap-3 p-4 sm:p-5">
              <div className="flex flex-wrap items-end justify-between gap-3">
                <div className="grid gap-2">
                  <p className={eyebrowClass}>Pylon network</p>
                  <h2 className="m-0 text-lg font-semibold tracking-normal text-khala-text">
                    Omega Pylon stats
                  </h2>
                </div>
                <span className="text-xs text-khala-text-muted">
                  Feed loading
                </span>
              </div>
              <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 xl:grid-cols-2">
                <StatMetric
                  detail="v0.2.5+ heartbeat window"
                  label="Pylons online"
                  value="-"
                />
                <StatMetric
                  detail="Public readiness"
                  label="Wallet ready"
                  value="-"
                />
                <StatMetric
                  detail="Recent check-ins"
                  label="Seen in 24h"
                  value="-"
                />
                <StatMetric
                  detail="Stats loading"
                  label="Earning gate"
                  value="-"
                />
                <StatMetric
                  detail="Stats loading"
                  label="Version floor"
                  value="-"
                />
              </div>
              <div className="grid gap-2 border border-khala-border bg-khala-surface p-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.25fr)]">
                <div className="grid gap-2 text-xs text-khala-text-muted">
                  <div className="text-khala-text">Source none</div>
                  <div>Relay none</div>
                  <div>Timestamp unavailable</div>
                  <div>Training participants 0 / assigned 0</div>
                </div>
                <div className="text-xs text-khala-text-faint">
                  Loading recent Pylon presence.
                </div>
              </div>
            </Card>

            <Card className="grid gap-4 p-4 sm:p-5">
              <div className="flex flex-wrap items-end justify-between gap-3">
                <div className="grid gap-2">
                  <p className={eyebrowClass}>Join fleet</p>
                  <h2 className="m-0 text-lg font-semibold tracking-normal text-khala-text">
                    Have Codex or Claude? Join the fleet.
                  </h2>
                </div>
                <a className={linkClass} href="/docs/connect-codex-fleet">
                  Fleet docs
                </a>
              </div>
              <div className="grid gap-3 lg:grid-cols-[minmax(0,0.95fr)_minmax(0,1.05fr)]">
                <div className="grid gap-3 border border-khala-border bg-khala-surface p-3">
                  <p className="m-0 max-w-3xl text-sm leading-6 text-khala-text">
                    Connect your own coding-agent capacity so a per-user
                    Artanis can burn down public issue backlogs through your
                    local Pylon. Credentials stay on your machine; public
                    projections use generic fleet labels and refs only.
                  </p>
                  <div className="grid gap-2 text-xs text-khala-text-muted sm:grid-cols-3">
                    <div className="border border-khala-border p-2">
                      Paste-free device login
                    </div>
                    <div className="border border-khala-border p-2">
                      Isolated Codex account homes
                    </div>
                    <div className="border border-khala-border p-2">
                      More accounts, more throughput
                    </div>
                  </div>
                </div>
                <div className="grid gap-3 border border-khala-border bg-khala-surface p-3">
                  <p className="m-0 text-xs text-khala-text-muted">
                    Start here
                  </p>
                  <ol className="grid gap-1">
                    {fleetOnboardingCommands.map(command => (
                      <li
                        className="overflow-x-auto border border-khala-border bg-black px-3 py-2 text-xs leading-6 text-khala-text"
                        key={command}
                      >
                        <code>{command}</code>
                      </li>
                    ))}
                  </ol>
                  <div className="flex flex-wrap gap-2">
                    <a className={linkClass} href="/docs/connect-codex-fleet">
                      Read the setup guide
                    </a>
                  </div>
                </div>
              </div>
            </Card>
          </div>
        </div>
      </div>
    </main>
  )
}

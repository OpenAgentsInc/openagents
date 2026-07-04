import {
  CheckCircle2,
  MessageSquareText,
  XCircle,
} from 'lucide-react'

type SessionSummary = Readonly<{
  sessionRef: string
  adapter: string
  state: 'completed' | 'running'
  objectiveRef: string
  accountRefHash: string | null
  lastProgressRef?: string
  updatedAt: string
}>

type DecisionAction = Readonly<{
  icon: typeof CheckCircle2
  label: string
  verb: 'answer' | 'approve' | 'deny'
}>

export const protocolSessionFixtures: ReadonlyArray<SessionSummary> = [
  {
    sessionRef: 'session.pylon.codex_composer.fixture0001',
    adapter: 'codex',
    state: 'running',
    objectiveRef: 'objective.fixture.abc123',
    accountRefHash: 'account.pylon.codex.fixturehash01',
    lastProgressRef: 'progress.fixture.0001',
    updatedAt: '2026-06-13T12:00:00.000Z',
  },
  {
    sessionRef: 'session.pylon.claude_composer.fixture0002',
    adapter: 'claude_agent',
    state: 'completed',
    objectiveRef: 'objective.fixture.def456',
    accountRefHash: null,
    updatedAt: '2026-06-13T12:01:00.000Z',
  },
] as const

export const protocolDecisionFixture = {
  requestId: 'decision.fixture.req01',
  actionRef: 'action.fixture.approve_pr',
  expiresAtMs: 1_900_000_000_000,
  state: 'pending',
  resolvedVerb: 'none',
} as const

const panelClass =
  'grid gap-4 border border-khala-border/80 bg-khala-surface p-5 text-khala-text-muted'

const eyebrowClass =
  'm-0 font-mono text-sm uppercase tracking-wide text-khala-text-faint'

const codeClass =
  'break-all font-mono text-sm text-khala-energy-soft'

function stateClass(state: SessionSummary['state']) {
  return state === 'running'
    ? 'border-khala-energy-cyan/40 bg-khala-energy/10 text-khala-energy-soft'
    : 'border-khala-success/40 bg-khala-success/10 text-khala-success'
}

function SessionList() {
  return (
    <section
      className="grid border border-khala-border/70 bg-black"
      data-autopilot-session-list=""
      aria-label="Autopilot sessions"
    >
      {protocolSessionFixtures.map(session => (
        <article
          className="grid gap-3 border-b border-khala-border/60 p-4 last:border-b-0 md:grid-cols-[minmax(0,1fr)_auto]"
          data-autopilot-session-ref={session.sessionRef}
          key={session.sessionRef}
        >
          <div className="grid min-w-0 gap-2">
            <code className={codeClass}>{session.sessionRef}</code>
            <div className="flex flex-wrap gap-2 text-sm text-khala-text-muted">
              <span>{session.adapter}</span>
              <span>{session.objectiveRef}</span>
              <span>{session.lastProgressRef ?? 'none'}</span>
            </div>
          </div>
          <div className="grid content-start gap-2 justify-self-start md:justify-self-end">
            <span
              className={`w-fit border px-2 py-1 font-mono text-sm ${stateClass(session.state)}`}
            >
              {session.state}
            </span>
            <time className="font-mono text-xs text-khala-text-faint">
              {session.updatedAt}
            </time>
          </div>
        </article>
      ))}
    </section>
  )
}

const decisionActions: ReadonlyArray<DecisionAction> = [
  { icon: CheckCircle2, label: 'Approve', verb: 'approve' },
  { icon: XCircle, label: 'Deny', verb: 'deny' },
  { icon: MessageSquareText, label: 'Answer', verb: 'answer' },
]

function DecisionCard() {
  return (
    <article
      className={panelClass}
      data-autopilot-decision-id={protocolDecisionFixture.requestId}
    >
      <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto]">
        <div className="grid min-w-0 gap-2">
          <p className={eyebrowClass}>Pending decision</p>
          <h2 className="m-0 break-all text-2xl font-semibold tracking-tight text-white">
            {protocolDecisionFixture.actionRef}
          </h2>
          <code className={codeClass}>{protocolDecisionFixture.requestId}</code>
        </div>
        <span
          className="h-fit w-fit border border-khala-warning/45 bg-khala-warning/10 px-2 py-1 font-mono text-sm text-khala-warning"
          data-autopilot-decision-state={protocolDecisionFixture.state}
        >
          {protocolDecisionFixture.state}
        </span>
      </div>
      <dl className="grid gap-3 sm:grid-cols-2">
        <div className="grid gap-1 border border-khala-border/70 bg-black p-3">
          <dt className={eyebrowClass}>Resolved</dt>
          <dd className="m-0 font-mono text-base text-white">
            {protocolDecisionFixture.resolvedVerb}
          </dd>
        </div>
        <div className="grid gap-1 border border-khala-border/70 bg-black p-3">
          <dt className={eyebrowClass}>Expires</dt>
          <dd className="m-0 font-mono text-base text-white">
            {protocolDecisionFixture.expiresAtMs}
          </dd>
        </div>
      </dl>
      <div className="flex flex-wrap gap-2">
        {decisionActions.map(action => {
          const Icon = action.icon

          return (
            <button
              className="khala-focus inline-flex min-h-11 items-center gap-2 border border-khala-border bg-khala-surface-raised px-3 py-2 font-mono text-sm text-khala-text disabled:cursor-not-allowed disabled:opacity-70"
              data-autopilot-decision-action={action.verb}
              disabled
              key={action.verb}
              type="button"
            >
              <Icon aria-hidden="true" size={16} strokeWidth={1.8} />
              <span>{action.label}</span>
            </button>
          )
        })}
      </div>
    </article>
  )
}

export function ClientsPreviewPage() {
  return (
    <main className="min-h-dvh bg-black text-white" data-route="clients-preview">
      <div className="mx-auto grid w-full max-w-7xl gap-6 px-4 py-8 font-mono sm:px-6 lg:px-8">
        <header className="grid gap-3 border-b border-khala-border/80 pb-5">
          <a
            className="khala-focus w-fit border border-khala-border bg-khala-surface-raised px-3 py-2 font-mono text-sm text-khala-text"
            href="/"
          >
            OpenAgents
          </a>
          <p className={eyebrowClass}>Clients preview</p>
          <h1 className="m-0 text-balance text-4xl font-semibold tracking-tight text-white sm:text-5xl">
            Autopilot control surface
          </h1>
        </header>
        <div className="grid gap-5 lg:grid-cols-[minmax(0,1.15fr)_minmax(20rem,0.85fr)]">
          <section className={panelClass}>
            <div className="grid gap-2">
              <p className={eyebrowClass}>Protocol fixtures</p>
              <h2 className="m-0 text-2xl font-semibold tracking-tight text-white">
                Sessions
              </h2>
            </div>
            <SessionList />
          </section>
          <section className="grid content-start gap-3">
            <h2 className="m-0 text-2xl font-semibold tracking-tight text-white">
              Decision
            </h2>
            <DecisionCard />
          </section>
        </div>
      </div>
    </main>
  )
}

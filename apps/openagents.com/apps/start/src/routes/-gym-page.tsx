const panelClass =
  'grid gap-4 border border-khala-border/80 bg-khala-surface p-5 text-khala-text-muted'

const eyebrowClass =
  'm-0 font-mono text-sm uppercase tracking-wide text-khala-text-faint'

const controlClass =
  'flex min-h-11 items-center gap-2 border border-khala-border/70 bg-khala-surface-muted px-3 py-2 font-mono text-base text-khala-text-muted sm:text-sm'

const selectClass =
  'min-h-11 border border-khala-border/80 bg-black px-3 py-2 font-mono text-base text-khala-text outline-none sm:text-sm'

const statClass =
  'grid gap-1 border border-khala-border/70 bg-black p-3 font-mono'

function EmptyState({
  body,
  heading,
  marker,
}: Readonly<{
  body: string
  heading: string
  marker: string
}>) {
  return (
    <div
      className="grid place-items-start gap-2 border border-dashed border-khala-border/80 bg-black p-6"
      data-gym-run-progress-accessible-mirror={marker === 'run-progress' ? '' : undefined}
      data-gym-run-progress-empty={marker === 'run-progress' ? '' : undefined}
      data-gym-terminal-bench-empty={marker === 'terminal-bench' ? '' : undefined}
    >
      <p className={eyebrowClass}>Live data</p>
      <p className="m-0 text-lg font-semibold text-white">{heading}</p>
      <p className="m-0 max-w-[78ch] text-pretty text-base/7 text-khala-text-muted sm:text-sm/6">
        {body}
      </p>
    </div>
  )
}

function TerminalBenchPanel() {
  return (
    <section className={panelClass} data-gym-terminal-bench-panel="">
      <div className="grid gap-2">
        <p className={eyebrowClass}>Terminal-Bench Gym replay</p>
        <h2 className="m-0 max-w-[24ch] text-balance text-3xl font-semibold tracking-tight text-white sm:text-4xl">
          Terminal-Bench 2.0 run field
        </h2>
        <p className="m-0 max-w-[78ch] text-pretty text-base/7 text-khala-text-muted sm:text-sm/6">
          A public field for profile lanes, verifier placement,
          accepted/failing tasks, cost basis, latency, throughput, and claim
          caveats. It renders from real published benchmark reports only.
        </p>
      </div>
      <EmptyState
        marker="terminal-bench"
        heading="No decision-grade benchmark reports published yet"
        body="When a real Terminal-Bench report is ingested and authorized for the web, its lanes, verifier placement, and caveats appear here. No fixture or placeholder pass rates are shown."
      />
    </section>
  )
}

function RunProgressPanel() {
  return (
    <section className={panelClass} data-gym-run-progress-panel="">
      <div className="grid gap-2">
        <p className={eyebrowClass}>Live Gym run follow-along</p>
        <h2 className="m-0 max-w-[24ch] text-balance text-3xl font-semibold tracking-tight text-white sm:text-4xl">
          Follow an active Terminal-Bench run
        </h2>
        <p className="m-0 max-w-[78ch] text-pretty text-base/7 text-khala-text-muted sm:text-sm/6">
          Counts, pass-rate over completed tasks, the official denominator,
          and freshness update from the public-safe progress projection as a
          real run is ingested; raw prompts, responses, logs, trajectories,
          keys, and private endpoints are never included.
        </p>
      </div>
      <EmptyState
        marker="run-progress"
        heading="No active Gym run"
        body="Live runs appear here when a real Harbor/Khala benchmark is ingested. Pass rate is always computed over completed tasks, with the official denominator kept separate, so a partial run is never read as a final solve rate."
      />
    </section>
  )
}

function ExperimentControls() {
  const lanes = [
    'Khala own capacity',
    'OpenRouter BYOK',
    'Local OpenAI-compatible',
  ]
  const coordinators = ['Single agent', 'Verifier pick', 'Best of n']
  const modules = [
    'Prompt-only baseline',
    'Program signature modules',
    'Replay verifier enabled',
  ]

  return (
    <div className="grid gap-4 lg:grid-cols-[1.2fr_0.8fr]">
      <section className={panelClass}>
        <p className={eyebrowClass}>Experiment</p>
        <div className="grid gap-4 md:grid-cols-2">
          <label className="grid gap-1.5 font-mono text-base text-khala-text-muted sm:text-sm">
            <span>Environment</span>
            <select className={selectClass} disabled value="bundled-decision-suite-v1">
              <option value="bundled-decision-suite-v1">
                Bundled decision suite v1
              </option>
            </select>
          </label>
          <label className="grid gap-1.5 font-mono text-base text-khala-text-muted sm:text-sm">
            <span>Fan-out mode</span>
            <select className={selectClass} disabled value="single">
              <option value="single">Single</option>
            </select>
          </label>
          <label className="grid gap-1.5 font-mono text-base text-khala-text-muted sm:text-sm">
            <span>Concurrency</span>
            <input className={selectClass} disabled max={8} min={1} type="number" value={1} />
          </label>
          <label className="grid gap-1.5 font-mono text-base text-khala-text-muted sm:text-sm">
            <span>Transport</span>
            <select className={selectClass} disabled value="fixture-only">
              <option value="fixture-only">Fixture-only compile plan</option>
            </select>
          </label>
        </div>
        <div className="grid gap-2">
          <p className={eyebrowClass}>Provider fan-out</p>
          <div className="grid gap-2 sm:grid-cols-3">
            {lanes.map(lane => (
              <label className={controlClass} key={lane}>
                <input checked disabled readOnly type="checkbox" />
                <span>{lane}</span>
              </label>
            ))}
          </div>
        </div>
        <div className="grid gap-2">
          <p className={eyebrowClass}>Coordinator candidates</p>
          <div className="grid gap-2 sm:grid-cols-3">
            {coordinators.map(candidate => (
              <label className={controlClass} key={candidate}>
                <input checked disabled readOnly type="checkbox" />
                <span>{candidate}</span>
              </label>
            ))}
          </div>
        </div>
        <div className="grid gap-2">
          <p className={eyebrowClass}>Program signature modules</p>
          <div className="grid gap-2 sm:grid-cols-3">
            {modules.map(module => (
              <label className={controlClass} key={module}>
                <input checked disabled readOnly type="checkbox" />
                <span>{module}</span>
              </label>
            ))}
          </div>
        </div>
      </section>
      <aside className={panelClass} data-gym-no-spend-explainer="">
        <p className={eyebrowClass}>Economics</p>
        <h2 className="m-0 text-balance text-2xl font-semibold tracking-tight text-white">
          Locked to no spend
        </h2>
        <p className="m-0 text-pretty text-base/7 text-khala-text-muted sm:text-sm/6">
          This Start route preserves the public Gym controls, but it does not
          dispatch provider calls, debit credits, mint invoices, or publish a
          report. Spend-bearing runs still require the existing owner-armed
          backend preflight.
        </p>
        <div className="grid gap-3">
          <div className={statClass}>
            <span className="text-khala-text-faint">Seam</span>
            <span className="text-base font-semibold text-white">fixture compile only</span>
          </div>
          <div className={statClass}>
            <span className="text-khala-text-faint">Public data</span>
            <span className="text-base font-semibold text-white">live projections only</span>
          </div>
          <div className={statClass}>
            <span className="text-khala-text-faint">Private material</span>
            <span className="text-base font-semibold text-white">never rendered</span>
          </div>
        </div>
      </aside>
    </div>
  )
}

export function GymPage() {
  return (
    <main className="min-h-dvh bg-black text-white" data-gym-page="" data-route="gym">
      <div className="mx-auto grid w-full max-w-7xl gap-6 px-4 py-8 font-mono sm:px-6 lg:px-8">
        <header className="grid gap-3">
          <div
            className="w-fit border border-khala-energy-cyan/30 bg-khala-energy/10 px-3 py-1 font-mono text-sm font-semibold uppercase tracking-wide text-khala-energy-soft"
            data-gym-no-spend-banner=""
          >
            Live data only / no-spend
          </div>
          <h1 className="m-0 text-balance text-4xl font-semibold tracking-tight text-white sm:text-5xl">
            OpenAgents Gym
          </h1>
          <p className="m-0 max-w-[78ch] text-pretty text-base/7 text-khala-text-muted">
            A public lab for Khala policy shapes and Terminal-Bench run
            visualization. Configure the bundled decision suite below. Live
            runs and benchmark reports populate the surfaces once a real
            Harbor/Khala run is ingested; nothing on this page is fabricated,
            and this page never reaches provider accounts or billing.
          </p>
        </header>
        <TerminalBenchPanel />
        <RunProgressPanel />
        <ExperimentControls />
      </div>
    </main>
  )
}

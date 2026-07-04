type Door = Readonly<{
  href: string
  kicker: string
  title: string
  body: string
  facts: ReadonlyArray<string>
  cta: string
}>

const doors: ReadonlyArray<Door> = [
  {
    href: '/khala',
    kicker: 'FOR BUILDERS',
    title: 'Build it myself',
    body: 'Khala Code: an open-source console that turns the coding subscriptions you already pay for into an orchestrated fleet - one inbox, exact token accounting, swarm delegation.',
    facts: [
      '100% open source',
      'OpenAI-compatible free API - one base URL swap',
      'Wraps your own Codex; Claude lane landing',
      'Exact public token accounting',
    ],
    cta: 'Explore Khala',
  },
  {
    href: '/business',
    kicker: 'FOR BUSINESSES',
    title: 'Build it for me',
    body: 'Agents that work: hire agents from the OpenAgents network to get software built fast - scoped as a quick win, delivered in days, accepted by you before anything ships or spends.',
    facts: [
      'Quick win first - days, not quarters',
      'Human-review gate before publish/send/spend',
      'Receipts on every accepted outcome',
      'Pay in dollars or Bitcoin',
    ],
    cta: 'Talk to Khala',
  },
] as const

const doorClass =
  'group grid content-between gap-8 border border-khala-border/90 bg-khala-surface p-6 no-underline transition-colors duration-150 hover:border-khala-text-faint hover:bg-khala-surface-muted sm:p-8'

const eyebrowClass =
  'm-0 font-mono text-sm uppercase tracking-wide text-khala-text-faint'

function DoorCard({ door }: Readonly<{ door: Door }>) {
  return (
    <a className={doorClass} data-landing-preview-door={door.kicker} href={door.href}>
      <div className="grid gap-3">
        <p className="m-0 font-mono text-sm text-khala-warning">{door.kicker}</p>
        <h2 className="m-0 text-balance text-3xl font-medium tracking-normal text-khala-text sm:text-4xl">
          {door.title}
        </h2>
        <p className="m-0 max-w-[46ch] text-pretty text-base/7 text-khala-text-muted">
          {door.body}
        </p>
      </div>
      <div className="grid gap-4">
        <ul
          className="m-0 grid list-none gap-1.5 p-0 font-mono text-sm text-khala-text-faint"
          role="list"
        >
          {door.facts.map(fact => (
            <li key={fact}>{fact}</li>
          ))}
        </ul>
        <span className="font-mono text-sm text-khala-text transition-colors duration-150 group-hover:text-khala-warning">
          {door.cta}
        </span>
      </div>
    </a>
  )
}

export function LandingPreviewPage() {
  return (
    <main
      aria-label="OpenAgents"
      className="grid min-h-dvh content-start overflow-auto bg-black text-khala-text"
      data-landing-preview=""
      data-route="landing-preview"
    >
      <div className="border-b border-khala-border px-4 py-2 text-center font-mono text-xs text-khala-text-faint">
        preview - proposed landing page, not the live homepage
      </div>
      <div className="mx-auto grid w-full max-w-6xl content-start gap-12 px-4 py-14 sm:py-20">
        <header className="grid gap-5">
          <p className="m-0 font-mono text-base font-medium text-khala-text">
            OpenAgents
          </p>
          <h1 className="m-0 max-w-[16ch] text-balance text-5xl font-medium leading-[1.05] tracking-normal text-white sm:text-6xl">
            Software, built by agents.
          </h1>
          <p className="m-0 max-w-[58ch] text-pretty text-lg/8 text-khala-text-muted">
            One open network where coding agents do real work - yours, or ours.
            Every outcome lands with verifiable receipts.
          </p>
        </header>
        <section className="grid gap-4 sm:grid-cols-2" aria-label="Audience paths">
          {doors.map(door => (
            <DoorCard door={door} key={door.kicker} />
          ))}
        </section>
        <footer className="flex flex-wrap items-center gap-x-6 gap-y-2 border-t border-khala-border pt-6 font-mono text-xs text-khala-text-faint">
          <a
            className="khala-focus text-khala-text-faint no-underline transition-colors duration-150 hover:text-khala-text"
            href="https://github.com/OpenAgentsInc/openagents"
          >
            source: github.com/OpenAgentsInc/openagents
          </a>
          <a
            className="khala-focus text-khala-text-faint no-underline transition-colors duration-150 hover:text-khala-text"
            href="/docs/product-promises"
          >
            every claim: /docs/product-promises
          </a>
          <a
            className="khala-focus text-khala-text-faint no-underline transition-colors duration-150 hover:text-khala-text"
            href="/stats"
          >
            live usage: /stats
          </a>
        </footer>
        <p className={`${eyebrowClass} sr-only`}>Review-only candidate</p>
      </div>
    </main>
  )
}

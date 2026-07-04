import type * as React from 'react'

type ActivityPane = Readonly<{
  id: string
  title: string
  copy: string
}>

const panes: ReadonlyArray<ActivityPane> = [
  {
    id: 'fleet-map',
    title: 'Fleet Map',
    copy: 'Pylon presence, assignment readiness, and public worker refs appear here after the live timeline controller hydrates.',
  },
  {
    id: 'active-tasks',
    title: 'Active Task Board',
    copy: 'Public training windows and claimed work rows stay bounded to public-safe refs.',
  },
  {
    id: 'fleet',
    title: 'Fleet',
    copy: 'Capacity and heartbeat rows remain read-only public projections.',
  },
  {
    id: 'money',
    title: 'Money',
    copy: 'Settlement rows require receipt-backed evidence before real Bitcoin movement is shown.',
  },
  {
    id: 'forum',
    title: 'Forum',
    copy: 'Public forum topic and post activity lands here without private workspace material.',
  },
  {
    id: 'timeline',
    title: 'Timeline',
    copy: 'Cursor-addressable public events render by newest activity once the API payload loads.',
  },
] as const

const filters = ['all', 'boot', 'work', 'verify', 'settle', 'forum', 'operator'] as const

const panelClass =
  'grid gap-3 border border-khala-border/80 bg-khala-surface p-4 text-khala-text-muted'

const eyebrowClass =
  'm-0 font-mono text-sm uppercase tracking-wide text-khala-text-faint'

function ActivityLightDom() {
  return (
    <div className="grid gap-5">
      <header className="grid gap-3">
        <p className={eyebrowClass}>OpenAgents activity</p>
        <h1 className="m-0 text-balance text-4xl font-semibold tracking-tight text-white sm:text-5xl">
          Live public activity
        </h1>
        <p className="m-0 max-w-[84ch] text-pretty text-base/7 text-khala-text-muted">
          Read-only public projection. No settlement, payout, deployment,
          accepted-work, provider, wallet, or public-claim authority is
          available here.
        </p>
      </header>
      <section
        className="grid gap-3 border border-khala-border/80 bg-black p-4"
        data-activity-source-lag=""
      >
        <div className="grid gap-1">
          <p className={eyebrowClass}>Source lag</p>
          <p className="m-0 text-base/7 text-khala-text-muted sm:text-sm/6">
            Source status rows load from /api/public/activity-timeline.
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          {filters.map(filter => (
            <span
              className="border border-khala-border/70 bg-khala-surface-muted px-2 py-1 font-mono text-sm text-khala-text"
              data-activity-filter={filter}
              key={filter}
            >
              {filter}
            </span>
          ))}
        </div>
      </section>
      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
        {panes.map(pane => (
          <section
            className={panelClass}
            data-activity-pane={pane.id}
            key={pane.id}
          >
            <h2 className="m-0 text-2xl font-semibold tracking-tight text-white">
              {pane.title}
            </h2>
            <p className="m-0 text-pretty text-base/7 text-khala-text-muted sm:text-sm/6">
              {pane.copy}
            </p>
          </section>
        ))}
      </div>
      <aside className={panelClass} data-activity-pane="proof" data-proof-drawer="">
        <h2 className="m-0 text-2xl font-semibold tracking-tight text-white">
          Proof Drawer
        </h2>
        <p className="m-0 text-pretty text-base/7 text-khala-text-muted sm:text-sm/6">
          Select a public activity event to inspect its reproducible public
          refs, URLs, and source-lag state.
        </p>
      </aside>
    </div>
  )
}

export function ActivityPage() {
  return (
    <main
      aria-label="OpenAgents public activity"
      className="min-h-dvh bg-black text-white"
      data-route="activity"
    >
      <div className="mx-auto grid w-full max-w-7xl gap-6 px-4 py-8 font-mono sm:px-6 lg:px-8">
        <a
          className="khala-focus w-fit border border-khala-border bg-khala-surface-raised px-3 py-2 font-mono text-sm text-khala-text"
          href="/"
        >
          OpenAgents
        </a>
        <oa-public-activity-timeline data-route="activity" data-start-activity-timeline="">
          <ActivityLightDom />
        </oa-public-activity-timeline>
      </div>
    </main>
  )
}

declare module 'react' {
  namespace JSX {
    interface IntrinsicElements {
      'oa-public-activity-timeline': React.DetailedHTMLProps<
        React.HTMLAttributes<HTMLElement>,
        HTMLElement
      >
    }
  }
}

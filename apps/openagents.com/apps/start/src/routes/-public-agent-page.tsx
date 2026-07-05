const eyebrowClass =
  'm-0 font-mono text-xs font-semibold uppercase leading-none tracking-wide text-khala-energy-soft'

// Display-name mapping ported verbatim from the Foldkit original
// (apps/web/src/page/loggedOut/page/publicAgent.ts `displayName`).
export const publicAgentDisplayName = (agentRef: string): string =>
  agentRef === 'artanis'
    ? 'Artanis'
    : agentRef === 'adjutant'
      ? 'Autopilot'
      : agentRef

// `openagents.com/agents/{agentRef}` (and the short `/adjutant` alias) — the
// generic public-agent shell. The Foldkit `view()` function only renders the
// richer goal/activity content once `model.publicAgent` reaches
// `PublicAgentLoaded` or `PublicAgentFailed` for this exact `agentRef`; every
// other case (including the true first-paint `PublicAgentIdle` state) falls
// through to this exact minimal shell — eyebrow, agent name, and a single
// "Loading public goal." line. No prior TS-6 Start route has wired a live
// client fetch on a standalone page, so this port renders that same honest
// first-paint shell rather than fabricating goal/activity data. `/artanis`
// is the one exception: the Foldkit view always renders the full recruitment
// console for that ref regardless of load state, ported separately as
// `ArtanisConsolePage`.
export function PublicAgentPage({ agentRef }: Readonly<{ agentRef: string }>) {
  const agentName = publicAgentDisplayName(agentRef)

  return (
    <main
      className="min-h-dvh bg-khala-void text-khala-text"
      data-agent={agentRef}
      data-component="public-agent-page"
      data-route="public-agent"
    >
      <div className="mx-auto grid min-h-dvh w-full max-w-5xl content-start gap-6 px-6 py-10 font-mono sm:px-8">
        <p className={eyebrowClass}>Public agent</p>
        <h1 className="m-0 text-3xl font-semibold text-khala-text">
          {agentName}
        </h1>
        <p className="m-0 text-sm text-khala-text-muted">
          Loading public goal.
        </p>
      </div>
    </main>
  )
}

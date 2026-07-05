import { createFileRoute } from '@tanstack/react-router'

import { ArtanisConsolePage } from '../-artanis-console-page'
import {
  publicAgentDisplayName,
  PublicAgentPage,
} from '../-public-agent-page'

// `openagents.com/agents/{agentRef}` — the canonical parameterized public-agent
// route. The Foldkit `urlToAppRoute` resolves both this path and the short
// `/artanis` / `/adjutant` aliases to the same `PublicAgentRoute({ agentRef })`
// value, and `view(model, agentRef)` renders identically regardless of which
// URL got there. This route reproduces that parity: `agentRef === 'artanis'`
// renders the same full recruitment console as the standalone `/artanis`
// route, and every other ref (including `adjutant`, matching the standalone
// `/adjutant` alias) renders the generic honest-empty-state shell.
export const Route = createFileRoute('/agents/$agentRef')({
  component: AgentRefPage,
  head: ({ params }) => ({
    meta: [
      {
        title: `${publicAgentDisplayName(params.agentRef)} - OpenAgents`,
      },
    ],
  }),
})

function AgentRefPage() {
  const { agentRef } = Route.useParams()

  return agentRef === 'artanis' ? (
    <ArtanisConsolePage />
  ) : (
    <PublicAgentPage agentRef={agentRef} />
  )
}

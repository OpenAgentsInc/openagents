import { createFileRoute } from '@tanstack/react-router'

import { PublicAgentPage } from './-public-agent-page'

export const Route = createFileRoute('/adjutant')({
  component: () => <PublicAgentPage agentRef="adjutant" />,
  head: () => ({
    meta: [{ title: 'Autopilot - OpenAgents' }],
  }),
})

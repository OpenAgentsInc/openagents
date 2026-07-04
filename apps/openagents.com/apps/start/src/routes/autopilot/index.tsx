import { createFileRoute } from '@tanstack/react-router'

import { AutopilotPage } from '../-funnel-components'

export const Route = createFileRoute('/autopilot/')({
  component: AutopilotPage,
  head: () => ({
    meta: [
      { title: 'Autopilot - OpenAgents' },
      {
        name: 'description',
        content:
          'Describe what you want done. Autopilot scopes the work, shows you a quick win, and keeps a human-review gate before anything ships.',
      },
    ],
  }),
})

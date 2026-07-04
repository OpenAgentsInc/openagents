import { createFileRoute } from '@tanstack/react-router'

import { AutopilotPage } from '../-funnel-components'

export const Route = createFileRoute('/autopilot/legal')({
  component: LegalAutopilotPage,
  head: () => ({
    meta: [
      { title: 'Autopilot for legal teams - OpenAgents' },
      {
        name: 'description',
        content:
          'Describe your legal work. Autopilot scopes it, shows you a quick win, and keeps a review gate before anything is sent.',
      },
    ],
  }),
})

function LegalAutopilotPage() {
  return <AutopilotPage legal />
}

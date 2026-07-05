import { createFileRoute } from '@tanstack/react-router'

import { OnboardingPage } from './-onboarding-page'

export const Route = createFileRoute('/onboarding')({
  component: OnboardingPage,
  head: () => ({
    meta: [
      { title: 'Stop Babysitting Your AI - OpenAgents' },
      {
        name: 'description',
        content:
          'OpenAgents Autopilot: launch coding agents, close your laptop, and stay in the loop from anywhere.',
      },
    ],
  }),
})

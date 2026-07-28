import { createFileRoute } from '@tanstack/react-router'

import { WorkPage } from '../-work-page'

// 2026-07-28 owner direction: public sales landing page. Launch-ui sections,
// AI-employees positioning, CTAs drive into the Sarah sales agent at
// sarah.openagents.com.
export const Route = createFileRoute('/work/')({
  component: WorkPage,
  head: () => ({
    meta: [
      { title: 'AI employees that work — OpenAgents' },
      {
        name: 'description',
        content:
          'OpenAgents builds AI employees and agent fleets that do real business work — software, lead generation, QA, operations — with human verification and receipts on every accepted outcome. Talk to Sarah, our AI sales employee.',
      },
    ],
  }),
})

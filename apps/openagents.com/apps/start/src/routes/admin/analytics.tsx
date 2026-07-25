import { createFileRoute } from '@tanstack/react-router'

import { AdminAnalyticsPage } from '../-admin-analytics-page'

export const Route = createFileRoute('/admin/analytics')({
  component: AdminAnalyticsPage,
  head: () => ({
    meta: [
      { title: 'Website analytics - OpenAgents' },
      { name: 'robots', content: 'noindex, nofollow' },
    ],
  }),
})

import { createFileRoute } from '@tanstack/react-router'

import { SalesLandingPage } from '../-sales-landing-page'

// WEB-1 (#8565) sales-landing PREVIEW route. Clearly-marked, review-only, and
// rollback-safe: it lives at `/preview/sales-landing` alongside the existing
// `/preview/landing` and does not touch the app root or any product route.
export const Route = createFileRoute('/preview/sales-landing')({
  component: SalesLandingPage,
  head: () => ({
    meta: [
      { title: 'Sales landing preview — OpenAgents' },
      {
        name: 'description',
        content:
          'Review-only OpenAgents sales landing candidate (WEB-1 #8565): launch-ui sections, Protoss-blue theme, live public counters and plan catalog. Copy pending owner sign-off; not the live homepage.',
      },
      { name: 'robots', content: 'noindex,nofollow' },
    ],
  }),
})

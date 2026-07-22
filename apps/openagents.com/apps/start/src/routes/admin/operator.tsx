import { createFileRoute } from '@tanstack/react-router'

import { AdminOperatorPage } from '../-admin-operator-page'

// #9188: admin-only operator dashboard. Auth is enforced server-side by the
// `/api/admin/operator/overview` endpoint (`isOpenAgentsAdminEmail`); a
// non-admin session receives a 403 and the page renders the refusal view.
// `admin/operator.tsx` with no sibling `admin.tsx` layout keeps this a plain
// leaf route (see the note in `artanis/index.tsx`).
export const Route = createFileRoute('/admin/operator')({
  component: AdminOperatorRoutePage,
  head: () => ({
    meta: [
      { title: 'Operator dashboard - OpenAgents' },
      {
        name: 'robots',
        content: 'noindex, nofollow',
      },
    ],
  }),
})

function AdminOperatorRoutePage() {
  return <AdminOperatorPage />
}

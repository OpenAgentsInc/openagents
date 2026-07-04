import { createFileRoute } from '@tanstack/react-router'

import { BusinessKpiPage } from '../../-business-kpi-page'

export const Route = createFileRoute('/business/kpi/$engagementRef')({
  component: BusinessKpiRoutePage,
  head: ({ params }) => ({
    meta: [
      { title: `KPI ${params.engagementRef} - OpenAgents` },
      {
        name: 'description',
        content:
          'Public-safe OpenAgents business KPI scorekeeper rendered through the TanStack Start staging app.',
      },
    ],
  }),
})

function BusinessKpiRoutePage() {
  const { engagementRef } = Route.useParams()

  return <BusinessKpiPage engagementRef={engagementRef} />
}

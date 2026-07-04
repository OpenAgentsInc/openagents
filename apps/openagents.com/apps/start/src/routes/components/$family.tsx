import { createFileRoute } from '@tanstack/react-router'

import { ComponentsPage } from '../-components-page'

export const Route = createFileRoute('/components/$family')({
  component: ComponentsFamilyPage,
  head: ({ params }) => ({
    meta: [
      { title: `${params.family} components - OpenAgents` },
      {
        name: 'description',
        content:
          'OpenAgents component family contract rendered through the TanStack Start staging app.',
      },
    ],
  }),
})

function ComponentsFamilyPage() {
  const { family } = Route.useParams()

  return <ComponentsPage selectedFamily={family} />
}

import { createFileRoute } from '@tanstack/react-router'

import { ComponentsPage } from '../-components-page'

export const Route = createFileRoute('/components/')({
  component: ComponentsPage,
  head: () => ({
    meta: [
      { title: 'Component library - OpenAgents' },
      {
        name: 'description',
        content:
          'Internal OpenAgents component library workbench rendered through the TanStack Start staging app.',
      },
    ],
  }),
})

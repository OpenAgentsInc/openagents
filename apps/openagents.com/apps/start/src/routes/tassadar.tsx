import { createFileRoute } from '@tanstack/react-router'

import { TassadarInfoPage } from './-app-shell-routes'

export const Route = createFileRoute('/tassadar')({
  component: TassadarInfoPage,
  head: () => ({
    meta: [
      { title: 'Tassadar - OpenAgents' },
      {
        name: 'description',
        content:
          'Tassadar is OpenAgents open distributed AI model training run with replay verification and public receipts.',
      },
    ],
  }),
})

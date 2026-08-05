import { createFileRoute } from '@tanstack/react-router'

import { TassadarPage } from './-tassadar-page'

export const Route = createFileRoute('/tassadar')({
  component: TassadarPage,
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

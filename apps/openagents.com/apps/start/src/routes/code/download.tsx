import { createFileRoute } from '@tanstack/react-router'

import { KhalaCodeDownloadPage } from '../-funnel-components'

export const Route = createFileRoute('/code/download')({
  component: KhalaCodeDownloadPage,
  head: () => ({
    meta: [
      { title: 'Khala Code install paths - OpenAgents' },
      {
        name: 'description',
        content:
          'Install paths for Khala Code, with the Codex requirement kept visible.',
      },
    ],
  }),
})

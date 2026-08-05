import { createFileRoute } from '@tanstack/react-router'

import {
  DownloadPage,
  downloadPageDescription,
  downloadPageStructuredData,
} from './-download-page'
import { loadOmegaDownloadResolution } from './-omega-download-data'

export const Route = createFileRoute('/download')({
  // The Electron OpenAgents Desktop entry and its resolver are retired, so
  // `/download` is the Omega download page. The Omega entry loads from the
  // signed Omega download manifest: during SSR it is verified in-process, so
  // the no-JavaScript page is the fully resolved page, and a verification
  // failure server-renders the honest unavailable state with zero URLs.
  loader: async () => {
    const omega = await loadOmegaDownloadResolution()
    return { omega }
  },
  component: DownloadRoute,
  head: ({ loaderData }) => {
    const structuredData = downloadPageStructuredData(loaderData?.omega)
    return {
      meta: [
        { title: 'Download Omega' },
        { name: 'description', content: downloadPageDescription(loaderData?.omega) },
        { name: 'theme-color', content: '#05070d' },
      ],
      links: [{ rel: 'canonical', href: 'https://openagents.com/download' }],
      ...(structuredData === null
        ? {}
        : {
            scripts: [
              { type: 'application/ld+json', children: structuredData },
            ],
          }),
    }
  },
})

function DownloadRoute() {
  const { omega } = Route.useLoaderData()
  return <DownloadPage omega={omega} />
}

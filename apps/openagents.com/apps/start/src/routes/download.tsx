import { createFileRoute } from '@tanstack/react-router'

import {
  loadDesktopDownloadResolution,
  parseDownloadSearch,
  type DownloadSearch,
} from './-download-data'
import {
  DownloadPage,
  downloadPageDescription,
  downloadPageStructuredData,
} from './-download-page'

export const Route = createFileRoute('/download')({
  validateSearch: (search): DownloadSearch => parseDownloadSearch(search),
  loaderDeps: ({ search }) => search,
  // DIST-11 (#8924): the page renders exclusively from the DIST-10 resolver
  // projection. During SSR the resolver runs in-process against the incoming
  // request headers, so the no-JavaScript page is the fully resolved page; a
  // feed failure server-renders the honest unavailable state.
  loader: ({ deps }) => loadDesktopDownloadResolution(deps),
  component: DownloadRoute,
  head: ({ loaderData }) => {
    const structuredData = downloadPageStructuredData(loaderData)
    return {
      meta: [
        { title: 'Download OpenAgents Desktop' },
        { name: 'description', content: downloadPageDescription(loaderData) },
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
  return <DownloadPage resolution={Route.useLoaderData()} />
}

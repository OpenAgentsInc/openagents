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
import { loadOmegaDownloadResolution } from './-omega-download-data'

export const Route = createFileRoute('/download')({
  validateSearch: (search): DownloadSearch => parseDownloadSearch(search),
  loaderDeps: ({ search }) => search,
  // DIST-11 (#8924): the Desktop entry renders exclusively from the DIST-10
  // resolver projection. During SSR the resolver runs in-process against the
  // incoming request headers, so the no-JavaScript page is the fully resolved
  // page; a feed failure server-renders the honest unavailable state.
  // #9280: the SEPARATE Omega product entry loads in parallel from the signed
  // Omega download manifest and fails soft the same way.
  loader: async ({ deps }) => {
    const [desktop, omega] = await Promise.all([
      loadDesktopDownloadResolution(deps),
      loadOmegaDownloadResolution(),
    ])
    return { desktop, omega }
  },
  component: DownloadRoute,
  head: ({ loaderData }) => {
    const structuredData = downloadPageStructuredData(loaderData?.desktop)
    return {
      meta: [
        { title: 'Download OpenAgents Desktop' },
        { name: 'description', content: downloadPageDescription(loaderData?.desktop) },
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
  const { desktop, omega } = Route.useLoaderData()
  return <DownloadPage omega={omega} resolution={desktop} />
}

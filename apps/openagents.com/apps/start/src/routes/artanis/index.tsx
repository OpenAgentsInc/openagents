import { createFileRoute } from '@tanstack/react-router'

import { ArtanisConsolePage } from '../-artanis-console-page'

// `X/index.tsx` + `createFileRoute('/X/')` — the established convention
// (see `autopilot/index.tsx`, `code/index.tsx`, `khala/index.tsx`,
// `components/index.tsx`) for avoiding the nested-route layout footgun: a
// bare `artanis.tsx` sibling would become an implicit TanStack Router layout
// for the existing `artanis/traces.tsx` and `artanis/accounts.tsx` children,
// and since no page component here renders `<Outlet />`, that would silently
// serve this page's content in place of those children's.
export const Route = createFileRoute('/artanis/')({
  component: ArtanisConsolePage,
  head: () => ({
    meta: [
      { title: 'Artanis - OpenAgents' },
      {
        name: 'description',
        content:
          'The live Artanis fleet recruitment console: the Pylon fleet map, active task board, virtual merge queue, and how to connect your own Codex or Claude capacity.',
      },
    ],
  }),
})

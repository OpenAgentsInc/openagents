import { createFileRoute } from '@tanstack/react-router'

import { SharePage } from '../-share-page'

// `openagents.com/share/{shareId}` — the shared workroom-timeline viewer,
// ported from `apps/web/src/page/loggedOut/page/share.ts`. No sibling
// `share.tsx` file exists, so this is a single leaf route with no
// nested-route-layout risk (the footgun fixed across five other routes in
// TS-6 Slice 15).
export const Route = createFileRoute('/share/$shareId')({
  component: ShareRoutePage,
  head: () => ({
    meta: [{ title: 'Shared conversation - OpenAgents' }],
  }),
})

function ShareRoutePage() {
  const { shareId } = Route.useParams()

  return <SharePage shareId={shareId} />
}

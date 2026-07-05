import { createFileRoute } from '@tanstack/react-router'

import { TrainingRunsPage } from '../../-training-runs-page'

// `openagents.com/training/runs/{runId}` — the per-run detail alias of the
// already-migrated `/training/runs` list route. The Foldkit
// `publicTrainingRunRouter` (apps/web/src/route.ts) parses this exact path
// shape (`training/runs/{runId}`) into `PublicTrainingRunRoute({ runId })`,
// and `loggedOut/view.ts` renders it with the very same
// `TrainingRuns.view(model.publicTrainingRuns, route.runId)` function the
// list route uses with `runId: null`. See `-training-runs-page.tsx` for why
// the honest Idle-state markup is identical between the two routes.
export const Route = createFileRoute('/training/runs/$runId')({
  component: TrainingRunDetailPage,
  head: ({ params }) => ({
    meta: [
      { title: `Training run ${params.runId} - OpenAgents` },
      {
        name: 'description',
        content:
          'Public CS336 training run state, verification, and settlement projection, rendered through the TanStack Start staging app.',
      },
    ],
  }),
})

function TrainingRunDetailPage() {
  const { runId } = Route.useParams()

  return <TrainingRunsPage runId={runId} />
}

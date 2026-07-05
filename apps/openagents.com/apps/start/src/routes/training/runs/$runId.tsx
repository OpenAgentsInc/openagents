import { createFileRoute } from '@tanstack/react-router'

import { TrainingRunsDeprecatedPage } from '../../-training-runs-deprecated-page'

// `openagents.com/training/runs/{runId}` — the per-run detail alias of the
// `/training/runs` list route. Deprecated-for-now (owner decision,
// 2026-07-05) along with the list route — see
// -training-runs-deprecated-page.tsx and the TS-6 tracking doc. The real
// per-run detail component stays dormant in `-training-runs-page.tsx`
// (`TrainingRunsPage` with a `runId` prop) for restoration; this route just
// no longer renders it.
export const Route = createFileRoute('/training/runs/$runId')({
  component: TrainingRunsDeprecatedPage,
  head: () => ({
    meta: [
      { title: 'Training Runs (temporarily unavailable) - OpenAgents' },
      {
        name: 'description',
        content:
          'The public CS336 training run detail view is temporarily unavailable while this feature is reworked.',
      },
    ],
  }),
})

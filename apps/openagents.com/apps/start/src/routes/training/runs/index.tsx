import { createFileRoute } from '@tanstack/react-router'

import { TrainingRunsDeprecatedPage } from '../../-training-runs-deprecated-page'

// Deprecated-for-now (owner decision, 2026-07-05) — see
// -training-runs-deprecated-page.tsx and the TS-6 tracking doc. The real
// `TrainingRunsPage` component stays dormant in `-training-runs-page.tsx`
// for restoration; this route just no longer renders it.
export const Route = createFileRoute('/training/runs/')({
  component: TrainingRunsDeprecatedPage,
  head: () => ({
    meta: [
      { title: 'Training Runs (temporarily unavailable) - OpenAgents' },
      {
        name: 'description',
        content:
          'The public CS336 training run listing is temporarily unavailable while this feature is reworked.',
      },
    ],
  }),
})

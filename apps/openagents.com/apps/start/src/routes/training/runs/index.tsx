import { createFileRoute } from '@tanstack/react-router'

import { TrainingRunsPage } from '../../-training-runs-page'

export const Route = createFileRoute('/training/runs/')({
  component: TrainingRunsPage,
  head: () => ({
    meta: [
      { title: 'Training Runs - OpenAgents' },
      {
        name: 'description',
        content:
          'Public CS336 training run state, verification, and settlement projection, rendered through the TanStack Start staging app.',
      },
    ],
  }),
})

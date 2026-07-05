import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { TrainingRunsPage } from './-training-runs-page'

describe('Start /training/runs route', () => {
  test('server-renders the public run-state header', () => {
    const html = renderToStaticMarkup(<TrainingRunsPage />)

    expect(html).toContain('data-route="training-runs"')
    expect(html).toContain('Training Runs')
    expect(html).toContain(
      'Public CS336 run state, verification, and settlement projection.',
    )
    expect(html).toContain('Backed by /api/training/runs.')
  })

  test('renders the honest idle empty state instead of fabricated run rows', () => {
    const html = renderToStaticMarkup(<TrainingRunsPage />)

    expect(html).toContain(
      'No Worker-authoritative training runs are recorded yet.',
    )
    expect(html).toContain('No run projection is available for this route.')
    expect(html).toContain('/api/training/runs')
    expect(html).toContain('/api/training/leaderboards')
  })

  test('does not render a data-run-id attribute when no runId is given', () => {
    const html = renderToStaticMarkup(<TrainingRunsPage />)

    expect(html).not.toContain('data-run-id')
  })
})

describe('Start /training/runs/$runId route', () => {
  test('server-renders the same honest header and empty-state copy as the list route', () => {
    const html = renderToStaticMarkup(
      <TrainingRunsPage runId="run.cs336.a1.demo" />,
    )

    expect(html).toContain('data-route="training-runs"')
    expect(html).toContain('Training Runs')
    expect(html).toContain(
      'Public CS336 run state, verification, and settlement projection.',
    )
    expect(html).toContain(
      'No Worker-authoritative training runs are recorded yet.',
    )
    expect(html).toContain('No run projection is available for this route.')
    expect(html).toContain('/api/training/runs')
    expect(html).toContain('/api/training/leaderboards')
  })

  test('surfaces the requested runId via data-run-id without fabricating divergent content', () => {
    const html = renderToStaticMarkup(
      <TrainingRunsPage runId="run.cs336.a1.demo" />,
    )

    expect(html).toContain('data-run-id="run.cs336.a1.demo"')
  })
})

import { describe, expect, it } from 'vitest'

import { tassadarSpacetimeWorldSubscriptionQueries } from './tassadarSpacetimeWorld'

describe('tassadar SpacetimeDB subscription queries', () => {
  it('subscribes to timeline-backed public activity world events', () => {
    const queries = tassadarSpacetimeWorldSubscriptionQueries(
      'run.tassadar.executor.20260615',
    )

    expect(queries).toContain(
      "SELECT * FROM world_event WHERE run_ref = 'run.tassadar.executor.20260615'",
    )
    expect(queries).toContain(
      "SELECT * FROM world_event WHERE run_ref = 'run.public_activity_timeline'",
    )
  })
})

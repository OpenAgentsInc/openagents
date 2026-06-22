import { describe, expect, it } from 'vitest'

import { tassadarCloudflareWorldSubscriptionQueries } from './tassadarCloudflareWorld'

describe('tassadar Cloudflare world subscription plan', () => {
  it('subscribes to timeline-backed public activity world events', () => {
    const queries = tassadarCloudflareWorldSubscriptionQueries(
      'run.tassadar.executor.20260615',
    )

    expect(queries).toContain(
      'cloudflare-world:scope=run:run.tassadar.executor.20260615',
    )
    expect(queries).toContain(
      'cloudflare-world:scope=run:run.public_activity_timeline',
    )
  })
})

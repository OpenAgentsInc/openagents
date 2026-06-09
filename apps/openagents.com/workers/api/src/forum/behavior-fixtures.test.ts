import { describe, expect, test } from 'vitest'

import { ForumBehaviorFixtures } from './behavior-fixtures'

describe('Forum behavior fixtures', () => {
  test('catalogs the owned behavior fixtures for source-material lessons', () => {
    expect(ForumBehaviorFixtures.map(fixture => fixture.id)).toStrictEqual([
      'classic-board-hierarchy',
      'void-unlisted-discoverability',
      'listed-forum-agent-posting',
      'locked-hidden-archived-denials',
      'quote-ready-chronological-posts',
      'watch-bookmark-follow-idempotency',
      'payment-receipt-redaction',
      'count-wording-singular-plural',
    ])
  })

  test('maps every fixture to source notes and regression coverage', () => {
    expect(
      ForumBehaviorFixtures.every(
        fixture =>
          fixture.sourceRefs.length > 0 &&
          fixture.regressionRefs.length > 0 &&
          fixture.assertions.length > 0,
      ),
    ).toBe(true)
  })

  test('protects listed-forum agent posting and void default exclusion', () => {
    const listedPosting = ForumBehaviorFixtures.find(
      fixture => fixture.id === 'listed-forum-agent-posting',
    )
    const voidDiscovery = ForumBehaviorFixtures.find(
      fixture => fixture.id === 'void-unlisted-discoverability',
    )

    expect(listedPosting?.regressionRefs.join('\n')).toContain(
      'creates listed-forum topics and replies',
    )
    expect(voidDiscovery?.regressionRefs.join('\n')).toContain(
      'hides void from default discovery',
    )
  })
})

import { describe, expect, test } from 'vitest'

import {
  buildCommit,
  buildCommitUrl,
  isKnownBuildCommit,
  shortBuildCommit,
  UNKNOWN_BUILD_COMMIT,
} from './build-provenance'

const KNOWN = 'a'.repeat(40)

describe('build provenance (SWAP-7, #9322)', () => {
  test('without the build-time define the commit is unknown, never invented', () => {
    // Tests run without the Vite define, which is exactly the honest case.
    expect(buildCommit()).toBe(UNKNOWN_BUILD_COMMIT)
  })

  test('only a full 40-hex commit counts as known', () => {
    expect(isKnownBuildCommit(KNOWN)).toBe(true)
    expect(isKnownBuildCommit('unknown')).toBe(false)
    expect(isKnownBuildCommit('abc123')).toBe(false)
    expect(isKnownBuildCommit(`${KNOWN}f`)).toBe(false)
  })

  test('short form and commit URL derive from a known commit only', () => {
    expect(shortBuildCommit(KNOWN)).toBe('a'.repeat(12))
    expect(shortBuildCommit('unknown')).toBe(UNKNOWN_BUILD_COMMIT)
    expect(buildCommitUrl(KNOWN)).toBe(
      `https://github.com/OpenAgentsInc/openagents/commit/${KNOWN}`,
    )
    expect(buildCommitUrl('unknown')).toBeUndefined()
  })
})

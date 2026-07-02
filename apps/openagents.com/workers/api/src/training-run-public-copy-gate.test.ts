import { readFileSync } from 'node:fs'
import { describe, expect, test } from 'vitest'

const publicTrainingRunSources = [
  'src/training-run-window-authority.ts',
  'src/training-run-window-routes.ts',
  '../../apps/web/src/page/loggedOut/page/trainingRuns.ts',
  '../../docs/live/AGENTS.md',
  '../../docs/live/PYLON.md',
]

const forbiddenCopyPatterns: ReadonlyArray<readonly [string, RegExp]> = [
  [
    'unbounded model training claim',
    /training (?:a|the|our|your)? ?model on (?:people'?s|contributors'?|users'?) devices/i,
  ],
  [
    'pending displayed as paid',
    /pending (?:payouts?|payments?|settlements?) (?:are|count as) (?:paid|settled)/i,
  ],
  [
    'paid displayed without provider confirmation',
    /(?:paid|settled) payouts? include (?:pending|wallet-side|offered|claimed)/i,
  ],
]

describe('public training run copy gate', () => {
  test('keeps public training run copy inside the provenance and settlement boundary', () => {
    const violations = publicTrainingRunSources.flatMap(path => {
      const source = readFileSync(path, 'utf8')

      return forbiddenCopyPatterns
        .filter(([, pattern]) => pattern.test(source))
        .map(([label]) => `${path}: ${label}`)
    })

    expect(violations).toEqual([])
  })
})

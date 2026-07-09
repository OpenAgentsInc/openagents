import { describe, expect, test } from 'bun:test'

import {
  evaluateSalesQualityCase,
  loadSalesQualityFixtures,
  runSalesQualityPack,
  wordCount,
} from './sales-quality-eval.mjs'

describe('SQ-5 sales-quality eval pack (#8622)', () => {
  test('all fixture cases confirm under hard oracles', () => {
    const pack = runSalesQualityPack()
    expect(pack.summary.refuted).toBe(0)
    expect(pack.summary.confirmed).toBe(pack.summary.total)
    expect(pack.summary.total).toBe(7)
  })

  test('pain hunting rejects multi-question turn', () => {
    const r = evaluateSalesQualityCase({
      id: 'pain_hunting_first_two_turns',
      assistantTurns: [
        'What hurts? And who cares?',
        'How many tickets?',
      ],
    })
    expect(r.status).toBe('REFUTED')
  })

  test('voice length gate', () => {
    expect(wordCount('one two three')).toBe(3)
    const long = {
      id: 'voice_length_avatar_safe',
      maxWords: 5,
      assistantText: 'one two three four five six seven',
    }
    expect(evaluateSalesQualityCase(long).status).toBe('REFUTED')
  })

  test('fixtures load with schema', () => {
    const f = loadSalesQualityFixtures()
    expect(f.schema).toBe('sarah.sales_quality_fixtures.v1')
    expect(f.cases.length).toBe(7)
  })
})

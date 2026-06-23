import { Option } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  DEFAULT_SPECULATION_POLICY,
  NOT_MEASURED,
  NO_SPECULATION,
  UNKNOWN_SPECULATION,
  buildKhalaSpeculationMetadata,
  decideSpeculation,
  decodeKhalaSpeculationMetadata,
  isDraftFreeMode,
  isLearnedMode,
} from './khala-speculation'

describe('khala speculation — mode classifiers', () => {
  test('draft-free modes are exactly n-gram + lookahead (Worker-runnable today)', () => {
    expect(isDraftFreeMode('n_gram')).toBe(true)
    expect(isDraftFreeMode('lookahead')).toBe(true)
    expect(isDraftFreeMode('eagle')).toBe(false)
    expect(isDraftFreeMode('none')).toBe(false)
    expect(isDraftFreeMode('not_measured')).toBe(false)
  })

  test('eagle is the only learned mode (the later Psionic / hidden-state lane)', () => {
    expect(isLearnedMode('eagle')).toBe(true)
    expect(isLearnedMode('n_gram')).toBe(false)
    expect(isLearnedMode('lookahead')).toBe(false)
    expect(isLearnedMode('none')).toBe(false)
  })
})

describe('khala speculation — honest builder (no fabricated acceptance)', () => {
  test('absent mode yields the honest-unknown shape, never a fabricated rate', () => {
    const meta = buildKhalaSpeculationMetadata({})
    expect(meta).toEqual(UNKNOWN_SPECULATION)
    expect(meta.mode).toBe('not_measured')
    expect(meta.active).toBe(false)
    expect(meta.acceptanceRate).toBe(NOT_MEASURED)
    expect(meta.draftTokensProposed).toBe(NOT_MEASURED)
    expect(meta.draftTokensAccepted).toBe(NOT_MEASURED)
  })

  test('explicit mode `none` is the KNOWN no-speculation shape (not the unknown one)', () => {
    const meta = buildKhalaSpeculationMetadata({ mode: 'none' })
    expect(meta).toEqual(NO_SPECULATION)
    expect(meta.mode).toBe('none')
    expect(meta.active).toBe(false)
    // No drafts ran => acceptance is the sentinel, NOT a measured 0 (which would
    // falsely imply a drafter ran and accepted nothing).
    expect(meta.acceptanceRate).toBe(NOT_MEASURED)
  })

  test('a real drafting pass records counts + a derived acceptance rate', () => {
    const meta = buildKhalaSpeculationMetadata({
      mode: 'n_gram',
      draftTokensProposed: 100,
      draftTokensAccepted: 78,
    })
    expect(meta.mode).toBe('n_gram')
    expect(meta.active).toBe(true)
    expect(meta.acceptanceRate).toBe(0.78)
    expect(meta.draftTokensProposed).toBe(100)
    expect(meta.draftTokensAccepted).toBe(78)
  })

  test('acceptance rate is the sentinel when counts are missing (never a bare number)', () => {
    const meta = buildKhalaSpeculationMetadata({
      mode: 'lookahead',
      draftTokensProposed: 50,
      // accepted omitted
    })
    expect(meta.acceptanceRate).toBe(NOT_MEASURED)
    expect(meta.draftTokensProposed).toBe(50)
    expect(meta.draftTokensAccepted).toBe(NOT_MEASURED)
  })

  test('zero proposals => rate is the sentinel (a rate over zero proposals is undefined, not 0)', () => {
    const meta = buildKhalaSpeculationMetadata({
      mode: 'n_gram',
      draftTokensProposed: 0,
      draftTokensAccepted: 0,
    })
    expect(meta.acceptanceRate).toBe(NOT_MEASURED)
  })

  test('accepted is clamped to <= proposed so a malformed disclosure never exceeds rate 1', () => {
    const meta = buildKhalaSpeculationMetadata({
      mode: 'n_gram',
      draftTokensProposed: 10,
      draftTokensAccepted: 99,
    })
    expect(meta.acceptanceRate).toBe(1)
    expect(meta.draftTokensAccepted).toBe(10)
  })

  test('the built metadata round-trips through the decoder', () => {
    const meta = buildKhalaSpeculationMetadata({
      mode: 'n_gram',
      draftTokensProposed: 100,
      draftTokensAccepted: 70,
    })
    const decoded = decodeKhalaSpeculationMetadata(meta)
    expect(Option.isSome(decoded)).toBe(true)
  })
})

describe('decideSpeculation — dynamic disablement (the book operating rule)', () => {
  test('ENABLES a draft-free mode at low batch + low pressure', () => {
    const decision = decideSpeculation({
      requestedMode: 'n_gram',
      signal: { batchSize: 1, computePressure: 0.1 },
    })
    expect(decision.enabled).toBe(true)
    expect(decision.selectedMode).toBe('n_gram')
    expect(decision.reason).toBe('enabled_low_batch')
  })

  test('lookahead enables at the batch boundary (batch == maxProfitableBatchSize)', () => {
    const decision = decideSpeculation({
      requestedMode: 'lookahead',
      signal: {
        batchSize: DEFAULT_SPECULATION_POLICY.maxProfitableBatchSize,
        computePressure: 0,
      },
    })
    expect(decision.enabled).toBe(true)
    expect(decision.selectedMode).toBe('lookahead')
  })

  test('DISABLES above the batch threshold (high concurrency => verification hurts throughput)', () => {
    const decision = decideSpeculation({
      requestedMode: 'n_gram',
      signal: {
        batchSize: DEFAULT_SPECULATION_POLICY.maxProfitableBatchSize + 1,
        computePressure: 0.1,
      },
    })
    expect(decision.enabled).toBe(false)
    expect(decision.selectedMode).toBe('none')
    expect(decision.reason).toBe('disabled_high_batch')
  })

  test('DISABLES above the compute-pressure threshold (no spare compute to verify drafts)', () => {
    const decision = decideSpeculation({
      requestedMode: 'n_gram',
      signal: {
        batchSize: 1,
        computePressure:
          DEFAULT_SPECULATION_POLICY.maxProfitableComputePressure + 0.01,
      },
    })
    expect(decision.enabled).toBe(false)
    expect(decision.reason).toBe('disabled_high_pressure')
  })

  test('DISABLES conservatively when the pressure signal is unknown', () => {
    expect(
      decideSpeculation({
        requestedMode: 'n_gram',
        signal: { batchSize: NOT_MEASURED, computePressure: 0.1 },
      }).reason,
    ).toBe('disabled_pressure_unknown')
    expect(
      decideSpeculation({
        requestedMode: 'n_gram',
        signal: { batchSize: 1, computePressure: NOT_MEASURED },
      }).reason,
    ).toBe('disabled_pressure_unknown')
  })

  test('DISABLES a learned/unavailable mode (eagle is the Psionic lane, not Worker-runnable)', () => {
    const decision = decideSpeculation({
      requestedMode: 'eagle',
      signal: { batchSize: 1, computePressure: 0 },
    })
    expect(decision.enabled).toBe(false)
    expect(decision.reason).toBe('disabled_mode_unavailable')
  })

  test('DISABLES when speculation was not requested (chat / none)', () => {
    expect(
      decideSpeculation({
        requestedMode: 'none',
        signal: { batchSize: 1, computePressure: 0 },
      }).reason,
    ).toBe('disabled_not_requested')
  })

  test('a custom policy widens/narrows the profitable window deterministically', () => {
    const decision = decideSpeculation({
      requestedMode: 'n_gram',
      signal: { batchSize: 8, computePressure: 0.5 },
      policy: { maxProfitableBatchSize: 16, maxProfitableComputePressure: 0.9 },
    })
    expect(decision.enabled).toBe(true)
  })

  test('PURE: same inputs => same decision', () => {
    const input = {
      requestedMode: 'n_gram' as const,
      signal: { batchSize: 2, computePressure: 0.2 },
    }
    expect(decideSpeculation(input)).toEqual(decideSpeculation(input))
  })
})

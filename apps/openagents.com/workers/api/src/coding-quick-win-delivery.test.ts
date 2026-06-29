import { describe, expect, test } from 'vitest'

import {
  assertCodingQuickWinDeliverable,
  buildCodingQuickWinDeliveryEvidence,
  CodingQuickWinDeliveryInvariantError,
  codingQuickWinDeliveredEvidenceRef,
  publicCodingQuickWinDeliveryProjection,
  type CodingQuickWinDeliveryInput,
} from './coding-quick-win-delivery'

const passing: CodingQuickWinDeliveryInput = {
  repo: 'example/checkout',
  baseRef: 'a1b2c3d',
  verificationCommand: 'bun test src/checkout',
  verificationExitCode: 0,
  verificationOutputRef: 'log:run-001',
  diffRef: 'https://github.com/example/checkout/pull/42',
}

describe('buildCodingQuickWinDeliveryEvidence', () => {
  test('derives verification_passed and handback-ready for a 0 exit code', () => {
    const evidence = buildCodingQuickWinDeliveryEvidence(passing)
    expect(evidence.verificationStatus).toBe('verification_passed')
    expect(evidence.acceptableForHandback).toBe(true)
    expect(evidence.offeringPromiseId).toBe('business.coding_quick_win.v1')
  })

  test('derives verification_failed for a non-zero exit code', () => {
    const evidence = buildCodingQuickWinDeliveryEvidence({
      ...passing,
      verificationExitCode: 1,
    })
    expect(evidence.verificationStatus).toBe('verification_failed')
    expect(evidence.acceptableForHandback).toBe(false)
  })

  test('derives verification_not_run when no exit code is supplied', () => {
    const evidence = buildCodingQuickWinDeliveryEvidence({
      ...passing,
      verificationExitCode: null,
      verificationOutputRef: null,
    })
    expect(evidence.verificationStatus).toBe('verification_not_run')
    expect(evidence.acceptableForHandback).toBe(false)
  })

  test('never trusts a caller-asserted status: derives only from exit code', () => {
    // A non-zero exit can never become passed regardless of other fields.
    const evidence = buildCodingQuickWinDeliveryEvidence({
      ...passing,
      verificationExitCode: 2,
    })
    expect(evidence.verificationStatus).not.toBe('verification_passed')
  })

  test('rejects a passed delivery with no captured output', () => {
    expect(() =>
      buildCodingQuickWinDeliveryEvidence({
        ...passing,
        verificationOutputRef: null,
      }),
    ).toThrow(CodingQuickWinDeliveryInvariantError)
  })

  test('requires repo, baseRef, verificationCommand, and diffRef', () => {
    for (const field of [
      'repo',
      'baseRef',
      'verificationCommand',
      'diffRef',
    ] as const) {
      expect(() =>
        buildCodingQuickWinDeliveryEvidence({ ...passing, [field]: '   ' }),
      ).toThrow(CodingQuickWinDeliveryInvariantError)
    }
  })

  test('is deterministic for identical input', () => {
    expect(buildCodingQuickWinDeliveryEvidence(passing)).toEqual(
      buildCodingQuickWinDeliveryEvidence(passing),
    )
  })

  test('uses default reviewable-not-merged caveat when none supplied', () => {
    const evidence = buildCodingQuickWinDeliveryEvidence(passing)
    expect(evidence.reviewGateCaveatRef).toBe(
      'caveat.coding_quick_win.reviewable_not_merged',
    )
  })
})

describe('assertCodingQuickWinDeliverable', () => {
  test('passes for a verification_passed delivery', () => {
    const evidence = buildCodingQuickWinDeliveryEvidence(passing)
    expect(() => assertCodingQuickWinDeliverable(evidence)).not.toThrow()
  })

  test('rejects a failed delivery', () => {
    const evidence = buildCodingQuickWinDeliveryEvidence({
      ...passing,
      verificationExitCode: 1,
    })
    expect(() => assertCodingQuickWinDeliverable(evidence)).toThrow(
      CodingQuickWinDeliveryInvariantError,
    )
  })
})

describe('codingQuickWinDeliveredEvidenceRef', () => {
  test('returns the diff ref for a handback-ready delivery', () => {
    const evidence = buildCodingQuickWinDeliveryEvidence(passing)
    expect(codingQuickWinDeliveredEvidenceRef(evidence)).toBe(
      'https://github.com/example/checkout/pull/42',
    )
  })

  test('throws for a non-handback-ready delivery (never fakes delivery)', () => {
    const evidence = buildCodingQuickWinDeliveryEvidence({
      ...passing,
      verificationExitCode: 1,
    })
    expect(() => codingQuickWinDeliveredEvidenceRef(evidence)).toThrow(
      CodingQuickWinDeliveryInvariantError,
    )
  })
})

describe('publicCodingQuickWinDeliveryProjection', () => {
  test('keeps status and command but drops internal diff/output refs', () => {
    const projection = publicCodingQuickWinDeliveryProjection(
      buildCodingQuickWinDeliveryEvidence(passing),
    )
    expect(projection.verificationStatus).toBe('verification_passed')
    expect(projection.verificationCommand).toBe('bun test src/checkout')
    expect(projection).not.toHaveProperty('diffRef')
    expect(projection).not.toHaveProperty('verificationOutputRef')
    expect(projection).not.toHaveProperty('baseRef')
  })
})

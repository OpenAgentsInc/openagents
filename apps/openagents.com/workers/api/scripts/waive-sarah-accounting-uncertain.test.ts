import { describe, expect, test } from 'vitest'

import {
  OWNER_GATE,
  type WaiverTarget,
  assertExpectedTargets,
  assertOwnerGate,
  parseArguments,
  publicReceiptFor,
  targetSetDigest,
  waiverPayloadDigest,
} from './waive-sarah-accounting-uncertain'

const repositoryRoot = '/repo'
const common = [
  '--environment',
  'production',
  '--private-output',
  '/private/waiver.json',
  '--reason',
  'Owner waived platform credit accounting',
  '--evidence-ref',
  'issue:9285',
] as const

const targets: ReadonlyArray<WaiverTarget> = [
  {
    sessionRef: 'private-session-ref',
    generation: 1,
    providerSessionRefDigest: 'a'.repeat(64),
    reservedMsat: 256_000,
    recordedChargeMsat: 15,
  },
]

describe('Sarah accounting waiver CLI safety', () => {
  test('defaults to a non-mutating preview and needs no owner gate', () => {
    const input = parseArguments(common, repositoryRoot)
    expect(input.apply).toBe(false)
    expect(() => assertOwnerGate(input.apply, undefined)).not.toThrow()
  })

  test('apply requires both an exact target set and the owner gate', () => {
    expect(() =>
      parseArguments(['--apply', ...common], repositoryRoot),
    ).toThrow()
    const digest = targetSetDigest(targets)
    const input = parseArguments(
      [
        '--apply',
        ...common,
        '--expected-target-count',
        '1',
        '--expected-target-set-digest',
        digest,
      ],
      repositoryRoot,
    )
    expect(() => assertOwnerGate(input.apply, undefined)).toThrow(/OWNER_GATE/u)
    expect(() => assertOwnerGate(input.apply, OWNER_GATE)).not.toThrow()
  })

  test('refuses a changed count or target-set digest', () => {
    expect(() =>
      assertExpectedTargets(targets, 2, targetSetDigest(targets)),
    ).toThrow(/target set changed/u)
    expect(() => assertExpectedTargets(targets, 1, 'b'.repeat(64))).toThrow(
      /target set changed/u,
    )
  })

  test('derives replay-stable per-row authority and redacts public output', () => {
    const input = parseArguments(common, repositoryRoot)
    expect(waiverPayloadDigest(targets[0]!, input)).toBe(
      waiverPayloadDigest(targets[0]!, input),
    )
    const privateReceipt = { targets, provider: 'private-provider-ref' }
    const publicReceipt = publicReceiptFor(
      input,
      targets,
      [{ waiverReceiptRef: 'private-waiver-receipt-ref' }],
      privateReceipt,
    )
    const serialized = JSON.stringify(publicReceipt)
    expect(serialized).not.toContain('private-session-ref')
    expect(serialized).not.toContain('private-provider-ref')
    expect(serialized).not.toContain('private-waiver-receipt-ref')
    expect(publicReceipt).toMatchObject({
      targetCount: 1,
      reservedMsat: 256_000,
      recordedChargeMsat: 15,
      mode: 'preview',
    })
  })
})

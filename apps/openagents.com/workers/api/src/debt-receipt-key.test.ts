import { createHash } from 'node:crypto'
import { describe, expect, test } from 'vitest'

import {
  DebtReceiptKey,
  DebtReceiptKeyUnsafe,
  PatchNoveltyKey,
  debtReceiptKeyShortRef,
  deriveDebtReceiptKey,
  derivePatchNoveltyKey,
  patchNoveltyKeyShortRef,
} from './debt-receipt-key'

const keyInput = {
  debtReceiptRef: 'receipt.public.debt.5334',
  objectiveDigest: 'objective.public.debt_receipt.5334.dual_format_to_zero',
  repoBaselineRef: 'baseline.public.commit.c43992567',
  scopeDigest: 'scope.public.debt_receipt.5334.tassadar_fixture_pairs',
}

describe('Debt receipt fingerprint keys', () => {
  test('DebtReceiptKey = sha256(debtReceiptRef | repoBaselineRef | scopeDigest | objectiveDigest)', () => {
    const expectedDigest = createHash('sha256')
      .update(
        [
          keyInput.debtReceiptRef,
          keyInput.repoBaselineRef,
          keyInput.scopeDigest,
          keyInput.objectiveDigest,
        ]
          .map(part => `${part.length}:${part}`)
          .join('|'),
        'utf8',
      )
      .digest('hex')

    expect(deriveDebtReceiptKey(keyInput)).toBe(
      `debt_receipt_key:${expectedDigest}`,
    )
  })

  test('is deterministic and order-sensitive over its fields', () => {
    expect(deriveDebtReceiptKey(keyInput)).toBe(deriveDebtReceiptKey(keyInput))
    expect(
      deriveDebtReceiptKey({
        ...keyInput,
        scopeDigest: keyInput.objectiveDigest,
        objectiveDigest: keyInput.scopeDigest,
      }),
    ).not.toBe(deriveDebtReceiptKey(keyInput))
  })

  test('PatchNoveltyKey carries the DebtReceiptKey plus patch and behavior digests', () => {
    const debtReceiptKey = deriveDebtReceiptKey(keyInput)
    const patchKey = derivePatchNoveltyKey({
      behaviorReceiptDigest: 'sha256:behavior',
      debtReceiptKey,
      normalizedPatchDigest: 'patch-id:abc',
    })

    expect(patchKey.startsWith('patch_novelty_key:')).toBe(true)
    // a different patch against the same DebtReceiptKey is a distinct novelty key
    expect(
      derivePatchNoveltyKey({
        behaviorReceiptDigest: 'sha256:behavior',
        debtReceiptKey,
        normalizedPatchDigest: 'patch-id:other',
      }),
    ).not.toBe(patchKey)
  })

  test('rejects empty key fields', () => {
    expect(() =>
      deriveDebtReceiptKey({ ...keyInput, scopeDigest: '   ' }),
    ).toThrow(DebtReceiptKeyUnsafe)
  })

  test('short refs are public-safe truncations of the keys', () => {
    const debtReceiptKey = deriveDebtReceiptKey(keyInput)
    const patchKey = derivePatchNoveltyKey({
      behaviorReceiptDigest: 'sha256:behavior',
      debtReceiptKey,
      normalizedPatchDigest: 'patch-id:abc',
    })

    expect(debtReceiptKeyShortRef(debtReceiptKey)).toMatch(
      /^debt_receipt_key\.[a-f0-9]{16}$/,
    )
    expect(patchNoveltyKeyShortRef(patchKey)).toMatch(
      /^patch_novelty_key\.[a-f0-9]{16}$/,
    )
  })

  test('typed key schemas reject malformed values', () => {
    expect(() => DebtReceiptKey.make('debt_receipt_key:not-hex')).toThrow()
    expect(() => PatchNoveltyKey.make('patch_novelty_key:short')).toThrow()
  })
})

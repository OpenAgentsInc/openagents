import { createHash } from 'node:crypto'
import { Schema as S } from 'effect'

// Typed fingerprint keys for the hygiene debt-receipt lane (EPIC #5335,
// dup/novelty fingerprint comment). These keys make invariant #6 implementable:
//
//   DebtReceiptKey  = sha256(debtReceiptRef | repoBaselineRef | scopeDigest | objectiveDigest)
//   PatchNoveltyKey = sha256(DebtReceiptKey | normalizedPatchDigest | behaviorReceiptDigest)
//
// Settlement rule: exactly one accepted settlement per DebtReceiptKey, then it
// retires. A near-duplicate patch (a PatchNoveltyKey carrying an already retired
// DebtReceiptKey) is a duplicate replay, not payable.
//
// Derivation is synchronous (node:crypto createHash, available in the Worker via
// nodejs_compat and in Bun/vitest), so the pure synchronous settlement
// projection can derive and compare keys without a runtime.

const DebtReceiptKeyPattern = /^debt_receipt_key:[a-f0-9]{64}$/
const PatchNoveltyKeyPattern = /^patch_novelty_key:[a-f0-9]{64}$/

export const DebtReceiptKey = S.String.check(
  S.isPattern(DebtReceiptKeyPattern),
).pipe(S.brand('DebtReceiptKey'))
export type DebtReceiptKey = typeof DebtReceiptKey.Type

export const PatchNoveltyKey = S.String.check(
  S.isPattern(PatchNoveltyKeyPattern),
).pipe(S.brand('PatchNoveltyKey'))
export type PatchNoveltyKey = typeof PatchNoveltyKey.Type

export const DebtReceiptKeyInput = S.Struct({
  debtReceiptRef: S.String,
  objectiveDigest: S.String,
  repoBaselineRef: S.String,
  scopeDigest: S.String,
})
export type DebtReceiptKeyInput = typeof DebtReceiptKeyInput.Type

export const PatchNoveltyKeyInput = S.Struct({
  behaviorReceiptDigest: S.String,
  debtReceiptKey: DebtReceiptKey,
  normalizedPatchDigest: S.String,
})
export type PatchNoveltyKeyInput = typeof PatchNoveltyKeyInput.Type

export class DebtReceiptKeyUnsafe extends S.TaggedErrorClass<DebtReceiptKeyUnsafe>()(
  'DebtReceiptKeyUnsafe',
  {
    reason: S.String,
  },
) {}

const decodeDebtReceiptKeyInput = S.decodeUnknownSync(DebtReceiptKeyInput)
const decodePatchNoveltyKeyInput = S.decodeUnknownSync(PatchNoveltyKeyInput)

const requireNonEmpty = (label: string, value: string): string => {
  const trimmed = value.trim()
  if (trimmed === '') {
    throw new DebtReceiptKeyUnsafe({ reason: `${label} must not be empty.` })
  }
  return trimmed
}

const sha256Hex = (value: string): string =>
  createHash('sha256').update(value, 'utf8').digest('hex')

// `sha256(a | b | c | d)` — the `|` separator is length-prefixed so distinct
// field groupings cannot collide ("a|bc" vs "ab|c").
const joinDigest = (parts: ReadonlyArray<string>): string =>
  parts.map(part => `${part.length}:${part}`).join('|')

export const deriveDebtReceiptKey = (
  input: DebtReceiptKeyInput,
): DebtReceiptKey => {
  const decoded = decodeDebtReceiptKeyInput(input)
  const digest = sha256Hex(
    joinDigest([
      requireNonEmpty('DebtReceiptKey debtReceiptRef', decoded.debtReceiptRef),
      requireNonEmpty('DebtReceiptKey repoBaselineRef', decoded.repoBaselineRef),
      requireNonEmpty('DebtReceiptKey scopeDigest', decoded.scopeDigest),
      requireNonEmpty('DebtReceiptKey objectiveDigest', decoded.objectiveDigest),
    ]),
  )
  return DebtReceiptKey.make(`debt_receipt_key:${digest}`)
}

export const derivePatchNoveltyKey = (
  input: PatchNoveltyKeyInput,
): PatchNoveltyKey => {
  const decoded = decodePatchNoveltyKeyInput(input)
  const digest = sha256Hex(
    joinDigest([
      decoded.debtReceiptKey,
      requireNonEmpty(
        'PatchNoveltyKey normalizedPatchDigest',
        decoded.normalizedPatchDigest,
      ),
      requireNonEmpty(
        'PatchNoveltyKey behaviorReceiptDigest',
        decoded.behaviorReceiptDigest,
      ),
    ]),
  )
  return PatchNoveltyKey.make(`patch_novelty_key:${digest}`)
}

export const debtReceiptKeyShortRef = (key: DebtReceiptKey): string =>
  `debt_receipt_key.${key.replace('debt_receipt_key:', '').slice(0, 16)}`

export const patchNoveltyKeyShortRef = (key: PatchNoveltyKey): string =>
  `patch_novelty_key.${key.replace('patch_novelty_key:', '').slice(0, 16)}`

import { assertNoProviderSecretMaterial } from '@openagentsinc/provider-account-schema'

import { isoTimestampAfterIso } from './runtime-primitives'

export const PACK_C_CHANGE_CAPTURE_VERSION = 'pack-c-change-capture:v1' as const

const PACK_C_CHANGE_CAPTURE_COLLECTION = 'pack_c_change_capture_public'

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PACK_C_CHANGE_PRIVATE_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ]patch/i,
  /raw[-_ ]file/i,
  /raw[-_ ]source/i,
  /raw[-_ ]shell/i,
  /raw[-_ ]prompt/i,
  /private[-_ ]repo/i,
  /private[-_ ]content/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
]

class PackCChangeCaptureError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'PackCChangeCaptureError'
  }
}

export type PackCChangeCaptureVisibility =
  | 'customer'
  | 'operator'
  | 'public'
  | 'team'

export type PackCChangeCaptureIdentityStatus = 'blocked' | 'ready' | 'stale'

export type PackCChangeCaptureInput = Readonly<{
  authorityReceiptRefs?: ReadonlyArray<string> | undefined
  baseCommitRef: string
  changeRef: string
  diagnosticRefs?: ReadonlyArray<string> | undefined
  fileCount: number
  fileSummaryRefs?: ReadonlyArray<string> | undefined
  generatedAt: string
  headCommitRef: string
  observedAt: string
  patchDigestRef: string | null
  publicSafe: boolean
  repositoryRef: string
  reviewCaveatRefs?: ReadonlyArray<string> | undefined
  staleAfterMs: number
  summaryRef: string
  verificationRefs?: ReadonlyArray<string> | undefined
  visibility: PackCChangeCaptureVisibility
  worktreeIdentityStatus: PackCChangeCaptureIdentityStatus
  worktreeRef: string
  writebackRequired: boolean
}>

export type PackCChangeCaptureProjection = Readonly<{
  ageMs: number
  authorityReceiptRefs: ReadonlyArray<string>
  baseCommitRef: string
  blockerRefs: ReadonlyArray<string>
  changeRef: string
  changeVersion: typeof PACK_C_CHANGE_CAPTURE_VERSION
  diagnosticRefs: ReadonlyArray<string>
  fileCount: number
  fileSummaryRefs: ReadonlyArray<string>
  freshness: 'fresh' | 'stale'
  generatedAt: string
  headCommitRef: string
  observedAt: string
  patchDigestRef: string | null
  publicSafe: boolean
  repositoryRef: string
  reviewCaveatRefs: ReadonlyArray<string>
  staleAt: string
  status: 'blocked' | 'review_ready' | 'stale'
  summaryRef: string
  verificationRefs: ReadonlyArray<string>
  visibility: PackCChangeCaptureVisibility
  worktreeIdentityStatus: PackCChangeCaptureIdentityStatus
  worktreeRef: string
  writebackRequired: boolean
}>

const assertNoPrivateChangeMaterial = (
  value: unknown,
  context: string,
): void => {
  assertNoProviderSecretMaterial(value, context)

  const text = typeof value === 'string' ? value : JSON.stringify(value)

  if (PACK_C_CHANGE_PRIVATE_MARKERS.some(marker => marker.test(text))) {
    throw new PackCChangeCaptureError(
      `${context} contains raw patch, private repo, local path, or shell material.`,
    )
  }
}

const safeRef = (field: string, value: string): string => {
  const trimmed = value.trim()
  assertNoPrivateChangeMaterial(trimmed, field)

  if (!SAFE_REF_PATTERN.test(trimmed)) {
    throw new PackCChangeCaptureError(`${field} must be a stable Pack C ref.`)
  }

  return trimmed
}

const safeRefs = (
  field: string,
  values: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => (values ?? []).map(value => safeRef(field, value))

const safeOptionalRef = (field: string, value: string | null): string | null =>
  value === null ? null : safeRef(field, value)

const ageMs = (generatedAt: string, observedAt: string): number =>
  Math.max(0, Date.parse(generatedAt) - Date.parse(observedAt))

const staleAt = (observedAt: string, staleAfterMs: number): string =>
  isoTimestampAfterIso(observedAt, staleAfterMs)

const blockerRefs = (
  input: PackCChangeCaptureInput,
  refs: Readonly<{
    authorityReceiptRefs: ReadonlyArray<string>
    verificationRefs: ReadonlyArray<string>
  }>,
): ReadonlyArray<string> => [
  ...(refs.verificationRefs.length === 0
    ? [`pack-c-change-capture-blocker:${input.changeRef}:missing-verification`]
    : []),
  ...(input.patchDigestRef === null
    ? [`pack-c-change-capture-blocker:${input.changeRef}:missing-patch-digest`]
    : []),
  ...(input.writebackRequired && refs.authorityReceiptRefs.length === 0
    ? [
        `pack-c-change-capture-blocker:${input.changeRef}:missing-writeback-authority`,
      ]
    : []),
  ...(input.worktreeIdentityStatus === 'stale'
    ? [
        `pack-c-change-capture-blocker:${input.changeRef}:stale-worktree-identity`,
      ]
    : []),
  ...(input.worktreeIdentityStatus === 'blocked'
    ? [
        `pack-c-change-capture-blocker:${input.changeRef}:blocked-worktree-identity`,
      ]
    : []),
  ...(input.visibility === 'public' && !input.publicSafe
    ? [
        `pack-c-change-capture-blocker:${input.changeRef}:unsafe-public-visibility`,
      ]
    : []),
]

export const projectPackCChangeCapture = (
  input: PackCChangeCaptureInput,
): PackCChangeCaptureProjection => {
  assertNoPrivateChangeMaterial(input, 'pack-c-change-capture.input')

  const authorityReceiptRefs = safeRefs(
    'pack-c-change-capture.authorityReceiptRefs',
    input.authorityReceiptRefs,
  )
  const verificationRefs = safeRefs(
    'pack-c-change-capture.verificationRefs',
    input.verificationRefs,
  )
  const blockers = blockerRefs(input, {
    authorityReceiptRefs,
    verificationRefs,
  })
  const observedAgeMs = ageMs(input.generatedAt, input.observedAt)
  const freshness =
    Number.isFinite(observedAgeMs) && observedAgeMs <= input.staleAfterMs
      ? 'fresh'
      : 'stale'
  const projection: PackCChangeCaptureProjection = {
    ageMs: observedAgeMs,
    authorityReceiptRefs,
    baseCommitRef: safeRef(
      'pack-c-change-capture.baseCommitRef',
      input.baseCommitRef,
    ),
    blockerRefs: blockers,
    changeRef: safeRef('pack-c-change-capture.changeRef', input.changeRef),
    changeVersion: PACK_C_CHANGE_CAPTURE_VERSION,
    diagnosticRefs: safeRefs(
      'pack-c-change-capture.diagnosticRefs',
      input.diagnosticRefs,
    ),
    fileCount: Math.max(0, Math.trunc(input.fileCount)),
    fileSummaryRefs: safeRefs(
      'pack-c-change-capture.fileSummaryRefs',
      input.fileSummaryRefs,
    ),
    freshness,
    generatedAt: input.generatedAt,
    headCommitRef: safeRef(
      'pack-c-change-capture.headCommitRef',
      input.headCommitRef,
    ),
    observedAt: input.observedAt,
    patchDigestRef: safeOptionalRef(
      'pack-c-change-capture.patchDigestRef',
      input.patchDigestRef,
    ),
    publicSafe: input.publicSafe,
    repositoryRef: safeRef(
      'pack-c-change-capture.repositoryRef',
      input.repositoryRef,
    ),
    reviewCaveatRefs: safeRefs(
      'pack-c-change-capture.reviewCaveatRefs',
      input.reviewCaveatRefs,
    ),
    staleAt: staleAt(input.observedAt, input.staleAfterMs),
    status:
      blockers.length > 0
        ? 'blocked'
        : freshness === 'stale'
          ? 'stale'
          : 'review_ready',
    summaryRef: safeRef('pack-c-change-capture.summaryRef', input.summaryRef),
    verificationRefs,
    visibility: input.visibility,
    worktreeIdentityStatus: input.worktreeIdentityStatus,
    worktreeRef: safeRef(
      'pack-c-change-capture.worktreeRef',
      input.worktreeRef,
    ),
    writebackRequired: input.writebackRequired,
  }

  assertNoPrivateChangeMaterial(projection, PACK_C_CHANGE_CAPTURE_COLLECTION)

  return projection
}

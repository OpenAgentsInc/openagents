import {
  type ArtifactReviewView,
  projectArtifactReview,
} from '@openagentsinc/autopilot-control-protocol'

import type {
  AutopilotMissionBriefing,
  AutopilotWorkExecutionCloseout,
  AutopilotWorkProjection,
} from '../model'

export type ForgeDiffReviewStatus =
  | 'blocked'
  | 'pending_delivery'
  | 'review_ready'

export type ForgeDiffReviewView = Readonly<{
  addedLineCount: number | null
  artifactReview: ArtifactReviewView
  artifactRefs: ReadonlyArray<string>
  authorityReceiptRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  changeCaptureRefs: ReadonlyArray<string>
  deliveryReadinessRefs: ReadonlyArray<string>
  fileCount: number | null
  omittedUnsafeRefCount: number
  patchDigestRef: string | null
  removedLineCount: number | null
  resultRefs: ReadonlyArray<string>
  reviewCaveatRefs: ReadonlyArray<string>
  status: ForgeDiffReviewStatus
  summaryRefs: ReadonlyArray<string>
  verificationRefs: ReadonlyArray<string>
  verificationState: 'missing' | 'present'
  workOrderRef: string
}>

export type ForgeDiffArtifactDrilldownStatus = 'blocked' | 'ready' | 'stale'

export type ForgeDiffArtifactDrilldownFileGroup = Readonly<{
  artifactRefs: ReadonlyArray<string>
  fileRefs: ReadonlyArray<string>
  groupRef: string
  hunkSummaryRefs: ReadonlyArray<string>
  summaryRefs: ReadonlyArray<string>
}>

export type ForgeDiffArtifactDrilldownAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  deployAuthority: false
  rawPatchAuthority: false
  settlementAuthority: false
  writebackAuthority: false
}>

export type ForgeDiffArtifactDrilldownView = Readonly<{
  artifactRefs: ReadonlyArray<string>
  authority: ForgeDiffArtifactDrilldownAuthority
  blockerRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  changeCaptureRefs: ReadonlyArray<string>
  deliveryReadinessRefs: ReadonlyArray<string>
  fileGroups: ReadonlyArray<ForgeDiffArtifactDrilldownFileGroup>
  generatedAt: string
  hunkSummaryRefs: ReadonlyArray<string>
  omittedUnsafeRefCount: number
  patchDigestRef: string | null
  publicSafe: true
  status: ForgeDiffArtifactDrilldownStatus
  verificationRefs: ReadonlyArray<string>
  workOrderRef: string
}>

type RefBundle = Readonly<{
  omittedUnsafeRefCount: number
  refs: ReadonlyArray<string>
}>

type OptionalRefBundle = Readonly<{
  omittedUnsafeRefCount: number
  ref: string | null
}>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_REVIEW_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:patch|file|source|shell|command|prompt|log)/i,
  /private[-_ ](?:repo|content|source)/i,
  /provider[-_ ]payload/i,
  /wallet|payment[-_ ](?:material|preimage|hash)/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:bearer|token|secret|mnemonic|preimage|invoice)\b/i,
]

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_REVIEW_MARKERS.some(marker => marker.test(trimmed))
    ? trimmed
    : null
}

const safeRefs = (
  ...groups: ReadonlyArray<ReadonlyArray<string> | undefined>
): RefBundle => {
  const refs = groups.flatMap(group => group ?? [])
  const sanitized = refs.reduce<Readonly<{ omitted: number; refs: string[] }>>(
    (state, ref) => {
      const safe = safeRef(ref)

      return safe === null
        ? { omitted: state.omitted + 1, refs: state.refs }
        : { omitted: state.omitted, refs: [...state.refs, safe] }
    },
    { omitted: 0, refs: [] },
  )

  return {
    omittedUnsafeRefCount: sanitized.omitted,
    refs: Array.from(new Set(sanitized.refs)),
  }
}

const safeOptionalRef = (
  value: string | null | undefined,
): OptionalRefBundle => {
  if (value === null || value === undefined) {
    return { omittedUnsafeRefCount: 0, ref: null }
  }

  const ref = safeRef(value)

  return ref === null
    ? { omittedUnsafeRefCount: 1, ref: null }
    : { omittedUnsafeRefCount: 0, ref }
}

const nonNegativeInteger = (value: number | undefined): number | null =>
  value === undefined || !Number.isSafeInteger(value) || value < 0
    ? null
    : value

const missingBlocker = (workOrderRef: string, suffix: string): string =>
  `forge-diff-review-blocker:${workOrderRef}:${suffix}`

const drilldownBlocker = (workOrderRef: string, suffix: string): string =>
  `forge-diff-artifact-drilldown-blocker:${workOrderRef}:${suffix}`

const changeCaptureBlockers = (
  workOrderRef: string,
  closeout: AutopilotWorkExecutionCloseout,
): ReadonlyArray<string> => [
  ...(closeout.changeCaptureStatus === 'stale'
    ? [missingBlocker(workOrderRef, 'stale-change-capture')]
    : []),
  ...(closeout.changeCaptureStatus === 'blocked'
    ? [missingBlocker(workOrderRef, 'blocked-change-capture')]
    : []),
  ...(closeout.worktreeIdentityStatus === 'stale'
    ? [missingBlocker(workOrderRef, 'stale-worktree-identity')]
    : []),
  ...(closeout.worktreeIdentityStatus === 'blocked'
    ? [missingBlocker(workOrderRef, 'blocked-worktree-identity')]
    : []),
  ...(closeout.deliveryReadinessFreshness === 'stale'
    ? [missingBlocker(workOrderRef, 'stale-delivery-readiness')]
    : []),
  ...(closeout.deliveryReadinessStatus === 'blocked'
    ? [missingBlocker(workOrderRef, 'blocked-delivery-readiness')]
    : []),
]

const closeoutArtifactReview = (
  work: AutopilotWorkProjection,
  closeout: AutopilotWorkExecutionCloseout,
  input: Readonly<{
    artifactRefs: ReadonlyArray<string>
    fileCount: number | null
    verificationState: ForgeDiffReviewView['verificationState']
  }>,
): ArtifactReviewView =>
  projectArtifactReview({
    ...closeout,
    artifact: {
      artifactRef: input.artifactRefs[0] ?? null,
    },
    review: work.reviewDecision ?? {},
    status: work.reviewDecision?.action ?? work.state,
    summary: {
      changedFileCount: input.fileCount,
      devCheckState:
        input.verificationState === 'present'
          ? 'verification_refs_present'
          : 'verification_refs_missing',
    },
  })

export const projectForgeDiffReview = (
  work: AutopilotWorkProjection,
  briefing: AutopilotMissionBriefing | null,
): ForgeDiffReviewView => {
  const closeout = work.executionCloseout

  if (closeout === null) {
    return {
      addedLineCount: null,
      artifactReview: projectArtifactReview({ status: work.state }),
      artifactRefs: [],
      authorityReceiptRefs: [],
      blockerRefs: [missingBlocker(work.workOrderRef, 'missing-delivery')],
      changeCaptureRefs: [],
      deliveryReadinessRefs: [],
      fileCount: null,
      omittedUnsafeRefCount: 0,
      patchDigestRef: null,
      removedLineCount: null,
      resultRefs: [],
      reviewCaveatRefs: [],
      status: 'pending_delivery',
      summaryRefs: [],
      verificationRefs: [],
      verificationState: 'missing',
      workOrderRef: work.workOrderRef,
    }
  }

  const artifactRefs = safeRefs(
    closeout.artifactRefs,
    briefing?.whatChanged.artifactRefs,
  )
  const authorityReceiptRefs = safeRefs(closeout.authorityReceiptRefs)
  const blockerRefs = safeRefs(
    closeout.blockerRefs,
    briefing?.whatIsBlocked.blockerRefs,
  )
  const changeCaptureRefs = safeRefs(closeout.changeCaptureRefs)
  const deliveryReadinessRefs = safeRefs(closeout.deliveryReadinessRefs)
  const resultRefs = safeRefs(
    closeout.resultRefs,
    briefing?.whatChanged.resultRefs,
  )
  const reviewCaveatRefs = safeRefs(closeout.reviewCaveatRefs)
  const summaryRefs = safeRefs(
    closeout.summaryRefs,
    briefing?.whatChanged.summaryRefs,
  )
  const verificationRefs = safeRefs(
    closeout.verificationRefs,
    closeout.testRefs,
    closeout.proofRefs,
  )
  const patchDigestRef = safeOptionalRef(closeout.patchDigestRef)
  const fileCount =
    nonNegativeInteger(closeout.fileCount) ?? artifactRefs.refs.length
  const addedLineCount = nonNegativeInteger(closeout.addedLineCount)
  const removedLineCount = nonNegativeInteger(closeout.removedLineCount)
  const verificationState =
    verificationRefs.refs.length === 0 ? 'missing' : 'present'
  const omittedUnsafeRefCount =
    artifactRefs.omittedUnsafeRefCount +
    authorityReceiptRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    changeCaptureRefs.omittedUnsafeRefCount +
    deliveryReadinessRefs.omittedUnsafeRefCount +
    resultRefs.omittedUnsafeRefCount +
    reviewCaveatRefs.omittedUnsafeRefCount +
    summaryRefs.omittedUnsafeRefCount +
    verificationRefs.omittedUnsafeRefCount +
    patchDigestRef.omittedUnsafeRefCount
  const derivedBlockers = [
    ...(changeCaptureRefs.refs.length === 0
      ? [missingBlocker(work.workOrderRef, 'missing-change-capture')]
      : []),
    ...(deliveryReadinessRefs.refs.length === 0
      ? [missingBlocker(work.workOrderRef, 'missing-delivery-readiness')]
      : []),
    ...(patchDigestRef.ref === null
      ? [missingBlocker(work.workOrderRef, 'missing-patch-digest')]
      : []),
    ...(verificationState === 'missing'
      ? [missingBlocker(work.workOrderRef, 'missing-verification')]
      : []),
    ...(closeout.writebackRequired !== false &&
    authorityReceiptRefs.refs.length === 0
      ? [missingBlocker(work.workOrderRef, 'missing-writeback-authority')]
      : []),
    ...(!closeout.publicSafe
      ? [missingBlocker(work.workOrderRef, 'unsafe-public-visibility')]
      : []),
    ...(omittedUnsafeRefCount > 0
      ? [missingBlocker(work.workOrderRef, 'unsafe-review-material-omitted')]
      : []),
    ...changeCaptureBlockers(work.workOrderRef, closeout),
  ]
  const allBlockers = safeRefs(blockerRefs.refs, derivedBlockers)
  const artifactReview = closeoutArtifactReview(work, closeout, {
    artifactRefs: artifactRefs.refs,
    fileCount,
    verificationState,
  })

  return {
    addedLineCount,
    artifactReview,
    artifactRefs: artifactRefs.refs,
    authorityReceiptRefs: authorityReceiptRefs.refs,
    blockerRefs: allBlockers.refs,
    changeCaptureRefs: changeCaptureRefs.refs,
    deliveryReadinessRefs: deliveryReadinessRefs.refs,
    fileCount,
    omittedUnsafeRefCount,
    patchDigestRef: patchDigestRef.ref,
    removedLineCount,
    resultRefs: resultRefs.refs,
    reviewCaveatRefs: reviewCaveatRefs.refs,
    status: allBlockers.refs.length === 0 ? 'review_ready' : 'blocked',
    summaryRefs: summaryRefs.refs,
    verificationRefs: verificationRefs.refs,
    verificationState,
    workOrderRef: work.workOrderRef,
  }
}

const DIFF_FILE_GROUP_KINDS: ReadonlySet<string> = new Set([
  'changed_file',
  'changed_files',
  'diff_file',
  'diff_files',
  'file_group',
])
const DIFF_HUNK_GROUP_KINDS: ReadonlySet<string> = new Set([
  'diff_hunk',
  'diff_hunks',
  'hunk_summary',
  'hunk_summaries',
])
const DIFF_SUMMARY_GROUP_KINDS: ReadonlySet<string> = new Set([
  'diff_artifact',
  'diff_summary',
  'review_summary',
])

const briefingDrilldownRefs = (
  briefing: AutopilotMissionBriefing | null,
  kinds: ReadonlySet<string>,
): RefBundle =>
  safeRefs(
    ...(briefing?.drilldown ?? [])
      .filter(group => kinds.has(group.kind))
      .map(group => group.refs),
  )

const fallbackFileGroup = (
  workOrderRef: string,
  review: ForgeDiffReviewView,
  summaryRefs: ReadonlyArray<string>,
  hunkSummaryRefs: ReadonlyArray<string>,
): ForgeDiffArtifactDrilldownFileGroup =>
  ({
    artifactRefs: review.artifactRefs,
    fileRefs: [],
    groupRef: `forge-diff-artifact-drilldown:${workOrderRef}:artifact-summary`,
    hunkSummaryRefs,
    summaryRefs,
  })

const fileGroups = (
  workOrderRef: string,
  review: ForgeDiffReviewView,
  fileRefs: ReadonlyArray<string>,
  summaryRefs: ReadonlyArray<string>,
  hunkSummaryRefs: ReadonlyArray<string>,
): ReadonlyArray<ForgeDiffArtifactDrilldownFileGroup> => {
  if (fileRefs.length === 0) {
    return review.artifactRefs.length === 0
      ? []
      : [fallbackFileGroup(workOrderRef, review, summaryRefs, hunkSummaryRefs)]
  }

  return fileRefs.map((fileRef, index) => ({
    artifactRefs: review.artifactRefs,
    fileRefs: [fileRef],
    groupRef: `forge-diff-artifact-drilldown:${workOrderRef}:file-${index + 1}`,
    hunkSummaryRefs,
    summaryRefs,
  }))
}

const drilldownStatus = (
  blockerRefs: ReadonlyArray<string>,
): ForgeDiffArtifactDrilldownStatus => {
  if (blockerRefs.length === 0) {
    return 'ready'
  }

  return blockerRefs.every(ref => ref.includes(':stale-'))
    ? 'stale'
    : 'blocked'
}

export const projectForgeDiffArtifactDrilldown = (
  work: AutopilotWorkProjection,
  briefing: AutopilotMissionBriefing | null,
): ForgeDiffArtifactDrilldownView => {
  const review = projectForgeDiffReview(work, briefing)
  const fileRefs = briefingDrilldownRefs(briefing, DIFF_FILE_GROUP_KINDS)
  const hunkSummaryRefs = briefingDrilldownRefs(briefing, DIFF_HUNK_GROUP_KINDS)
  const drilldownSummaryRefs = briefingDrilldownRefs(
    briefing,
    DIFF_SUMMARY_GROUP_KINDS,
  )
  const summaryRefs = safeRefs(review.summaryRefs, drilldownSummaryRefs.refs)
  const omittedUnsafeRefCount =
    review.omittedUnsafeRefCount +
    fileRefs.omittedUnsafeRefCount +
    hunkSummaryRefs.omittedUnsafeRefCount +
    drilldownSummaryRefs.omittedUnsafeRefCount +
    summaryRefs.omittedUnsafeRefCount
  const derivedBlockers = [
    ...(review.artifactRefs.length === 0
      ? [drilldownBlocker(work.workOrderRef, 'missing-artifact-ref')]
      : []),
    ...(review.patchDigestRef === null
      ? [drilldownBlocker(work.workOrderRef, 'missing-patch-digest')]
      : []),
    ...(review.verificationRefs.length === 0
      ? [drilldownBlocker(work.workOrderRef, 'missing-verification-ref')]
      : []),
    ...(omittedUnsafeRefCount === 0
      ? []
      : [drilldownBlocker(work.workOrderRef, 'unsafe-artifact-material-omitted')]),
  ]
  const blockerRefs = safeRefs(review.blockerRefs, derivedBlockers)
  const groups = fileGroups(
    work.workOrderRef,
    review,
    fileRefs.refs,
    summaryRefs.refs,
    hunkSummaryRefs.refs,
  )

  return {
    artifactRefs: review.artifactRefs,
    authority: {
      acceptedOutcomeAuthority: false,
      deployAuthority: false,
      rawPatchAuthority: false,
      settlementAuthority: false,
      writebackAuthority: false,
    },
    blockerRefs: blockerRefs.refs,
    caveatRefs: review.reviewCaveatRefs,
    changeCaptureRefs: review.changeCaptureRefs,
    deliveryReadinessRefs: review.deliveryReadinessRefs,
    fileGroups: groups,
    generatedAt: work.generatedAt,
    hunkSummaryRefs: hunkSummaryRefs.refs,
    omittedUnsafeRefCount,
    patchDigestRef: review.patchDigestRef,
    publicSafe: true,
    status: drilldownStatus(blockerRefs.refs),
    verificationRefs: review.verificationRefs,
    workOrderRef: work.workOrderRef,
  }
}

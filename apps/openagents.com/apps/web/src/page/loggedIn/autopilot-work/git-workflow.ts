import type {
  AutopilotWorkGitWorkflow,
  AutopilotWorkGitWorkflowEntry,
  AutopilotWorkGitWorkflowFreshness,
  AutopilotWorkGitWorkflowState,
  AutopilotWorkProjection,
} from '../model'

export type ForgeGitWorkflowStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'waiting'
  | 'warning'
  | 'unknown'

export type ForgeGitWorkflowAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  branchCreateAuthority: false
  checkRunAuthority: false
  commitAuthority: false
  deploymentAuthority: false
  fileReadAuthority: false
  gitExecutionAuthority: false
  githubWriteAuthority: false
  issueCommentAuthority: false
  prCreateAuthority: false
  publicClaimAuthority: false
  reviewSubmitAuthority: false
  settlementAuthority: false
  shellExecutionAuthority: false
  tagCreateAuthority: false
  toolGrantAuthority: false
  workerPayoutAuthority: false
  writebackAuthority: false
}>

export type ForgeGitWorkflowItem = Readonly<{
  blockerRefs: ReadonlyArray<string>
  branchRefs: ReadonlyArray<string>
  checkRefs: ReadonlyArray<string>
  commitRefs: ReadonlyArray<string>
  diffRefs: ReadonlyArray<string>
  freshness: AutopilotWorkGitWorkflowFreshness
  issueRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  prRefs: ReadonlyArray<string>
  repositoryRefs: ReadonlyArray<string>
  reviewRefs: ReadonlyArray<string>
  state: AutopilotWorkGitWorkflowState
  statusRefs: ReadonlyArray<string>
  workflowRef: string
  worktreeRefs: ReadonlyArray<string>
  writebackRefs: ReadonlyArray<string>
}>

export type ForgeGitWorkflowInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkGitWorkflowEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeGitWorkflowCounts = Readonly<{
  blocked: number
  checksPending: number
  prReady: number
  reviewReady: number
  total: number
  writebackReady: number
}>

export type ForgeGitWorkflowView = Readonly<{
  authority: ForgeGitWorkflowAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeGitWorkflowCounts
  entries: ReadonlyArray<ForgeGitWorkflowItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeGitWorkflowStatus
  versionRef: string | null
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
const PRIVATE_GIT_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:body|branch|check|command|commit|content|diff|file|git|github|issue|log|output|patch|pr|prompt|review|shell|source|status|stderr|stdout|trace|transcript|worktree)/i,
  /private[-_ ](?:branch|commit|content|diff|file|git|github|issue|patch|pr|repo|review|source|worktree|workspace)/i,
  /git[-_ ](?:command|diff|log|output|patch|status|stdout|stderr)/i,
  /github[-_ ](?:body|comment|payload|token)/i,
  /provider[-_ ]payload/i,
  /provider[-_ ]prompt/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const authority: ForgeGitWorkflowAuthority = {
  acceptedOutcomeAuthority: false,
  branchCreateAuthority: false,
  checkRunAuthority: false,
  commitAuthority: false,
  deploymentAuthority: false,
  fileReadAuthority: false,
  gitExecutionAuthority: false,
  githubWriteAuthority: false,
  issueCommentAuthority: false,
  prCreateAuthority: false,
  publicClaimAuthority: false,
  reviewSubmitAuthority: false,
  settlementAuthority: false,
  shellExecutionAuthority: false,
  tagCreateAuthority: false,
  toolGrantAuthority: false,
  workerPayoutAuthority: false,
  writebackAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_GIT_MARKERS.some(marker => marker.test(trimmed))
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

const blockerRef = (workOrderRef: string, suffix: string): string =>
  `forge-git-workflow-blocker:${workOrderRef}:${suffix}`

const normalizeEntry = (
  entry: AutopilotWorkGitWorkflowEntry,
): Readonly<{
  entry: ForgeGitWorkflowItem | null
  omittedUnsafeRefCount: number
}> => {
  const workflowRef = safeOptionalRef(entry.workflowRef)
  const blockerRefs = safeRefs(entry.blockerRefs)
  const branchRefs = safeRefs(entry.branchRefs)
  const checkRefs = safeRefs(entry.checkRefs)
  const commitRefs = safeRefs(entry.commitRefs)
  const diffRefs = safeRefs(entry.diffRefs)
  const issueRefs = safeRefs(entry.issueRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const prRefs = safeRefs(entry.prRefs)
  const repositoryRefs = safeRefs(entry.repositoryRefs)
  const reviewRefs = safeRefs(entry.reviewRefs)
  const statusRefs = safeRefs(entry.statusRefs)
  const worktreeRefs = safeRefs(entry.worktreeRefs)
  const writebackRefs = safeRefs(entry.writebackRefs)
  const omittedUnsafeRefCount =
    workflowRef.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    branchRefs.omittedUnsafeRefCount +
    checkRefs.omittedUnsafeRefCount +
    commitRefs.omittedUnsafeRefCount +
    diffRefs.omittedUnsafeRefCount +
    issueRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    prRefs.omittedUnsafeRefCount +
    repositoryRefs.omittedUnsafeRefCount +
    reviewRefs.omittedUnsafeRefCount +
    statusRefs.omittedUnsafeRefCount +
    worktreeRefs.omittedUnsafeRefCount +
    writebackRefs.omittedUnsafeRefCount

  return workflowRef.ref === null
    ? { entry: null, omittedUnsafeRefCount }
    : {
        entry: {
          blockerRefs: blockerRefs.refs,
          branchRefs: branchRefs.refs,
          checkRefs: checkRefs.refs,
          commitRefs: commitRefs.refs,
          diffRefs: diffRefs.refs,
          freshness: entry.freshness ?? 'unknown',
          issueRefs: issueRefs.refs,
          policyRefs: policyRefs.refs,
          prRefs: prRefs.refs,
          repositoryRefs: repositoryRefs.refs,
          reviewRefs: reviewRefs.refs,
          state: entry.state,
          statusRefs: statusRefs.refs,
          workflowRef: workflowRef.ref,
          worktreeRefs: worktreeRefs.refs,
          writebackRefs: writebackRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeGitWorkflowItem>,
): ForgeGitWorkflowCounts => ({
  blocked: entries.filter(entry => entry.state === 'blocked').length,
  checksPending: entries.filter(entry => entry.state === 'checks_pending').length,
  prReady: entries.filter(entry => entry.state === 'pr_ready').length,
  reviewReady: entries.filter(entry => entry.state === 'review_ready').length,
  total: entries.length,
  writebackReady: entries.filter(entry => entry.state === 'writeback_ready').length,
})

const staleBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeGitWorkflowItem>,
): ReadonlyArray<string> =>
  entries
    .filter(entry => entry.freshness === 'stale' && entry.blockerRefs.length === 0)
    .map(entry => blockerRef(workOrderRef, `stale-git-workflow-evidence:${entry.workflowRef}`))

const prReadyBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeGitWorkflowItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.state === 'pr_ready' &&
        entry.blockerRefs.length === 0 &&
        (entry.branchRefs.length === 0 ||
          entry.diffRefs.length === 0 ||
          entry.checkRefs.length === 0),
    )
    .map(entry => blockerRef(workOrderRef, `pr-ready-evidence-missing:${entry.workflowRef}`))

const reviewReadyBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeGitWorkflowItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.state === 'review_ready' &&
        entry.blockerRefs.length === 0 &&
        (entry.reviewRefs.length === 0 || entry.policyRefs.length === 0),
    )
    .map(entry => blockerRef(workOrderRef, `review-ready-evidence-missing:${entry.workflowRef}`))

const writebackReadyBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeGitWorkflowItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.state === 'writeback_ready' &&
        entry.blockerRefs.length === 0 &&
        (entry.writebackRefs.length === 0 || entry.policyRefs.length === 0),
    )
    .map(entry =>
      blockerRef(workOrderRef, `writeback-ready-evidence-missing:${entry.workflowRef}`),
    )

const statusForView = (
  entries: ReadonlyArray<ForgeGitWorkflowItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeGitWorkflowStatus => {
  if (blockerRefs.length > 0 || entries.some(entry => entry.state === 'blocked')) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.freshness === 'stale')) {
    return 'stale'
  }

  if (entries.some(entry => entry.state === 'checks_pending')) {
    return 'waiting'
  }

  if (
    entries.every(
      entry =>
        entry.state === 'pr_ready' ||
        entry.state === 'review_ready' ||
        entry.state === 'writeback_ready',
    )
  ) {
    return 'ready'
  }

  return entries.some(entry => entry.state === 'unknown') ? 'unknown' : 'warning'
}

export const projectForgeGitWorkflow = (
  input: ForgeGitWorkflowInput,
): ForgeGitWorkflowView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedEntries = (input.entries ?? []).map(normalizeEntry)
  const entries = normalizedEntries
    .flatMap(result => (result.entry === null ? [] : [result.entry]))
    .sort(
      (left, right) =>
        left.state.localeCompare(right.state) ||
        left.freshness.localeCompare(right.freshness) ||
        left.workflowRef.localeCompare(right.workflowRef),
    )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    normalizedEntries.reduce((sum, result) => sum + result.omittedUnsafeRefCount, 0)
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...entries.flatMap(entry => entry.blockerRefs),
      ...staleBlockers(input.workOrderRef, entries),
      ...prReadyBlockers(input.workOrderRef, entries),
      ...reviewReadyBlockers(input.workOrderRef, entries),
      ...writebackReadyBlockers(input.workOrderRef, entries),
      ...(input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-git-workflow-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-git-workflow-material-omitted')]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    counts: counts(entries),
    entries,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    snapshotRef: snapshotRef.ref,
    status: statusForView(entries, blockerRefs),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeGitWorkflowInput = (
  work: AutopilotWorkProjection,
): ForgeGitWorkflowInput => {
  const source: AutopilotWorkGitWorkflow | undefined = work.gitWorkflow

  if (source === undefined) {
    return {
      generatedAt: work.generatedAt,
      workOrderRef: work.workOrderRef,
    }
  }

  return {
    generatedAt: source.generatedAt ?? work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(source.blockerRefs === undefined ? {} : { blockerRefs: source.blockerRefs }),
    ...(source.entries === undefined ? {} : { entries: source.entries }),
    snapshotRef: source.snapshotRef,
    ...(source.versionRef === undefined ? {} : { versionRef: source.versionRef }),
  }
}

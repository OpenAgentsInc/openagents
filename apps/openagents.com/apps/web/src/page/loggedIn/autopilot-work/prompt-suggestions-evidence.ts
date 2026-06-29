import type {
  AutopilotWorkProjection,
  AutopilotWorkPromptSuggestionEntry,
  AutopilotWorkPromptSuggestionFreshness,
  AutopilotWorkPromptSuggestionKind,
  AutopilotWorkPromptSuggestionPrivacy,
  AutopilotWorkPromptSuggestionStatus,
} from '../model'

export type ForgePromptSuggestionsStatus =
  | 'blocked'
  | 'disabled'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'unknown'

export type ForgePromptSuggestionsAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  actionExecutionAuthority: false
  autocompleteStreamAuthority: false
  commandExecutionAuthority: false
  externalActionTriggerAuthority: false
  permissionGrantAuthority: false
  privateArtifactReadAuthority: false
  privateFileReadAuthority: false
  promptInsertionAuthority: false
  promptSubmissionAuthority: false
  rankingExecutionAuthority: false
  semanticRoutingDecisionAuthority: false
  settingsMutationAuthority: false
  settlementAuthority: false
  suggestionIndexingAuthority: false
  toolInvocationAuthority: false
  workerPayoutAuthority: false
}>

export type ForgePromptSuggestionItem = Readonly<{
  actionRef: string | null
  actionSeparationRefs: ReadonlyArray<string>
  auditRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  confidenceRefs: ReadonlyArray<string>
  destructiveActionRefs: ReadonlyArray<string>
  disablementRefs: ReadonlyArray<string>
  displayRefs: ReadonlyArray<string>
  expirationRefs: ReadonlyArray<string>
  externalActionRefs: ReadonlyArray<string>
  freshness: AutopilotWorkPromptSuggestionFreshness
  insertTextRefs: ReadonlyArray<string>
  kind: AutopilotWorkPromptSuggestionKind
  permissionRefs: ReadonlyArray<string>
  privacy: AutopilotWorkPromptSuggestionPrivacy
  privacyRefs: ReadonlyArray<string>
  provenanceRefs: ReadonlyArray<string>
  rankingRefs: ReadonlyArray<string>
  scopeRefs: ReadonlyArray<string>
  semanticSelectorRefs: ReadonlyArray<string>
  status: AutopilotWorkPromptSuggestionStatus
  suggestionRef: string
  validationRefs: ReadonlyArray<string>
}>

export type ForgePromptSuggestionsInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkPromptSuggestionEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgePromptSuggestionsCounts = Readonly<{
  actions: number
  disabled: number
  ready: number
  scoped: number
  semantic: number
  stale: number
  suggestions: number
}>

export type ForgePromptSuggestionsView = Readonly<{
  authority: ForgePromptSuggestionsAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgePromptSuggestionsCounts
  entries: ReadonlyArray<ForgePromptSuggestionItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgePromptSuggestionsStatus
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
const PRIVATE_SUGGESTION_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:artifact|command|content|file|inserted|model|output|payload|prompt|repo|suggestion|text|token)/i,
  /private[-_ ](?:artifact|content|data|file|payload|prompt|repo|scope|suggestion|workspace)/i,
  /artifact[-_ ]content/i,
  /destructive[-_ ]command[-_ ]text/i,
  /inserted[-_ ]text[-_ ]body/i,
  /model[-_ ]output/i,
  /prompt[-_ ]text/i,
  /repo(?:sitory)?[-_ ](?:data|payload|private)/i,
  /suggestion[-_ ]payload/i,
  /unvalidated[-_ ]model/i,
  /customer[-_ ](?:data|private|payload|record)/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const SCOPED_KINDS: ReadonlySet<AutopilotWorkPromptSuggestionKind> = new Set([
  'artifact',
  'file',
  'session',
  'symbol',
])

const SEMANTIC_KINDS: ReadonlySet<AutopilotWorkPromptSuggestionKind> = new Set([
  'follow_up_action',
  'prompt_starter',
  'workflow',
])

const authority: ForgePromptSuggestionsAuthority = {
  acceptedOutcomeAuthority: false,
  actionExecutionAuthority: false,
  autocompleteStreamAuthority: false,
  commandExecutionAuthority: false,
  externalActionTriggerAuthority: false,
  permissionGrantAuthority: false,
  privateArtifactReadAuthority: false,
  privateFileReadAuthority: false,
  promptInsertionAuthority: false,
  promptSubmissionAuthority: false,
  rankingExecutionAuthority: false,
  semanticRoutingDecisionAuthority: false,
  settingsMutationAuthority: false,
  settlementAuthority: false,
  suggestionIndexingAuthority: false,
  toolInvocationAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_SUGGESTION_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-prompt-suggestions-blocker:${workOrderRef}:${suffix}`

const keywordOnlyEvidence = (refs: ReadonlyArray<string>): boolean =>
  refs.some(ref => /keyword[-_]?only|ad[-_]?hoc[-_]?keyword/i.test(ref))

const normalizeItem = (
  item: AutopilotWorkPromptSuggestionEntry,
): Readonly<{
  item: ForgePromptSuggestionItem | null
  omittedUnsafeRefCount: number
}> => {
  const actionRef = safeOptionalRef(item.actionRef)
  const actionSeparationRefs = safeRefs(item.actionSeparationRefs)
  const auditRefs = safeRefs(item.auditRefs)
  const blockerRefs = safeRefs(item.blockerRefs)
  const confidenceRefs = safeRefs(item.confidenceRefs)
  const destructiveActionRefs = safeRefs(item.destructiveActionRefs)
  const disablementRefs = safeRefs(item.disablementRefs)
  const displayRefs = safeRefs(item.displayRefs)
  const expirationRefs = safeRefs(item.expirationRefs)
  const externalActionRefs = safeRefs(item.externalActionRefs)
  const insertTextRefs = safeRefs(item.insertTextRefs)
  const permissionRefs = safeRefs(item.permissionRefs)
  const privacyRefs = safeRefs(item.privacyRefs)
  const provenanceRefs = safeRefs(item.provenanceRefs)
  const rankingRefs = safeRefs(item.rankingRefs)
  const scopeRefs = safeRefs(item.scopeRefs)
  const semanticSelectorRefs = safeRefs(item.semanticSelectorRefs)
  const suggestionRef = safeOptionalRef(item.suggestionRef)
  const validationRefs = safeRefs(item.validationRefs)
  const omittedUnsafeRefCount =
    actionRef.omittedUnsafeRefCount +
    actionSeparationRefs.omittedUnsafeRefCount +
    auditRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    confidenceRefs.omittedUnsafeRefCount +
    destructiveActionRefs.omittedUnsafeRefCount +
    disablementRefs.omittedUnsafeRefCount +
    displayRefs.omittedUnsafeRefCount +
    expirationRefs.omittedUnsafeRefCount +
    externalActionRefs.omittedUnsafeRefCount +
    insertTextRefs.omittedUnsafeRefCount +
    permissionRefs.omittedUnsafeRefCount +
    privacyRefs.omittedUnsafeRefCount +
    provenanceRefs.omittedUnsafeRefCount +
    rankingRefs.omittedUnsafeRefCount +
    scopeRefs.omittedUnsafeRefCount +
    semanticSelectorRefs.omittedUnsafeRefCount +
    suggestionRef.omittedUnsafeRefCount +
    validationRefs.omittedUnsafeRefCount

  return suggestionRef.ref === null
    ? { item: null, omittedUnsafeRefCount }
    : {
        item: {
          actionRef: actionRef.ref,
          actionSeparationRefs: actionSeparationRefs.refs,
          auditRefs: auditRefs.refs,
          blockerRefs: blockerRefs.refs,
          confidenceRefs: confidenceRefs.refs,
          destructiveActionRefs: destructiveActionRefs.refs,
          disablementRefs: disablementRefs.refs,
          displayRefs: displayRefs.refs,
          expirationRefs: expirationRefs.refs,
          externalActionRefs: externalActionRefs.refs,
          freshness: item.freshness ?? 'unknown',
          insertTextRefs: insertTextRefs.refs,
          kind: item.kind,
          permissionRefs: permissionRefs.refs,
          privacy: item.privacy,
          privacyRefs: privacyRefs.refs,
          provenanceRefs: provenanceRefs.refs,
          rankingRefs: rankingRefs.refs,
          scopeRefs: scopeRefs.refs,
          semanticSelectorRefs: semanticSelectorRefs.refs,
          status: item.status,
          suggestionRef: suggestionRef.ref,
          validationRefs: validationRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgePromptSuggestionItem>,
): ForgePromptSuggestionsCounts => ({
  actions: entries.filter(entry => entry.actionRef !== null).length,
  disabled: entries.filter(entry => entry.status === 'disabled').length,
  ready: entries.filter(entry => entry.status === 'ready').length,
  scoped: entries.filter(entry => SCOPED_KINDS.has(entry.kind)).length,
  semantic: entries.filter(entry => SEMANTIC_KINDS.has(entry.kind)).length,
  stale: entries.filter(
    entry =>
      entry.status === 'stale' ||
      entry.status === 'expired' ||
      entry.freshness === 'stale' ||
      entry.freshness === 'expired',
  ).length,
  suggestions: entries.length,
})

const itemBlockers = (
  workOrderRef: string,
  item: ForgePromptSuggestionItem,
): ReadonlyArray<string> => {
  const blockers = [...item.blockerRefs]

  if (
    item.status === 'stale' ||
    item.status === 'expired' ||
    item.freshness === 'stale' ||
    item.freshness === 'expired'
  ) {
    blockers.push(
      blockerRef(workOrderRef, `stale-or-expired-suggestion:${item.suggestionRef}`),
    )
  }

  if (
    item.actionRef !== null &&
    item.insertTextRefs.length > 0 &&
    item.actionSeparationRefs.length === 0
  ) {
    blockers.push(
      blockerRef(workOrderRef, `action-insert-separation-missing:${item.suggestionRef}`),
    )
  }

  if (
    item.status === 'ready' &&
    SCOPED_KINDS.has(item.kind) &&
    (item.scopeRefs.length === 0 || item.privacyRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(workOrderRef, `scoped-suggestion-privacy-missing:${item.suggestionRef}`),
    )
  }

  if (
    item.status === 'ready' &&
    (item.destructiveActionRefs.length > 0 || item.externalActionRefs.length > 0) &&
    item.permissionRefs.length === 0
  ) {
    blockers.push(
      blockerRef(workOrderRef, `destructive-external-permission-missing:${item.suggestionRef}`),
    )
  }

  if (
    item.status === 'ready' &&
    SEMANTIC_KINDS.has(item.kind) &&
    (item.semanticSelectorRefs.length === 0 ||
      item.rankingRefs.length === 0 ||
      keywordOnlyEvidence(item.rankingRefs))
  ) {
    blockers.push(
      blockerRef(workOrderRef, `semantic-suggestion-ranking-evidence-missing:${item.suggestionRef}`),
    )
  }

  return blockers
}

const statusFrom = (
  entries: ReadonlyArray<ForgePromptSuggestionItem>,
  blockers: ReadonlyArray<string>,
): ForgePromptSuggestionsStatus => {
  if (blockers.length > 0 || entries.some(entry => entry.status === 'blocked')) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.every(entry => entry.status === 'disabled')) {
    return 'disabled'
  }

  if (
    entries.some(
      entry =>
        entry.status === 'stale' ||
        entry.status === 'expired' ||
        entry.freshness === 'stale' ||
        entry.freshness === 'expired',
    )
  ) {
    return 'stale'
  }

  if (entries.every(entry => entry.status === 'ready')) {
    return 'ready'
  }

  return 'unknown'
}

export const projectForgePromptSuggestionsEvidence = (
  input: ForgePromptSuggestionsInput,
): ForgePromptSuggestionsView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const inputBlockerRefs = safeRefs(input.blockerRefs)
  const normalized = (input.entries ?? []).map(normalizeItem)
  const entries = normalized.flatMap(result =>
    result.item === null ? [] : [result.item],
  )
  const normalizedOmissions = normalized.reduce(
    (total, result) => total + result.omittedUnsafeRefCount,
    0,
  )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    inputBlockerRefs.omittedUnsafeRefCount +
    normalizedOmissions
  const blockers = [
    ...inputBlockerRefs.refs,
    ...entries.flatMap(entry => itemBlockers(input.workOrderRef, entry)),
  ]

  if (input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null) {
    blockers.push(
      blockerRef(input.workOrderRef, 'missing-prompt-suggestions-snapshot-ref'),
    )
  }

  if (omittedUnsafeRefCount > 0) {
    blockers.push(
      blockerRef(input.workOrderRef, 'unsafe-prompt-suggestions-material-omitted'),
    )
  }

  const uniqueBlockers = Array.from(new Set(blockers))

  return {
    authority,
    blockerRefs: uniqueBlockers,
    counts: counts(entries),
    entries,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    snapshotRef: snapshotRef.ref,
    status: statusFrom(entries, uniqueBlockers),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgePromptSuggestionsInput = (
  work: AutopilotWorkProjection,
): ForgePromptSuggestionsInput => {
  const evidence = work.promptSuggestionsEvidence

  return {
    generatedAt: evidence?.generatedAt ?? work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(evidence?.blockerRefs === undefined
      ? {}
      : { blockerRefs: evidence.blockerRefs }),
    ...(evidence?.entries === undefined ? {} : { entries: evidence.entries }),
    ...(evidence?.snapshotRef === undefined
      ? {}
      : { snapshotRef: evidence.snapshotRef }),
    ...(evidence?.versionRef === undefined
      ? {}
      : { versionRef: evidence.versionRef }),
  }
}

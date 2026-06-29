import type {
  AutopilotWorkAccessibilityInteractionMode,
  AutopilotWorkAccessibilityNonInteractiveEntry,
  AutopilotWorkAccessibilityNonInteractiveFreshness,
  AutopilotWorkAccessibilityNonInteractiveStatus,
  AutopilotWorkProjection,
} from '../model'

export type ForgeAccessibilityNonInteractiveStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'unknown'

export type ForgeAccessibilityNonInteractiveAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  approvalGrantAuthority: false
  approvalPolicyMutationAuthority: false
  deployAuthority: false
  exitCodeMutationAuthority: false
  headlessCommandExecutionAuthority: false
  liveSpendAuthority: false
  preferenceWriteAuthority: false
  promptAnswerAuthority: false
  providerAccountMutationAuthority: false
  pushAuthority: false
  remoteBridgeStartAuthority: false
  settlementAuthority: false
  structuredOutputEmitAuthority: false
  terminalCapabilityMutationAuthority: false
  themeInstallAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeAccessibilityNonInteractiveItem = Readonly<{
  approvalResolverRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  ciPolicyRefs: ReadonlyArray<string>
  deployCaveatRefs: ReadonlyArray<string>
  exitCodeRefs: ReadonlyArray<string>
  freshness: AutopilotWorkAccessibilityNonInteractiveFreshness
  highContrastRefs: ReadonlyArray<string>
  keyboardNavigationRefs: ReadonlyArray<string>
  mode: AutopilotWorkAccessibilityInteractionMode
  modeRef: string
  noColorRefs: ReadonlyArray<string>
  notificationAvailabilityRefs: ReadonlyArray<string>
  promptAvailabilityRefs: ReadonlyArray<string>
  providerMutationCaveatRefs: ReadonlyArray<string>
  pushCaveatRefs: ReadonlyArray<string>
  reducedMotionRefs: ReadonlyArray<string>
  remoteBridgeAvailabilityRefs: ReadonlyArray<string>
  schemaRefs: ReadonlyArray<string>
  screenReaderStatusRefs: ReadonlyArray<string>
  spendCaveatRefs: ReadonlyArray<string>
  status: AutopilotWorkAccessibilityNonInteractiveStatus
  statusLabelRefs: ReadonlyArray<string>
  structuredOutputRefs: ReadonlyArray<string>
  terminalCapabilityRefs: ReadonlyArray<string>
  typedPromptBlockerRefs: ReadonlyArray<string>
}>

export type ForgeAccessibilityNonInteractiveInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkAccessibilityNonInteractiveEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeAccessibilityNonInteractiveCounts = Readonly<{
  ci: number
  headless: number
  nonInteractive: number
  ready: number
  screenReader: number
  stale: number
  total: number
}>

export type ForgeAccessibilityNonInteractiveView = Readonly<{
  authority: ForgeAccessibilityNonInteractiveAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeAccessibilityNonInteractiveCounts
  entries: ReadonlyArray<ForgeAccessibilityNonInteractiveItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeAccessibilityNonInteractiveStatus
  versionRef: string | null
  workOrderRef: string
}>

type RefBundle = Readonly<{ omittedUnsafeRefCount: number; refs: ReadonlyArray<string> }>
type OptionalRefBundle = Readonly<{ omittedUnsafeRefCount: number; ref: string | null }>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_ACCESSIBILITY_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:accessibility|capture|json|output|payload|prompt|structured[-_ ]output|terminal)/i,
  /private[-_ ](?:accessibility|capture|data|output|payload|prompt|screen[-_ ]reader|terminal|workspace)/i,
  /provider[-_ ]payload/i,
  /prompt[-_ ]text/i,
  /screen[-_ ]reader[-_ ]transcript/i,
  /structured[-_ ]output[-_ ]payload/i,
  /terminal[-_ ]capture[-_ ]content/i,
  /terminal[-_ ]output[-_ ]content/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret)\b/i,
  /\b(?:admin|auth|bearer|operator|private|refresh|secret|service|session)[_-]?token\b/i,
]

const NON_INTERACTIVE_MODES: ReadonlySet<AutopilotWorkAccessibilityInteractionMode> =
  new Set([
    'ci',
    'headless_service',
    'json_output',
    'non_interactive_command',
  ])

const authority: ForgeAccessibilityNonInteractiveAuthority = {
  acceptedOutcomeAuthority: false,
  approvalGrantAuthority: false,
  approvalPolicyMutationAuthority: false,
  deployAuthority: false,
  exitCodeMutationAuthority: false,
  headlessCommandExecutionAuthority: false,
  liveSpendAuthority: false,
  preferenceWriteAuthority: false,
  promptAnswerAuthority: false,
  providerAccountMutationAuthority: false,
  pushAuthority: false,
  remoteBridgeStartAuthority: false,
  settlementAuthority: false,
  structuredOutputEmitAuthority: false,
  terminalCapabilityMutationAuthority: false,
  themeInstallAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_ACCESSIBILITY_MARKERS.some(marker => marker.test(trimmed))
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

const safeOptionalRef = (value: string | null | undefined): OptionalRefBundle => {
  if (value === null || value === undefined) {
    return { omittedUnsafeRefCount: 0, ref: null }
  }

  const ref = safeRef(value)

  return ref === null
    ? { omittedUnsafeRefCount: 1, ref: null }
    : { omittedUnsafeRefCount: 0, ref }
}

const blockerRef = (workOrderRef: string, suffix: string): string =>
  `forge-accessibility-non-interactive-blocker:${workOrderRef}:${suffix}`

const hasRole = (refs: ReadonlyArray<string>, pattern: RegExp): boolean =>
  refs.some(ref => pattern.test(ref))

const normalizeItem = (
  item: AutopilotWorkAccessibilityNonInteractiveEntry,
): Readonly<{
  item: ForgeAccessibilityNonInteractiveItem | null
  omittedUnsafeRefCount: number
}> => {
  const approvalResolverRefs = safeRefs(item.approvalResolverRefs)
  const blockerRefs = safeRefs(item.blockerRefs)
  const ciPolicyRefs = safeRefs(item.ciPolicyRefs)
  const deployCaveatRefs = safeRefs(item.deployCaveatRefs)
  const exitCodeRefs = safeRefs(item.exitCodeRefs)
  const highContrastRefs = safeRefs(item.highContrastRefs)
  const keyboardNavigationRefs = safeRefs(item.keyboardNavigationRefs)
  const modeRef = safeOptionalRef(item.modeRef)
  const noColorRefs = safeRefs(item.noColorRefs)
  const notificationAvailabilityRefs = safeRefs(item.notificationAvailabilityRefs)
  const promptAvailabilityRefs = safeRefs(item.promptAvailabilityRefs)
  const providerMutationCaveatRefs = safeRefs(item.providerMutationCaveatRefs)
  const pushCaveatRefs = safeRefs(item.pushCaveatRefs)
  const reducedMotionRefs = safeRefs(item.reducedMotionRefs)
  const remoteBridgeAvailabilityRefs = safeRefs(item.remoteBridgeAvailabilityRefs)
  const schemaRefs = safeRefs(item.schemaRefs)
  const screenReaderStatusRefs = safeRefs(item.screenReaderStatusRefs)
  const spendCaveatRefs = safeRefs(item.spendCaveatRefs)
  const statusLabelRefs = safeRefs(item.statusLabelRefs)
  const structuredOutputRefs = safeRefs(item.structuredOutputRefs)
  const terminalCapabilityRefs = safeRefs(item.terminalCapabilityRefs)
  const typedPromptBlockerRefs = safeRefs(item.typedPromptBlockerRefs)
  const omittedUnsafeRefCount =
    approvalResolverRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    ciPolicyRefs.omittedUnsafeRefCount +
    deployCaveatRefs.omittedUnsafeRefCount +
    exitCodeRefs.omittedUnsafeRefCount +
    highContrastRefs.omittedUnsafeRefCount +
    keyboardNavigationRefs.omittedUnsafeRefCount +
    modeRef.omittedUnsafeRefCount +
    noColorRefs.omittedUnsafeRefCount +
    notificationAvailabilityRefs.omittedUnsafeRefCount +
    promptAvailabilityRefs.omittedUnsafeRefCount +
    providerMutationCaveatRefs.omittedUnsafeRefCount +
    pushCaveatRefs.omittedUnsafeRefCount +
    reducedMotionRefs.omittedUnsafeRefCount +
    remoteBridgeAvailabilityRefs.omittedUnsafeRefCount +
    schemaRefs.omittedUnsafeRefCount +
    screenReaderStatusRefs.omittedUnsafeRefCount +
    spendCaveatRefs.omittedUnsafeRefCount +
    statusLabelRefs.omittedUnsafeRefCount +
    structuredOutputRefs.omittedUnsafeRefCount +
    terminalCapabilityRefs.omittedUnsafeRefCount +
    typedPromptBlockerRefs.omittedUnsafeRefCount

  return modeRef.ref === null
    ? { item: null, omittedUnsafeRefCount }
    : {
        item: {
          approvalResolverRefs: approvalResolverRefs.refs,
          blockerRefs: blockerRefs.refs,
          ciPolicyRefs: ciPolicyRefs.refs,
          deployCaveatRefs: deployCaveatRefs.refs,
          exitCodeRefs: exitCodeRefs.refs,
          freshness: item.freshness ?? 'unknown',
          highContrastRefs: highContrastRefs.refs,
          keyboardNavigationRefs: keyboardNavigationRefs.refs,
          mode: item.mode,
          modeRef: modeRef.ref,
          noColorRefs: noColorRefs.refs,
          notificationAvailabilityRefs: notificationAvailabilityRefs.refs,
          promptAvailabilityRefs: promptAvailabilityRefs.refs,
          providerMutationCaveatRefs: providerMutationCaveatRefs.refs,
          pushCaveatRefs: pushCaveatRefs.refs,
          reducedMotionRefs: reducedMotionRefs.refs,
          remoteBridgeAvailabilityRefs: remoteBridgeAvailabilityRefs.refs,
          schemaRefs: schemaRefs.refs,
          screenReaderStatusRefs: screenReaderStatusRefs.refs,
          spendCaveatRefs: spendCaveatRefs.refs,
          status: item.status,
          statusLabelRefs: statusLabelRefs.refs,
          structuredOutputRefs: structuredOutputRefs.refs,
          terminalCapabilityRefs: terminalCapabilityRefs.refs,
          typedPromptBlockerRefs: typedPromptBlockerRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeAccessibilityNonInteractiveItem>,
): ForgeAccessibilityNonInteractiveCounts => ({
  ci: entries.filter(entry => entry.mode === 'ci').length,
  headless: entries.filter(entry => entry.mode === 'headless_service').length,
  nonInteractive: entries.filter(entry => NON_INTERACTIVE_MODES.has(entry.mode)).length,
  ready: entries.filter(entry => entry.status === 'ready').length,
  screenReader: entries.filter(entry => entry.mode === 'screen_reader').length,
  stale: entries.filter(entry => entry.freshness === 'stale' || entry.status === 'stale').length,
  total: entries.length,
})

const promptRequired = (
  item: ForgeAccessibilityNonInteractiveItem,
): boolean =>
  hasRole(
    item.promptAvailabilityRefs,
    /approval[-_.:/]?required|prompt[-_.:/]?required|requires[-_.:/]?prompt|waiting[-_.:/]?for[-_.:/]?prompt/i,
  )

const readyContractMissing = (
  item: ForgeAccessibilityNonInteractiveItem,
): boolean =>
  item.status === 'ready' &&
  (item.structuredOutputRefs.length === 0 ||
    item.schemaRefs.length === 0 ||
    item.statusLabelRefs.length === 0 ||
    item.noColorRefs.length === 0 ||
    item.exitCodeRefs.length === 0)

const ciCaveatMissing = (item: ForgeAccessibilityNonInteractiveItem): boolean =>
  item.mode === 'ci' &&
  (item.ciPolicyRefs.length === 0 ||
    item.spendCaveatRefs.length === 0 ||
    item.pushCaveatRefs.length === 0 ||
    item.deployCaveatRefs.length === 0 ||
    item.providerMutationCaveatRefs.length === 0)

const itemBlockers = (
  workOrderRef: string,
  item: ForgeAccessibilityNonInteractiveItem,
): ReadonlyArray<string> => {
  const blockers = [...item.blockerRefs]

  if (item.freshness === 'stale' || item.status === 'stale') {
    blockers.push(
      blockerRef(
        workOrderRef,
        `stale-accessibility-non-interactive-evidence:${item.modeRef}`,
      ),
    )
  }

  if (readyContractMissing(item)) {
    blockers.push(
      blockerRef(
        workOrderRef,
        `ready-mode-accessibility-contract-missing:${item.modeRef}`,
      ),
    )
  }

  if (
    NON_INTERACTIVE_MODES.has(item.mode) &&
    promptRequired(item) &&
    item.approvalResolverRefs.length === 0 &&
    item.typedPromptBlockerRefs.length === 0
  ) {
    blockers.push(
      blockerRef(workOrderRef, `non-interactive-prompt-blocker-missing:${item.modeRef}`),
    )
  }

  if (ciCaveatMissing(item)) {
    blockers.push(blockerRef(workOrderRef, `ci-safety-caveat-missing:${item.modeRef}`))
  }

  if (
    item.mode === 'screen_reader' &&
    (item.screenReaderStatusRefs.length === 0 ||
      item.keyboardNavigationRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(workOrderRef, `screen-reader-evidence-missing:${item.modeRef}`),
    )
  }

  return blockers
}

const statusFrom = (
  entries: ReadonlyArray<ForgeAccessibilityNonInteractiveItem>,
  blockers: ReadonlyArray<string>,
): ForgeAccessibilityNonInteractiveStatus => {
  if (blockers.length > 0 || entries.some(entry => entry.status === 'blocked')) {
    return 'blocked'
  }
  if (entries.length === 0) {
    return 'empty'
  }
  if (entries.some(entry => entry.freshness === 'stale' || entry.status === 'stale')) {
    return 'stale'
  }
  if (entries.every(entry => entry.status === 'ready')) {
    return 'ready'
  }
  return 'unknown'
}

export const projectForgeAccessibilityNonInteractiveEvidence = (
  input: ForgeAccessibilityNonInteractiveInput,
): ForgeAccessibilityNonInteractiveView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const inputBlockerRefs = safeRefs(input.blockerRefs)
  const normalized = (input.entries ?? []).map(normalizeItem)
  const entries = normalized.flatMap(result => (result.item === null ? [] : [result.item]))
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
      blockerRef(input.workOrderRef, 'missing-accessibility-non-interactive-snapshot-ref'),
    )
  }
  if (omittedUnsafeRefCount > 0) {
    blockers.push(
      blockerRef(input.workOrderRef, 'unsafe-accessibility-non-interactive-material-omitted'),
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

export const buildForgeAccessibilityNonInteractiveInput = (
  work: AutopilotWorkProjection,
): ForgeAccessibilityNonInteractiveInput => {
  const evidence = work.accessibilityNonInteractiveEvidence

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

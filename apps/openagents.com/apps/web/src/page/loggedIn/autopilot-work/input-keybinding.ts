import type {
  AutopilotWorkInputKeybinding,
  AutopilotWorkInputKeybindingEntry,
  AutopilotWorkInputKeybindingFreshness,
  AutopilotWorkInputKeybindingState,
  AutopilotWorkInputMode,
  AutopilotWorkProjection,
} from '../model'

export type ForgeInputKeybindingStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'unknown'

export type ForgeInputKeybindingAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  commandExecutionAuthority: false
  deploymentAuthority: false
  fileReadAuthority: false
  inputCaptureAuthority: false
  inputInjectionAuthority: false
  inputModeWriteAuthority: false
  keybindingExecutionAuthority: false
  keybindingWriteAuthority: false
  providerAuthority: false
  publicClaimAuthority: false
  settlementAuthority: false
  shellExecutionAuthority: false
  terminalProcessAuthority: false
  toolGrantAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeInputKeybindingItem = Readonly<{
  accessibilityRefs: ReadonlyArray<string>
  bindingMapRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  commandDescriptorRefs: ReadonlyArray<string>
  conflictRefs: ReadonlyArray<string>
  freshness: AutopilotWorkInputKeybindingFreshness
  inputModeRef: string
  keymapRefs: ReadonlyArray<string>
  mode: AutopilotWorkInputMode
  nonInteractiveFallbackRefs: ReadonlyArray<string>
  platformRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  state: AutopilotWorkInputKeybindingState
}>

export type ForgeInputKeybindingInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkInputKeybindingEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeInputKeybindingCounts = Readonly<{
  available: number
  blocked: number
  conflicts: number
  interactive: number
  total: number
}>

export type ForgeInputKeybindingView = Readonly<{
  authority: ForgeInputKeybindingAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeInputKeybindingCounts
  entries: ReadonlyArray<ForgeInputKeybindingItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeInputKeybindingStatus
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
const PRIVATE_INPUT_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /key[-_ ](?:log|press|stroke|text)/i,
  /input[-_ ](?:body|content|log|text)/i,
  /raw[-_ ](?:body|command|content|diagnostic|file|input|key|log|memory|output|payload|prompt|provider|request|shell|source|stderr|stdout|terminal|trace|transcript)/i,
  /private[-_ ](?:command|content|diagnostic|input|key|prompt|repo|source|terminal|transcript|workspace)/i,
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

const authority: ForgeInputKeybindingAuthority = {
  acceptedOutcomeAuthority: false,
  commandExecutionAuthority: false,
  deploymentAuthority: false,
  fileReadAuthority: false,
  inputCaptureAuthority: false,
  inputInjectionAuthority: false,
  inputModeWriteAuthority: false,
  keybindingExecutionAuthority: false,
  keybindingWriteAuthority: false,
  providerAuthority: false,
  publicClaimAuthority: false,
  settlementAuthority: false,
  shellExecutionAuthority: false,
  terminalProcessAuthority: false,
  toolGrantAuthority: false,
  workerPayoutAuthority: false,
}

const interactiveModes: ReadonlyArray<AutopilotWorkInputMode> = [
  'command_palette',
  'keyboard',
  'remote_control',
  'slash_command',
]

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_INPUT_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-input-keybinding-blocker:${workOrderRef}:${suffix}`

const normalizeEntry = (
  entry: AutopilotWorkInputKeybindingEntry,
): Readonly<{
  entry: ForgeInputKeybindingItem | null
  omittedUnsafeRefCount: number
}> => {
  const inputModeRef = safeOptionalRef(entry.inputModeRef)
  const accessibilityRefs = safeRefs(entry.accessibilityRefs)
  const bindingMapRefs = safeRefs(entry.bindingMapRefs)
  const blockerRefs = safeRefs(entry.blockerRefs)
  const commandDescriptorRefs = safeRefs(entry.commandDescriptorRefs)
  const conflictRefs = safeRefs(entry.conflictRefs)
  const keymapRefs = safeRefs(entry.keymapRefs)
  const nonInteractiveFallbackRefs = safeRefs(entry.nonInteractiveFallbackRefs)
  const platformRefs = safeRefs(entry.platformRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const omittedUnsafeRefCount =
    inputModeRef.omittedUnsafeRefCount +
    accessibilityRefs.omittedUnsafeRefCount +
    bindingMapRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    commandDescriptorRefs.omittedUnsafeRefCount +
    conflictRefs.omittedUnsafeRefCount +
    keymapRefs.omittedUnsafeRefCount +
    nonInteractiveFallbackRefs.omittedUnsafeRefCount +
    platformRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount

  return inputModeRef.ref === null
    ? { entry: null, omittedUnsafeRefCount }
    : {
        entry: {
          accessibilityRefs: accessibilityRefs.refs,
          bindingMapRefs: bindingMapRefs.refs,
          blockerRefs: blockerRefs.refs,
          commandDescriptorRefs: commandDescriptorRefs.refs,
          conflictRefs: conflictRefs.refs,
          freshness: entry.freshness ?? 'unknown',
          inputModeRef: inputModeRef.ref,
          keymapRefs: keymapRefs.refs,
          mode: entry.mode,
          nonInteractiveFallbackRefs: nonInteractiveFallbackRefs.refs,
          platformRefs: platformRefs.refs,
          policyRefs: policyRefs.refs,
          state: entry.state,
        },
        omittedUnsafeRefCount,
      }
}

const inputCounts = (
  entries: ReadonlyArray<ForgeInputKeybindingItem>,
): ForgeInputKeybindingCounts => ({
  available: entries.filter(entry => entry.state === 'available').length,
  blocked: entries.filter(entry => entry.state === 'blocked').length,
  conflicts: entries.filter(
    entry => entry.state === 'degraded' || entry.conflictRefs.length > 0,
  ).length,
  interactive: entries.filter(entry => interactiveModes.includes(entry.mode)).length,
  total: entries.length,
})

const staleInputBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeInputKeybindingItem>,
): ReadonlyArray<string> =>
  entries
    .filter(entry => entry.freshness === 'stale' && entry.blockerRefs.length === 0)
    .map(entry => blockerRef(workOrderRef, `stale-input-evidence:${entry.inputModeRef}`))

const policyBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeInputKeybindingItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.state === 'available' &&
        entry.policyRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `input-policy-missing:${entry.inputModeRef}`))

const conflictBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeInputKeybindingItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.state === 'degraded' &&
        entry.conflictRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `conflict-evidence-missing:${entry.inputModeRef}`))

const fallbackBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeInputKeybindingItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        interactiveModes.includes(entry.mode) &&
        entry.state === 'available' &&
        entry.nonInteractiveFallbackRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `non-interactive-fallback-missing:${entry.inputModeRef}`))

const commandDescriptorBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeInputKeybindingItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.state === 'available' &&
        entry.commandDescriptorRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `command-descriptor-missing:${entry.inputModeRef}`))

const statusForView = (
  entries: ReadonlyArray<ForgeInputKeybindingItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeInputKeybindingStatus => {
  if (blockerRefs.length > 0) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.freshness === 'stale')) {
    return 'stale'
  }

  return entries.every(entry => entry.freshness === 'unknown') ? 'unknown' : 'ready'
}

export const projectForgeInputKeybinding = (
  input: ForgeInputKeybindingInput,
): ForgeInputKeybindingView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedEntries = (input.entries ?? []).map(normalizeEntry)
  const entries = normalizedEntries
    .flatMap(result => (result.entry === null ? [] : [result.entry]))
    .sort(
      (left, right) =>
        left.mode.localeCompare(right.mode) ||
        left.state.localeCompare(right.state) ||
        left.inputModeRef.localeCompare(right.inputModeRef),
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
      ...staleInputBlockers(input.workOrderRef, entries),
      ...policyBlockers(input.workOrderRef, entries),
      ...conflictBlockers(input.workOrderRef, entries),
      ...fallbackBlockers(input.workOrderRef, entries),
      ...commandDescriptorBlockers(input.workOrderRef, entries),
      ...(input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-input-keybinding-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-input-keybinding-material-omitted')]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    counts: inputCounts(entries),
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

export const buildForgeInputKeybindingInput = (
  work: AutopilotWorkProjection,
): ForgeInputKeybindingInput => {
  const source: AutopilotWorkInputKeybinding | undefined = work.inputKeybinding

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

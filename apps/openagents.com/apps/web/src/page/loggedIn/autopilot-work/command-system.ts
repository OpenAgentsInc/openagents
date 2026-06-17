import type {
  AutopilotWorkCommandEntry,
  AutopilotWorkCommandFreshness,
  AutopilotWorkCommandKind,
  AutopilotWorkCommandState,
  AutopilotWorkCommandSystem,
  AutopilotWorkProjection,
} from '../model'

export type ForgeCommandSystemStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'unknown'

export type ForgeCommandSystemAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  catalogWriteAuthority: false
  commandExecutionAuthority: false
  deploymentAuthority: false
  fileReadAuthority: false
  inputModeWriteAuthority: false
  intentRoutingAuthority: false
  keybindingWriteAuthority: false
  parserExecutionAuthority: false
  providerAuthority: false
  publicClaimAuthority: false
  retrievalRoutingAuthority: false
  settingsWriteAuthority: false
  settlementAuthority: false
  shellExecutionAuthority: false
  toolGrantAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeCommandItem = Readonly<{
  blockerRefs: ReadonlyArray<string>
  capabilityRefs: ReadonlyArray<string>
  commandDescriptorRefs: ReadonlyArray<string>
  commandRef: string
  conflictRefs: ReadonlyArray<string>
  fallbackRefs: ReadonlyArray<string>
  freshness: AutopilotWorkCommandFreshness
  inputModeRefs: ReadonlyArray<string>
  kind: AutopilotWorkCommandKind
  parserRefs: ReadonlyArray<string>
  plannerRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  selectorRefs: ReadonlyArray<string>
  state: AutopilotWorkCommandState
}>

export type ForgeCommandSystemInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  commands?: ReadonlyArray<AutopilotWorkCommandEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeCommandSystemCounts = Readonly<{
  available: number
  blocked: number
  conflicted: number
  total: number
  unavailable: number
}>

export type ForgeCommandSystemView = Readonly<{
  authority: ForgeCommandSystemAuthority
  blockerRefs: ReadonlyArray<string>
  commands: ReadonlyArray<ForgeCommandItem>
  counts: ForgeCommandSystemCounts
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeCommandSystemStatus
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
const PRIVATE_COMMAND_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /command[-_ ](?:body|content|line|text)/i,
  /prompt[-_ ](?:body|content|text)/i,
  /raw[-_ ](?:body|command|content|diagnostic|file|input|key|log|memory|output|payload|prompt|provider|request|shell|source|stderr|stdout|terminal|trace|transcript)/i,
  /private[-_ ](?:command|content|diagnostic|input|prompt|repo|source|terminal|transcript|workspace)/i,
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

const authority: ForgeCommandSystemAuthority = {
  acceptedOutcomeAuthority: false,
  catalogWriteAuthority: false,
  commandExecutionAuthority: false,
  deploymentAuthority: false,
  fileReadAuthority: false,
  inputModeWriteAuthority: false,
  intentRoutingAuthority: false,
  keybindingWriteAuthority: false,
  parserExecutionAuthority: false,
  providerAuthority: false,
  publicClaimAuthority: false,
  retrievalRoutingAuthority: false,
  settingsWriteAuthority: false,
  settlementAuthority: false,
  shellExecutionAuthority: false,
  toolGrantAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_COMMAND_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-command-system-blocker:${workOrderRef}:${suffix}`

const normalizeCommand = (
  command: AutopilotWorkCommandEntry,
): Readonly<{
  command: ForgeCommandItem | null
  omittedUnsafeRefCount: number
}> => {
  const commandRef = safeOptionalRef(command.commandRef)
  const blockerRefs = safeRefs(command.blockerRefs)
  const capabilityRefs = safeRefs(command.capabilityRefs)
  const commandDescriptorRefs = safeRefs(command.commandDescriptorRefs)
  const conflictRefs = safeRefs(command.conflictRefs)
  const fallbackRefs = safeRefs(command.fallbackRefs)
  const inputModeRefs = safeRefs(command.inputModeRefs)
  const parserRefs = safeRefs(command.parserRefs)
  const plannerRefs = safeRefs(command.plannerRefs)
  const policyRefs = safeRefs(command.policyRefs)
  const selectorRefs = safeRefs(command.selectorRefs)
  const omittedUnsafeRefCount =
    commandRef.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    capabilityRefs.omittedUnsafeRefCount +
    commandDescriptorRefs.omittedUnsafeRefCount +
    conflictRefs.omittedUnsafeRefCount +
    fallbackRefs.omittedUnsafeRefCount +
    inputModeRefs.omittedUnsafeRefCount +
    parserRefs.omittedUnsafeRefCount +
    plannerRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    selectorRefs.omittedUnsafeRefCount

  return commandRef.ref === null
    ? { command: null, omittedUnsafeRefCount }
    : {
        command: {
          blockerRefs: blockerRefs.refs,
          capabilityRefs: capabilityRefs.refs,
          commandDescriptorRefs: commandDescriptorRefs.refs,
          commandRef: commandRef.ref,
          conflictRefs: conflictRefs.refs,
          fallbackRefs: fallbackRefs.refs,
          freshness: command.freshness ?? 'unknown',
          inputModeRefs: inputModeRefs.refs,
          kind: command.kind,
          parserRefs: parserRefs.refs,
          plannerRefs: plannerRefs.refs,
          policyRefs: policyRefs.refs,
          selectorRefs: selectorRefs.refs,
          state: command.state,
        },
        omittedUnsafeRefCount,
      }
}

const commandCounts = (
  commands: ReadonlyArray<ForgeCommandItem>,
): ForgeCommandSystemCounts => ({
  available: commands.filter(command => command.state === 'available').length,
  blocked: commands.filter(command => command.state === 'blocked').length,
  conflicted: commands.filter(command => command.state === 'conflicted').length,
  total: commands.length,
  unavailable: commands.filter(command => command.state === 'unavailable').length,
})

const staleCommandBlockers = (
  workOrderRef: string,
  commands: ReadonlyArray<ForgeCommandItem>,
): ReadonlyArray<string> =>
  commands
    .filter(command => command.freshness === 'stale' && command.blockerRefs.length === 0)
    .map(command => blockerRef(workOrderRef, `stale-command-evidence:${command.commandRef}`))

const policyBlockers = (
  workOrderRef: string,
  commands: ReadonlyArray<ForgeCommandItem>,
): ReadonlyArray<string> =>
  commands
    .filter(
      command =>
        command.state === 'available' &&
        command.policyRefs.length === 0 &&
        command.blockerRefs.length === 0,
    )
    .map(command => blockerRef(workOrderRef, `command-policy-missing:${command.commandRef}`))

const semanticRouteBlockers = (
  workOrderRef: string,
  commands: ReadonlyArray<ForgeCommandItem>,
): ReadonlyArray<string> =>
  commands
    .filter(
      command =>
        command.state === 'available' &&
        command.selectorRefs.length === 0 &&
        command.blockerRefs.length === 0,
    )
    .map(command => blockerRef(workOrderRef, `semantic-selector-missing:${command.commandRef}`))

const parserPlannerBlockers = (
  workOrderRef: string,
  commands: ReadonlyArray<ForgeCommandItem>,
): ReadonlyArray<string> =>
  commands.flatMap(command =>
    command.state === 'available' && command.blockerRefs.length === 0
      ? [
          ...(command.parserRefs.length === 0
            ? [blockerRef(workOrderRef, `parser-ref-missing:${command.commandRef}`)]
            : []),
          ...(command.plannerRefs.length === 0
            ? [blockerRef(workOrderRef, `planner-ref-missing:${command.commandRef}`)]
            : []),
        ]
      : [],
  )

const conflictBlockers = (
  workOrderRef: string,
  commands: ReadonlyArray<ForgeCommandItem>,
): ReadonlyArray<string> =>
  commands
    .filter(
      command =>
        command.state === 'conflicted' &&
        command.conflictRefs.length === 0 &&
        command.blockerRefs.length === 0,
    )
    .map(command => blockerRef(workOrderRef, `conflict-evidence-missing:${command.commandRef}`))

const fallbackBlockers = (
  workOrderRef: string,
  commands: ReadonlyArray<ForgeCommandItem>,
): ReadonlyArray<string> =>
  commands
    .filter(
      command =>
        command.state === 'unavailable' &&
        command.fallbackRefs.length === 0 &&
        command.blockerRefs.length === 0,
    )
    .map(command => blockerRef(workOrderRef, `fallback-ref-missing:${command.commandRef}`))

const descriptorBlockers = (
  workOrderRef: string,
  commands: ReadonlyArray<ForgeCommandItem>,
): ReadonlyArray<string> =>
  commands
    .filter(
      command =>
        command.state === 'available' &&
        command.commandDescriptorRefs.length === 0 &&
        command.blockerRefs.length === 0,
    )
    .map(command => blockerRef(workOrderRef, `command-descriptor-missing:${command.commandRef}`))

const statusForView = (
  commands: ReadonlyArray<ForgeCommandItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeCommandSystemStatus => {
  if (blockerRefs.length > 0) {
    return 'blocked'
  }

  if (commands.length === 0) {
    return 'empty'
  }

  if (commands.some(command => command.freshness === 'stale')) {
    return 'stale'
  }

  return commands.every(command => command.freshness === 'unknown') ? 'unknown' : 'ready'
}

export const projectForgeCommandSystem = (
  input: ForgeCommandSystemInput,
): ForgeCommandSystemView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedCommands = (input.commands ?? []).map(normalizeCommand)
  const commands = normalizedCommands
    .flatMap(result => (result.command === null ? [] : [result.command]))
    .sort(
      (left, right) =>
        left.kind.localeCompare(right.kind) ||
        left.state.localeCompare(right.state) ||
        left.commandRef.localeCompare(right.commandRef),
    )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    normalizedCommands.reduce((sum, result) => sum + result.omittedUnsafeRefCount, 0)
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...commands.flatMap(command => command.blockerRefs),
      ...staleCommandBlockers(input.workOrderRef, commands),
      ...policyBlockers(input.workOrderRef, commands),
      ...semanticRouteBlockers(input.workOrderRef, commands),
      ...parserPlannerBlockers(input.workOrderRef, commands),
      ...conflictBlockers(input.workOrderRef, commands),
      ...fallbackBlockers(input.workOrderRef, commands),
      ...descriptorBlockers(input.workOrderRef, commands),
      ...(input.commands !== undefined && input.commands.length > 0 && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-command-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-command-material-omitted')]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    commands,
    counts: commandCounts(commands),
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    snapshotRef: snapshotRef.ref,
    status: statusForView(commands, blockerRefs),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeCommandSystemInput = (
  work: AutopilotWorkProjection,
): ForgeCommandSystemInput => {
  const source: AutopilotWorkCommandSystem | undefined = work.commandSystem

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
    ...(source.commands === undefined ? {} : { commands: source.commands }),
    snapshotRef: source.snapshotRef,
    ...(source.versionRef === undefined ? {} : { versionRef: source.versionRef }),
  }
}

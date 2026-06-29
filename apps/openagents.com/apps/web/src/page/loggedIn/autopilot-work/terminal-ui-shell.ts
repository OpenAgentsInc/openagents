import type {
  AutopilotWorkProjection,
  AutopilotWorkTerminalSurface,
  AutopilotWorkTerminalSurfaceFreshness,
  AutopilotWorkTerminalSurfaceMode,
  AutopilotWorkTerminalSurfaceState,
  AutopilotWorkTerminalUiShell,
} from '../model'

export type ForgeTerminalUiShellStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'unknown'

export type ForgeTerminalUiShellAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  commandExecutionAuthority: false
  deploymentAuthority: false
  fileReadAuthority: false
  inputInjectionAuthority: false
  keybindingWriteAuthority: false
  providerAuthority: false
  ptyAuthority: false
  publicClaimAuthority: false
  settlementAuthority: false
  shellExecutionAuthority: false
  terminalEmulatorAuthority: false
  terminalProcessAuthority: false
  toolGrantAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeTerminalSurfaceItem = Readonly<{
  accessibilityRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  commandDescriptorRefs: ReadonlyArray<string>
  freshness: AutopilotWorkTerminalSurfaceFreshness
  inputDescriptorRefs: ReadonlyArray<string>
  mode: AutopilotWorkTerminalSurfaceMode
  nonInteractiveRefs: ReadonlyArray<string>
  paneRefs: ReadonlyArray<string>
  parityRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  shellRefs: ReadonlyArray<string>
  state: AutopilotWorkTerminalSurfaceState
  streamRefs: ReadonlyArray<string>
  surfaceRef: string
  transcriptSummaryRefs: ReadonlyArray<string>
}>

export type ForgeTerminalUiShellInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  generatedAt: string
  snapshotRef?: string
  surfaces?: ReadonlyArray<AutopilotWorkTerminalSurface>
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeTerminalUiShellCounts = Readonly<{
  available: number
  blocked: number
  degraded: number
  interactive: number
  total: number
}>

export type ForgeTerminalUiShellView = Readonly<{
  authority: ForgeTerminalUiShellAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeTerminalUiShellCounts
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeTerminalUiShellStatus
  surfaces: ReadonlyArray<ForgeTerminalSurfaceItem>
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
const PRIVATE_TERMINAL_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:body|command|content|diagnostic|file|input|log|memory|output|payload|prompt|provider|request|shell|source|stderr|stdout|terminal|trace|transcript)/i,
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

const authority: ForgeTerminalUiShellAuthority = {
  acceptedOutcomeAuthority: false,
  commandExecutionAuthority: false,
  deploymentAuthority: false,
  fileReadAuthority: false,
  inputInjectionAuthority: false,
  keybindingWriteAuthority: false,
  providerAuthority: false,
  ptyAuthority: false,
  publicClaimAuthority: false,
  settlementAuthority: false,
  shellExecutionAuthority: false,
  terminalEmulatorAuthority: false,
  terminalProcessAuthority: false,
  toolGrantAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_TERMINAL_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-terminal-ui-shell-blocker:${workOrderRef}:${suffix}`

const normalizeSurface = (
  surface: AutopilotWorkTerminalSurface,
): Readonly<{
  omittedUnsafeRefCount: number
  surface: ForgeTerminalSurfaceItem | null
}> => {
  const surfaceRef = safeOptionalRef(surface.surfaceRef)
  const accessibilityRefs = safeRefs(surface.accessibilityRefs)
  const blockerRefs = safeRefs(surface.blockerRefs)
  const commandDescriptorRefs = safeRefs(surface.commandDescriptorRefs)
  const inputDescriptorRefs = safeRefs(surface.inputDescriptorRefs)
  const nonInteractiveRefs = safeRefs(surface.nonInteractiveRefs)
  const paneRefs = safeRefs(surface.paneRefs)
  const parityRefs = safeRefs(surface.parityRefs)
  const policyRefs = safeRefs(surface.policyRefs)
  const shellRefs = safeRefs(surface.shellRefs)
  const streamRefs = safeRefs(surface.streamRefs)
  const transcriptSummaryRefs = safeRefs(surface.transcriptSummaryRefs)
  const omittedUnsafeRefCount =
    surfaceRef.omittedUnsafeRefCount +
    accessibilityRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    commandDescriptorRefs.omittedUnsafeRefCount +
    inputDescriptorRefs.omittedUnsafeRefCount +
    nonInteractiveRefs.omittedUnsafeRefCount +
    paneRefs.omittedUnsafeRefCount +
    parityRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    shellRefs.omittedUnsafeRefCount +
    streamRefs.omittedUnsafeRefCount +
    transcriptSummaryRefs.omittedUnsafeRefCount

  return surfaceRef.ref === null
    ? { omittedUnsafeRefCount, surface: null }
    : {
        omittedUnsafeRefCount,
        surface: {
          accessibilityRefs: accessibilityRefs.refs,
          blockerRefs: blockerRefs.refs,
          commandDescriptorRefs: commandDescriptorRefs.refs,
          freshness: surface.freshness ?? 'unknown',
          inputDescriptorRefs: inputDescriptorRefs.refs,
          mode: surface.mode,
          nonInteractiveRefs: nonInteractiveRefs.refs,
          paneRefs: paneRefs.refs,
          parityRefs: parityRefs.refs,
          policyRefs: policyRefs.refs,
          shellRefs: shellRefs.refs,
          state: surface.state,
          streamRefs: streamRefs.refs,
          surfaceRef: surfaceRef.ref,
          transcriptSummaryRefs: transcriptSummaryRefs.refs,
        },
      }
}

const terminalCounts = (
  surfaces: ReadonlyArray<ForgeTerminalSurfaceItem>,
): ForgeTerminalUiShellCounts => ({
  available: surfaces.filter(surface => surface.state === 'available').length,
  blocked: surfaces.filter(surface => surface.state === 'blocked').length,
  degraded: surfaces.filter(surface => surface.state === 'degraded').length,
  interactive: surfaces.filter(surface => surface.mode === 'interactive').length,
  total: surfaces.length,
})

const staleSurfaceBlockers = (
  workOrderRef: string,
  surfaces: ReadonlyArray<ForgeTerminalSurfaceItem>,
): ReadonlyArray<string> =>
  surfaces
    .filter(surface => surface.freshness === 'stale' && surface.blockerRefs.length === 0)
    .map(surface => blockerRef(workOrderRef, `stale-surface-evidence:${surface.surfaceRef}`))

const interactivePolicyBlockers = (
  workOrderRef: string,
  surfaces: ReadonlyArray<ForgeTerminalSurfaceItem>,
): ReadonlyArray<string> =>
  surfaces
    .filter(
      surface =>
        surface.mode === 'interactive' &&
        surface.state === 'available' &&
        surface.policyRefs.length === 0 &&
        surface.blockerRefs.length === 0,
    )
    .map(surface => blockerRef(workOrderRef, `interactive-policy-missing:${surface.surfaceRef}`))

const missingEvidenceBlockers = (
  workOrderRef: string,
  surfaces: ReadonlyArray<ForgeTerminalSurfaceItem>,
): ReadonlyArray<string> =>
  surfaces.flatMap(surface => [
    ...(surface.state === 'available' && surface.shellRefs.length === 0
      ? [blockerRef(workOrderRef, `missing-shell-evidence:${surface.surfaceRef}`)]
      : []),
    ...(surface.state === 'available' &&
    surface.streamRefs.length === 0 &&
    surface.paneRefs.length === 0
      ? [blockerRef(workOrderRef, `missing-stream-or-pane-evidence:${surface.surfaceRef}`)]
      : []),
  ])

const statusForView = (
  surfaces: ReadonlyArray<ForgeTerminalSurfaceItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeTerminalUiShellStatus => {
  if (blockerRefs.length > 0) {
    return 'blocked'
  }

  if (surfaces.length === 0) {
    return 'empty'
  }

  if (surfaces.some(surface => surface.freshness === 'stale')) {
    return 'stale'
  }

  return surfaces.every(surface => surface.freshness === 'unknown') ? 'unknown' : 'ready'
}

export const projectForgeTerminalUiShell = (
  input: ForgeTerminalUiShellInput,
): ForgeTerminalUiShellView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedSurfaces = (input.surfaces ?? []).map(normalizeSurface)
  const surfaces = normalizedSurfaces
    .flatMap(result => (result.surface === null ? [] : [result.surface]))
    .sort(
      (left, right) =>
        left.mode.localeCompare(right.mode) ||
        left.state.localeCompare(right.state) ||
        left.surfaceRef.localeCompare(right.surfaceRef),
    )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    normalizedSurfaces.reduce((sum, result) => sum + result.omittedUnsafeRefCount, 0)
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...surfaces.flatMap(surface => surface.blockerRefs),
      ...staleSurfaceBlockers(input.workOrderRef, surfaces),
      ...interactivePolicyBlockers(input.workOrderRef, surfaces),
      ...missingEvidenceBlockers(input.workOrderRef, surfaces),
      ...(input.surfaces !== undefined && input.surfaces.length > 0 && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-terminal-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-terminal-material-omitted')]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    counts: terminalCounts(surfaces),
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    snapshotRef: snapshotRef.ref,
    status: statusForView(surfaces, blockerRefs),
    surfaces,
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeTerminalUiShellInput = (
  work: AutopilotWorkProjection,
): ForgeTerminalUiShellInput => {
  const source: AutopilotWorkTerminalUiShell | undefined = work.terminalUiShell

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
    snapshotRef: source.snapshotRef,
    ...(source.surfaces === undefined ? {} : { surfaces: source.surfaces }),
    ...(source.versionRef === undefined ? {} : { versionRef: source.versionRef }),
  }
}

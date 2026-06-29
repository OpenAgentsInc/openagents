import type {
  AutopilotWorkInstructionFreshness,
  AutopilotWorkInstructionLayer,
  AutopilotWorkInstructionLayerKind,
  AutopilotWorkInstructionLayerState,
  AutopilotWorkInstructionLayering,
  AutopilotWorkInstructionRedactionClass,
  AutopilotWorkProjection,
} from '../model'

export type ForgeInstructionLayeringStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'unknown'

export type ForgeInstructionLayeringAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  deploymentAuthority: false
  memoryWriteAuthority: false
  modelCallAuthority: false
  promptAssemblyAuthority: false
  promptOverrideWriteAuthority: false
  publicClaimAuthority: false
  settingsWriteAuthority: false
  settlementAuthority: false
  skillCommandLoadAuthority: false
  toolGrantAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeInstructionLayerItem = Readonly<{
  allowedToolRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  capabilityDeltaRefs: ReadonlyArray<string>
  freshness: AutopilotWorkInstructionFreshness
  kind: AutopilotWorkInstructionLayerKind
  layerRef: string
  metadataRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  precedence: number
  redactionClass: AutopilotWorkInstructionRedactionClass
  replacementSourceRef: string | null
  sourceRefs: ReadonlyArray<string>
  state: AutopilotWorkInstructionLayerState
  tokenEstimate: number | null
}>

export type ForgeInstructionLayeringInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  generatedAt: string
  layers?: ReadonlyArray<AutopilotWorkInstructionLayer>
  projectionRef?: string | null
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeInstructionLayeringCounts = Readonly<{
  appended: number
  applied: number
  localOnly: number
  replaced: number
  skipped: number
  stale: number
  total: number
}>

export type ForgeInstructionLayeringView = Readonly<{
  authority: ForgeInstructionLayeringAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeInstructionLayeringCounts
  generatedAt: string
  layers: ReadonlyArray<ForgeInstructionLayerItem>
  omittedUnsafeRefCount: number
  projectionRef: string | null
  publicSafe: true
  snapshotRef: string | null
  status: ForgeInstructionLayeringStatus
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
const PRIVATE_INSTRUCTION_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /prompt[-_ ](?:body|text|content)/i,
  /provider[-_ ]prompt/i,
  /raw[-_ ](?:body|content|diagnostic|file|instruction|log|memory|payload|prompt|provider|request|shell|source|trace|transcript)/i,
  /private[-_ ](?:content|diagnostic|instruction|memory|prompt|repo|source|workspace)/i,
  /provider[-_ ]payload/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const authority: ForgeInstructionLayeringAuthority = {
  acceptedOutcomeAuthority: false,
  deploymentAuthority: false,
  memoryWriteAuthority: false,
  modelCallAuthority: false,
  promptAssemblyAuthority: false,
  promptOverrideWriteAuthority: false,
  publicClaimAuthority: false,
  settingsWriteAuthority: false,
  settlementAuthority: false,
  skillCommandLoadAuthority: false,
  toolGrantAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_INSTRUCTION_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-instruction-layering-blocker:${workOrderRef}:${suffix}`

const normalizeLayer = (
  layer: AutopilotWorkInstructionLayer,
): Readonly<{
  layer: ForgeInstructionLayerItem | null
  omittedUnsafeRefCount: number
}> => {
  const layerRef = safeOptionalRef(layer.layerRef)
  const replacementSourceRef = safeOptionalRef(layer.replacementSourceRef)
  const allowedToolRefs = safeRefs(layer.allowedToolRefs)
  const blockerRefs = safeRefs(layer.blockerRefs)
  const capabilityDeltaRefs = safeRefs(layer.capabilityDeltaRefs)
  const metadataRefs = safeRefs(layer.metadataRefs)
  const policyRefs = safeRefs(layer.policyRefs)
  const sourceRefs = safeRefs(layer.sourceRefs)
  const omittedUnsafeRefCount =
    layerRef.omittedUnsafeRefCount +
    replacementSourceRef.omittedUnsafeRefCount +
    allowedToolRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    capabilityDeltaRefs.omittedUnsafeRefCount +
    metadataRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    sourceRefs.omittedUnsafeRefCount

  return layerRef.ref === null
    ? { layer: null, omittedUnsafeRefCount }
    : {
        layer: {
          allowedToolRefs: allowedToolRefs.refs,
          blockerRefs: blockerRefs.refs,
          capabilityDeltaRefs: capabilityDeltaRefs.refs,
          freshness: layer.freshness ?? 'unknown',
          kind: layer.kind,
          layerRef: layerRef.ref,
          metadataRefs: metadataRefs.refs,
          policyRefs: policyRefs.refs,
          precedence: layer.precedence,
          redactionClass: layer.redactionClass ?? 'private_ref',
          replacementSourceRef: replacementSourceRef.ref,
          sourceRefs: sourceRefs.refs,
          state: layer.state,
          tokenEstimate: layer.tokenEstimate ?? null,
        },
        omittedUnsafeRefCount,
      }
}

const layerCounts = (
  layers: ReadonlyArray<ForgeInstructionLayerItem>,
): ForgeInstructionLayeringCounts => ({
  appended: layers.filter(layer => layer.state === 'appended').length,
  applied: layers.filter(layer => layer.state === 'applied').length,
  localOnly: layers.filter(layer => layer.redactionClass === 'local_only').length,
  replaced: layers.filter(layer => layer.state === 'replaced').length,
  skipped: layers.filter(layer => layer.state === 'skipped').length,
  stale: layers.filter(layer => layer.freshness === 'stale').length,
  total: layers.length,
})

const runtimePolicyBlockers = (
  workOrderRef: string,
  layers: ReadonlyArray<ForgeInstructionLayerItem>,
): ReadonlyArray<string> =>
  layers
    .filter(
      layer =>
        layer.kind === 'runtime_policy' &&
        (layer.state === 'replaced' || layer.state === 'skipped'),
    )
    .map(layer => blockerRef(workOrderRef, `runtime-policy-not-overridable:${layer.layerRef}`))

const replacementEvidenceBlockers = (
  workOrderRef: string,
  layers: ReadonlyArray<ForgeInstructionLayerItem>,
): ReadonlyArray<string> =>
  layers
    .filter(
      layer =>
        layer.state === 'replaced' &&
        layer.replacementSourceRef === null &&
        layer.blockerRefs.length === 0,
    )
    .map(layer => blockerRef(workOrderRef, `replacement-evidence-missing:${layer.layerRef}`))

const toolGrantPolicyBlockers = (
  workOrderRef: string,
  layers: ReadonlyArray<ForgeInstructionLayerItem>,
): ReadonlyArray<string> =>
  layers
    .filter(
      layer =>
        (layer.kind === 'skill_instruction' ||
          layer.kind === 'command_instruction') &&
        layer.allowedToolRefs.length > 0 &&
        layer.policyRefs.length === 0,
    )
    .map(layer => blockerRef(workOrderRef, `tool-grant-policy-missing:${layer.layerRef}`))

const providerProjectionBlockers = (
  workOrderRef: string,
  projectionRef: string | null,
  snapshotRef: string | null,
): ReadonlyArray<string> =>
  projectionRef !== null && snapshotRef === null
    ? [blockerRef(workOrderRef, 'provider-projection-without-snapshot')]
    : []

const statusForView = (
  layers: ReadonlyArray<ForgeInstructionLayerItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeInstructionLayeringStatus => {
  if (blockerRefs.length > 0) {
    return 'blocked'
  }

  if (layers.length === 0) {
    return 'empty'
  }

  if (layers.some(layer => layer.freshness === 'stale')) {
    return 'stale'
  }

  return layers.every(layer => layer.freshness === 'unknown') ? 'unknown' : 'ready'
}

export const projectForgeInstructionLayering = (
  input: ForgeInstructionLayeringInput,
): ForgeInstructionLayeringView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const projectionRef = safeOptionalRef(input.projectionRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedLayers = (input.layers ?? []).map(normalizeLayer)
  const layers = normalizedLayers
    .flatMap(result => (result.layer === null ? [] : [result.layer]))
    .sort((left, right) => left.precedence - right.precedence)
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    projectionRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    normalizedLayers.reduce((sum, result) => sum + result.omittedUnsafeRefCount, 0)
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...layers.flatMap(layer => layer.blockerRefs),
      ...runtimePolicyBlockers(input.workOrderRef, layers),
      ...replacementEvidenceBlockers(input.workOrderRef, layers),
      ...toolGrantPolicyBlockers(input.workOrderRef, layers),
      ...providerProjectionBlockers(
        input.workOrderRef,
        projectionRef.ref,
        snapshotRef.ref,
      ),
      ...(input.layers !== undefined && input.layers.length > 0 && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-instruction-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-instruction-material-omitted')]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    counts: layerCounts(layers),
    generatedAt: input.generatedAt,
    layers,
    omittedUnsafeRefCount,
    projectionRef: projectionRef.ref,
    publicSafe: true,
    snapshotRef: snapshotRef.ref,
    status: statusForView(layers, blockerRefs),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeInstructionLayeringInput = (
  work: AutopilotWorkProjection,
): ForgeInstructionLayeringInput => {
  const source: AutopilotWorkInstructionLayering | undefined =
    work.instructionLayering

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
    ...(source.layers === undefined ? {} : { layers: source.layers }),
    ...(source.projectionRef === undefined ? {} : { projectionRef: source.projectionRef }),
    snapshotRef: source.snapshotRef,
    ...(source.versionRef === undefined ? {} : { versionRef: source.versionRef }),
  }
}

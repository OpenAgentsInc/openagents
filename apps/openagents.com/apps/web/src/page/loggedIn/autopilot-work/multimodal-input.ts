import type {
  AutopilotWorkMultimodalInput,
  AutopilotWorkMultimodalInputEntry,
  AutopilotWorkMultimodalInputFreshness,
  AutopilotWorkMultimodalInputModality,
  AutopilotWorkMultimodalInputState,
  AutopilotWorkProjection,
} from '../model'

export type ForgeMultimodalInputStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'waiting'
  | 'unknown'

export type ForgeMultimodalInputAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  cameraAccessAuthority: false
  clipboardReadAuthority: false
  deploymentAuthority: false
  fileAttachAuthority: false
  fileReadAuthority: false
  imageProcessingAuthority: false
  instructionInjectionAuthority: false
  mediaCaptureAuthority: false
  microphoneAccessAuthority: false
  promptAuthority: false
  publicClaimAuthority: false
  screenCaptureAuthority: false
  settlementAuthority: false
  shellExecutionAuthority: false
  transcriptionAuthority: false
  toolExecutionAuthority: false
  toolRoutingAuthority: false
  vadExecutionAuthority: false
  videoProcessingAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeMultimodalInputItem = Readonly<{
  attachmentRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  captureSurfaceRefs: ReadonlyArray<string>
  consentRefs: ReadonlyArray<string>
  contextIngestionRefs: ReadonlyArray<string>
  endpointRefs: ReadonlyArray<string>
  freshness: AutopilotWorkMultimodalInputFreshness
  inputRef: string
  modality: AutopilotWorkMultimodalInputModality
  policyRefs: ReadonlyArray<string>
  redactionRefs: ReadonlyArray<string>
  state: AutopilotWorkMultimodalInputState
  transcriptRefs: ReadonlyArray<string>
  vadRefs: ReadonlyArray<string>
}>

export type ForgeMultimodalInputInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkMultimodalInputEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeMultimodalInputCounts = Readonly<{
  blocked: number
  captureReady: number
  ingested: number
  pending: number
  total: number
}>

export type ForgeMultimodalInputView = Readonly<{
  authority: ForgeMultimodalInputAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeMultimodalInputCounts
  entries: ReadonlyArray<ForgeMultimodalInputItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeMultimodalInputStatus
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
const PRIVATE_MULTIMODAL_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:audio|body|camera|clipboard|command|content|file|image|input|media|microphone|output|prompt|screen|shell|source|transcript|vad|video)/i,
  /private[-_ ](?:audio|camera|content|file|image|input|media|prompt|screen|source|transcript|video|workspace)/i,
  /transcript[-_ ](?:body|content|raw|text)/i,
  /media[-_ ](?:body|content|payload|raw)/i,
  /provider[-_ ]payload/i,
  /provider[-_ ]prompt/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:data|file|blob|https?):/i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const authority: ForgeMultimodalInputAuthority = {
  acceptedOutcomeAuthority: false,
  cameraAccessAuthority: false,
  clipboardReadAuthority: false,
  deploymentAuthority: false,
  fileAttachAuthority: false,
  fileReadAuthority: false,
  imageProcessingAuthority: false,
  instructionInjectionAuthority: false,
  mediaCaptureAuthority: false,
  microphoneAccessAuthority: false,
  promptAuthority: false,
  publicClaimAuthority: false,
  screenCaptureAuthority: false,
  settlementAuthority: false,
  shellExecutionAuthority: false,
  transcriptionAuthority: false,
  toolExecutionAuthority: false,
  toolRoutingAuthority: false,
  vadExecutionAuthority: false,
  videoProcessingAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_MULTIMODAL_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-multimodal-input-blocker:${workOrderRef}:${suffix}`

const normalizeEntry = (
  entry: AutopilotWorkMultimodalInputEntry,
): Readonly<{
  entry: ForgeMultimodalInputItem | null
  omittedUnsafeRefCount: number
}> => {
  const inputRef = safeOptionalRef(entry.inputRef)
  const attachmentRefs = safeRefs(entry.attachmentRefs)
  const blockerRefs = safeRefs(entry.blockerRefs)
  const captureSurfaceRefs = safeRefs(entry.captureSurfaceRefs)
  const consentRefs = safeRefs(entry.consentRefs)
  const contextIngestionRefs = safeRefs(entry.contextIngestionRefs)
  const endpointRefs = safeRefs(entry.endpointRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const redactionRefs = safeRefs(entry.redactionRefs)
  const transcriptRefs = safeRefs(entry.transcriptRefs)
  const vadRefs = safeRefs(entry.vadRefs)
  const omittedUnsafeRefCount =
    inputRef.omittedUnsafeRefCount +
    attachmentRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    captureSurfaceRefs.omittedUnsafeRefCount +
    consentRefs.omittedUnsafeRefCount +
    contextIngestionRefs.omittedUnsafeRefCount +
    endpointRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    redactionRefs.omittedUnsafeRefCount +
    transcriptRefs.omittedUnsafeRefCount +
    vadRefs.omittedUnsafeRefCount

  return inputRef.ref === null
    ? { entry: null, omittedUnsafeRefCount }
    : {
        entry: {
          attachmentRefs: attachmentRefs.refs,
          blockerRefs: blockerRefs.refs,
          captureSurfaceRefs: captureSurfaceRefs.refs,
          consentRefs: consentRefs.refs,
          contextIngestionRefs: contextIngestionRefs.refs,
          endpointRefs: endpointRefs.refs,
          freshness: entry.freshness ?? 'unknown',
          inputRef: inputRef.ref,
          modality: entry.modality,
          policyRefs: policyRefs.refs,
          redactionRefs: redactionRefs.refs,
          state: entry.state,
          transcriptRefs: transcriptRefs.refs,
          vadRefs: vadRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeMultimodalInputItem>,
): ForgeMultimodalInputCounts => ({
  blocked: entries.filter(entry => entry.state === 'blocked').length,
  captureReady: entries.filter(entry => entry.state === 'capture_ready').length,
  ingested: entries.filter(entry => entry.state === 'ingested').length,
  pending: entries.filter(entry => entry.state === 'pending').length,
  total: entries.length,
})

const staleBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeMultimodalInputItem>,
): ReadonlyArray<string> =>
  entries
    .filter(entry => entry.freshness === 'stale' && entry.blockerRefs.length === 0)
    .map(entry => blockerRef(workOrderRef, `stale-multimodal-evidence:${entry.inputRef}`))

const captureReadyBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeMultimodalInputItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.state === 'capture_ready' &&
        entry.blockerRefs.length === 0 &&
        (entry.consentRefs.length === 0 || entry.policyRefs.length === 0),
    )
    .map(entry => blockerRef(workOrderRef, `capture-consent-policy-missing:${entry.inputRef}`))

const transcriptBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeMultimodalInputItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.transcriptRefs.length > 0 &&
        entry.redactionRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `transcript-redaction-ref-missing:${entry.inputRef}`))

const ingestionBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeMultimodalInputItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.contextIngestionRefs.length > 0 &&
        entry.attachmentRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `context-ingestion-attachment-ref-missing:${entry.inputRef}`))

const statusForView = (
  entries: ReadonlyArray<ForgeMultimodalInputItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeMultimodalInputStatus => {
  if (blockerRefs.length > 0 || entries.some(entry => entry.state === 'blocked')) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.freshness === 'stale')) {
    return 'stale'
  }

  if (entries.some(entry => entry.state === 'pending')) {
    return 'waiting'
  }

  return entries.every(
    entry => entry.state === 'capture_ready' || entry.state === 'ingested',
  )
    ? 'ready'
    : 'unknown'
}

export const projectForgeMultimodalInput = (
  input: ForgeMultimodalInputInput,
): ForgeMultimodalInputView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedEntries = (input.entries ?? []).map(normalizeEntry)
  const entries = normalizedEntries
    .flatMap(result => (result.entry === null ? [] : [result.entry]))
    .sort(
      (left, right) =>
        left.state.localeCompare(right.state) ||
        left.modality.localeCompare(right.modality) ||
        left.inputRef.localeCompare(right.inputRef),
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
      ...captureReadyBlockers(input.workOrderRef, entries),
      ...transcriptBlockers(input.workOrderRef, entries),
      ...ingestionBlockers(input.workOrderRef, entries),
      ...(input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-multimodal-input-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-multimodal-input-material-omitted')]),
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

export const buildForgeMultimodalInputInput = (
  work: AutopilotWorkProjection,
): ForgeMultimodalInputInput => {
  const source: AutopilotWorkMultimodalInput | undefined = work.multimodalInput

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

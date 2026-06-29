import type {
  AutopilotWorkProjection,
  AutopilotWorkStructuredEventLog,
  AutopilotWorkStructuredEventLogEntry,
  AutopilotWorkStructuredEventLogFreshness,
  AutopilotWorkStructuredEventLogKind,
  AutopilotWorkStructuredEventLogRedactionClass,
  AutopilotWorkStructuredEventLogStatus,
  AutopilotWorkStructuredEventLogVisibility,
} from '../model'

export type ForgeStructuredEventLogStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'unknown'

export type ForgeStructuredEventLogAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  deploymentAuthority: false
  eventAppendAuthority: false
  eventDeleteAuthority: false
  eventTailAuthority: false
  exportGenerationAuthority: false
  projectionMutationAuthority: false
  publicClaimAuthority: false
  replayExecutionAuthority: false
  retentionDeletionAuthority: false
  schemaMigrationAuthority: false
  settlementAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeStructuredEventLogItem = Readonly<{
  actorRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  correlationRefs: ReadonlyArray<string>
  eventKind: AutopilotWorkStructuredEventLogKind
  eventRef: string
  exportRefs: ReadonlyArray<string>
  freshness: AutopilotWorkStructuredEventLogFreshness
  idempotencyRefs: ReadonlyArray<string>
  occurredAt: string | null
  parentRefs: ReadonlyArray<string>
  payloadSchemaVersionRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  projectionRefs: ReadonlyArray<string>
  redactionClass: AutopilotWorkStructuredEventLogRedactionClass
  replayRefs: ReadonlyArray<string>
  retentionRefs: ReadonlyArray<string>
  runRefs: ReadonlyArray<string>
  sequence: number
  sequenceRef: string | null
  serviceRefs: ReadonlyArray<string>
  status: AutopilotWorkStructuredEventLogStatus
  subjectRefs: ReadonlyArray<string>
  timestampRefs: ReadonlyArray<string>
  visibility: AutopilotWorkStructuredEventLogVisibility
}>

export type ForgeStructuredEventLogInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  eventStreamRefs?: ReadonlyArray<string>
  events?: ReadonlyArray<AutopilotWorkStructuredEventLogEntry>
  exportRefs?: ReadonlyArray<string>
  generatedAt: string
  policyRefs?: ReadonlyArray<string>
  projectionRefs?: ReadonlyArray<string>
  replayRefs?: ReadonlyArray<string>
  retentionRefs?: ReadonlyArray<string>
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeStructuredEventLogCounts = Readonly<{
  events: number
  failed: number
  privateEvents: number
  publicEvents: number
  stale: number
  teamEvents: number
}>

export type ForgeStructuredEventLogView = Readonly<{
  authority: ForgeStructuredEventLogAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeStructuredEventLogCounts
  eventStreamRefs: ReadonlyArray<string>
  events: ReadonlyArray<ForgeStructuredEventLogItem>
  exportRefs: ReadonlyArray<string>
  generatedAt: string
  omittedUnsafeRefCount: number
  policyRefs: ReadonlyArray<string>
  projectionRefs: ReadonlyArray<string>
  publicSafe: true
  replayRefs: ReadonlyArray<string>
  retentionRefs: ReadonlyArray<string>
  snapshotRef: string | null
  status: ForgeStructuredEventLogStatus
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
const PRIVATE_EVENT_LOG_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:body|command|content|event|file|log|payload|prompt|provider|shell|transcript|wallet)/i,
  /private[-_ ](?:content|event|file|log|payload|prompt|repo|source|transcript|workspace)/i,
  /event[-_ ](?:body|content|payload|raw)/i,
  /provider[-_ ]payload/i,
  /provider[-_ ]prompt/i,
  /wallet[-_ ](?:material|mnemonic|private)/i,
  /customer[-_ ](?:data|private|payload)/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const IDEMPOTENCY_REQUIRED_KINDS: ReadonlySet<AutopilotWorkStructuredEventLogKind> =
  new Set([
    'approval_denied',
    'approval_granted',
    'artifact_created',
    'cancellation',
    'compaction',
    'error',
    'file_edit',
    'receipt_created',
    'shell_execution',
    'status_transition',
    'tool_result',
  ])

const authority: ForgeStructuredEventLogAuthority = {
  acceptedOutcomeAuthority: false,
  deploymentAuthority: false,
  eventAppendAuthority: false,
  eventDeleteAuthority: false,
  eventTailAuthority: false,
  exportGenerationAuthority: false,
  projectionMutationAuthority: false,
  publicClaimAuthority: false,
  replayExecutionAuthority: false,
  retentionDeletionAuthority: false,
  schemaMigrationAuthority: false,
  settlementAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_EVENT_LOG_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-structured-event-log-blocker:${workOrderRef}:${suffix}`

const normalizeEvent = (
  event: AutopilotWorkStructuredEventLogEntry,
): Readonly<{
  event: ForgeStructuredEventLogItem | null
  omittedUnsafeRefCount: number
}> => {
  const actorRefs = safeRefs(event.actorRefs)
  const blockerRefs = safeRefs(event.blockerRefs)
  const correlationRefs = safeRefs(event.correlationRefs)
  const eventRef = safeOptionalRef(event.eventRef)
  const exportRefs = safeRefs(event.exportRefs)
  const idempotencyRefs = safeRefs(event.idempotencyRefs)
  const parentRefs = safeRefs(event.parentRefs)
  const payloadSchemaVersionRefs = safeRefs(event.payloadSchemaVersionRefs)
  const policyRefs = safeRefs(event.policyRefs)
  const projectionRefs = safeRefs(event.projectionRefs)
  const replayRefs = safeRefs(event.replayRefs)
  const retentionRefs = safeRefs(event.retentionRefs)
  const runRefs = safeRefs(event.runRefs)
  const sequenceRef = safeOptionalRef(event.sequenceRef)
  const serviceRefs = safeRefs(event.serviceRefs)
  const subjectRefs = safeRefs(event.subjectRefs)
  const timestampRefs = safeRefs(event.timestampRefs)
  const omittedUnsafeRefCount =
    actorRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    correlationRefs.omittedUnsafeRefCount +
    eventRef.omittedUnsafeRefCount +
    exportRefs.omittedUnsafeRefCount +
    idempotencyRefs.omittedUnsafeRefCount +
    parentRefs.omittedUnsafeRefCount +
    payloadSchemaVersionRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    projectionRefs.omittedUnsafeRefCount +
    replayRefs.omittedUnsafeRefCount +
    retentionRefs.omittedUnsafeRefCount +
    runRefs.omittedUnsafeRefCount +
    sequenceRef.omittedUnsafeRefCount +
    serviceRefs.omittedUnsafeRefCount +
    subjectRefs.omittedUnsafeRefCount +
    timestampRefs.omittedUnsafeRefCount

  return eventRef.ref === null
    ? { event: null, omittedUnsafeRefCount }
    : {
        event: {
          actorRefs: actorRefs.refs,
          blockerRefs: blockerRefs.refs,
          correlationRefs: correlationRefs.refs,
          eventKind: event.eventKind,
          eventRef: eventRef.ref,
          exportRefs: exportRefs.refs,
          freshness: event.freshness ?? 'unknown',
          idempotencyRefs: idempotencyRefs.refs,
          occurredAt: event.occurredAt ?? null,
          parentRefs: parentRefs.refs,
          payloadSchemaVersionRefs: payloadSchemaVersionRefs.refs,
          policyRefs: policyRefs.refs,
          projectionRefs: projectionRefs.refs,
          redactionClass: event.redactionClass,
          replayRefs: replayRefs.refs,
          retentionRefs: retentionRefs.refs,
          runRefs: runRefs.refs,
          sequence: event.sequence,
          sequenceRef: sequenceRef.ref,
          serviceRefs: serviceRefs.refs,
          status: event.status,
          subjectRefs: subjectRefs.refs,
          timestampRefs: timestampRefs.refs,
          visibility: event.visibility,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  events: ReadonlyArray<ForgeStructuredEventLogItem>,
): ForgeStructuredEventLogCounts => ({
  events: events.length,
  failed: events.filter(event => event.status === 'failed').length,
  privateEvents: events.filter(event => event.visibility === 'private').length,
  publicEvents: events.filter(event => event.visibility === 'public').length,
  stale: events.filter(event => event.freshness === 'stale').length,
  teamEvents: events.filter(event => event.visibility === 'team').length,
})

const staleBlockers = (
  workOrderRef: string,
  events: ReadonlyArray<ForgeStructuredEventLogItem>,
): ReadonlyArray<string> =>
  events
    .filter(event => event.freshness === 'stale' && event.blockerRefs.length === 0)
    .map(event =>
      blockerRef(workOrderRef, `stale-event-evidence:${event.eventRef}`),
    )

const duplicateSequenceBlockers = (
  workOrderRef: string,
  events: ReadonlyArray<ForgeStructuredEventLogItem>,
): ReadonlyArray<string> => {
  const seen = new Set<number>()
  const duplicates = events.filter(event => {
    if (seen.has(event.sequence)) {
      return true
    }

    seen.add(event.sequence)
    return false
  })

  return duplicates.map(event =>
    blockerRef(workOrderRef, `duplicate-event-sequence:${event.sequence}`),
  )
}

const sequenceGapBlockers = (
  workOrderRef: string,
  events: ReadonlyArray<ForgeStructuredEventLogItem>,
): ReadonlyArray<string> => {
  const sequences = Array.from(new Set(events.map(event => event.sequence))).sort(
    (left, right) => left - right,
  )
  const gaps = sequences.slice(1).flatMap((sequence, index) => {
    const previous = sequences[index]

    return previous === undefined || sequence === previous + 1
      ? []
      : [`${previous}:${sequence}`]
  })

  return gaps.map(gap =>
    blockerRef(workOrderRef, `event-sequence-gap:${gap}`),
  )
}

const publicRedactionBlockers = (
  workOrderRef: string,
  events: ReadonlyArray<ForgeStructuredEventLogItem>,
): ReadonlyArray<string> =>
  events
    .filter(
      event =>
        event.visibility === 'public' &&
        event.redactionClass !== 'public_safe' &&
        event.blockerRefs.length === 0,
    )
    .map(event =>
      blockerRef(workOrderRef, `public-event-redaction-missing:${event.eventRef}`),
    )

const schemaBlockers = (
  workOrderRef: string,
  events: ReadonlyArray<ForgeStructuredEventLogItem>,
): ReadonlyArray<string> =>
  events
    .filter(
      event =>
        event.payloadSchemaVersionRefs.length === 0 &&
        event.blockerRefs.length === 0,
    )
    .map(event =>
      blockerRef(workOrderRef, `event-schema-version-missing:${event.eventRef}`),
    )

const idempotencyBlockers = (
  workOrderRef: string,
  events: ReadonlyArray<ForgeStructuredEventLogItem>,
): ReadonlyArray<string> =>
  events
    .filter(
      event =>
        IDEMPOTENCY_REQUIRED_KINDS.has(event.eventKind) &&
        event.idempotencyRefs.length === 0 &&
        event.blockerRefs.length === 0,
    )
    .map(event =>
      blockerRef(workOrderRef, `event-idempotency-missing:${event.eventRef}`),
    )

const replayExportPolicyBlockers = (
  workOrderRef: string,
  events: ReadonlyArray<ForgeStructuredEventLogItem>,
): ReadonlyArray<string> =>
  events
    .filter(
      event =>
        event.replayRefs.length + event.exportRefs.length > 0 &&
        event.policyRefs.length === 0 &&
        event.blockerRefs.length === 0,
    )
    .map(event =>
      blockerRef(
        workOrderRef,
        `event-replay-export-policy-missing:${event.eventRef}`,
      ),
    )

const globalReplayExportPolicyBlockers = (
  input: Readonly<{
    exportRefs: ReadonlyArray<string>
    policyRefs: ReadonlyArray<string>
    replayRefs: ReadonlyArray<string>
    workOrderRef: string
  }>,
): ReadonlyArray<string> =>
  input.replayRefs.length + input.exportRefs.length > 0 &&
  input.policyRefs.length === 0
    ? [blockerRef(input.workOrderRef, 'event-log-replay-export-policy-missing')]
    : []

const failedStateBlockers = (
  workOrderRef: string,
  events: ReadonlyArray<ForgeStructuredEventLogItem>,
): ReadonlyArray<string> =>
  events
    .filter(event => event.status === 'failed' && event.blockerRefs.length === 0)
    .map(event =>
      blockerRef(workOrderRef, `failed-event-without-blocker:${event.eventRef}`),
    )

const statusForView = (
  events: ReadonlyArray<ForgeStructuredEventLogItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeStructuredEventLogStatus => {
  if (blockerRefs.length > 0) {
    return 'blocked'
  }

  if (events.length === 0) {
    return 'empty'
  }

  if (events.some(event => event.freshness === 'stale')) {
    return 'stale'
  }

  return events.every(event => event.freshness === 'unknown') ? 'unknown' : 'ready'
}

export const projectForgeStructuredEventLog = (
  input: ForgeStructuredEventLogInput,
): ForgeStructuredEventLogView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const eventStreamRefs = safeRefs(input.eventStreamRefs)
  const replayRefs = safeRefs(input.replayRefs)
  const projectionRefs = safeRefs(input.projectionRefs)
  const exportRefs = safeRefs(input.exportRefs)
  const retentionRefs = safeRefs(input.retentionRefs)
  const policyRefs = safeRefs(input.policyRefs)
  const normalizedEvents = (input.events ?? []).map(normalizeEvent)
  const events = normalizedEvents
    .flatMap(result => (result.event === null ? [] : [result.event]))
    .sort(
      (left, right) =>
        left.sequence - right.sequence || left.eventRef.localeCompare(right.eventRef),
    )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    eventStreamRefs.omittedUnsafeRefCount +
    replayRefs.omittedUnsafeRefCount +
    projectionRefs.omittedUnsafeRefCount +
    exportRefs.omittedUnsafeRefCount +
    retentionRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    normalizedEvents.reduce((sum, result) => sum + result.omittedUnsafeRefCount, 0)
  const hasEntries = (input.events ?? []).length > 0
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...events.flatMap(event => event.blockerRefs),
      ...staleBlockers(input.workOrderRef, events),
      ...duplicateSequenceBlockers(input.workOrderRef, events),
      ...sequenceGapBlockers(input.workOrderRef, events),
      ...publicRedactionBlockers(input.workOrderRef, events),
      ...schemaBlockers(input.workOrderRef, events),
      ...idempotencyBlockers(input.workOrderRef, events),
      ...replayExportPolicyBlockers(input.workOrderRef, events),
      ...globalReplayExportPolicyBlockers({
        exportRefs: exportRefs.refs,
        policyRefs: policyRefs.refs,
        replayRefs: replayRefs.refs,
        workOrderRef: input.workOrderRef,
      }),
      ...failedStateBlockers(input.workOrderRef, events),
      ...(hasEntries && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-structured-event-log-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-structured-event-log-material-omitted')]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    counts: counts(events),
    eventStreamRefs: eventStreamRefs.refs,
    events,
    exportRefs: exportRefs.refs,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    policyRefs: policyRefs.refs,
    projectionRefs: projectionRefs.refs,
    publicSafe: true,
    replayRefs: replayRefs.refs,
    retentionRefs: retentionRefs.refs,
    snapshotRef: snapshotRef.ref,
    status: statusForView(events, blockerRefs),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeStructuredEventLogInput = (
  work: AutopilotWorkProjection,
): ForgeStructuredEventLogInput => {
  const source: AutopilotWorkStructuredEventLog | undefined =
    work.structuredEventLog

  if (source === undefined) {
    return {
      eventStreamRefs: [work.eventStreamRef],
      generatedAt: work.generatedAt,
      workOrderRef: work.workOrderRef,
    }
  }

  return {
    generatedAt: source.generatedAt ?? work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(source.blockerRefs === undefined ? {} : { blockerRefs: source.blockerRefs }),
    ...(source.eventStreamRefs === undefined
      ? { eventStreamRefs: [work.eventStreamRef] }
      : { eventStreamRefs: source.eventStreamRefs }),
    ...(source.events === undefined ? {} : { events: source.events }),
    ...(source.exportRefs === undefined ? {} : { exportRefs: source.exportRefs }),
    ...(source.policyRefs === undefined ? {} : { policyRefs: source.policyRefs }),
    ...(source.projectionRefs === undefined
      ? {}
      : { projectionRefs: source.projectionRefs }),
    ...(source.replayRefs === undefined ? {} : { replayRefs: source.replayRefs }),
    ...(source.retentionRefs === undefined
      ? {}
      : { retentionRefs: source.retentionRefs }),
    snapshotRef: source.snapshotRef,
    ...(source.versionRef === undefined ? {} : { versionRef: source.versionRef }),
  }
}

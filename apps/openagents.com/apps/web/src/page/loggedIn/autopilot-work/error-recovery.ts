import type {
  AutopilotWorkErrorCategory,
  AutopilotWorkErrorRecoveryError,
  AutopilotWorkErrorRecoveryStrategy,
  AutopilotWorkErrorRedactionClass,
  AutopilotWorkErrorRetryability,
  AutopilotWorkErrorSeverity,
  AutopilotWorkEvent,
  AutopilotWorkProjection,
  AutopilotWorkRecoveryEvent,
  AutopilotWorkRecoveryEventKind,
  AutopilotWorkState,
} from '../model'

export type ForgeErrorRecoveryStatus =
  | 'blocked'
  | 'clear'
  | 'failed_closed'
  | 'recovered'
  | 'recovering'

export type ForgeErrorRecoveryAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  automaticRetryAuthority: false
  deploymentAuthority: false
  publicClaimAuthority: false
  runtimeMutationAuthority: false
  settlementAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeErrorRecoveryErrorItem = Readonly<{
  category: AutopilotWorkErrorCategory
  causeRef: string | null
  diagnosticRef: string | null
  errorRef: string
  occurredAt: string | null
  originServiceRef: string | null
  publicMessage: string | null
  recoveryStrategy: AutopilotWorkErrorRecoveryStrategy
  redactionClass: AutopilotWorkErrorRedactionClass
  relatedRefs: ReadonlyArray<string>
  retryability: AutopilotWorkErrorRetryability
  severity: AutopilotWorkErrorSeverity
}>

export type ForgeErrorRecoveryEventItem = Readonly<{
  blockerRefs: ReadonlyArray<string>
  errorRef: string | null
  eventRef: string
  kind: AutopilotWorkRecoveryEventKind
  occurredAt: string
  recoveryStrategy: AutopilotWorkErrorRecoveryStrategy | null
  receiptRefs: ReadonlyArray<string>
}>

export type ForgeErrorRecoveryInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  errors?: ReadonlyArray<AutopilotWorkErrorRecoveryError>
  events?: ReadonlyArray<AutopilotWorkRecoveryEvent>
  generatedAt: string
  recoveryRef?: string
  workOrderRef: string
  workState?: AutopilotWorkState
}>

export type ForgeErrorRecoveryView = Readonly<{
  authority: ForgeErrorRecoveryAuthority
  blockerRefs: ReadonlyArray<string>
  errors: ReadonlyArray<ForgeErrorRecoveryErrorItem>
  events: ReadonlyArray<ForgeErrorRecoveryEventItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  recoveryRef: string
  status: ForgeErrorRecoveryStatus
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

type MessageBundle = Readonly<{
  message: string | null
  omittedUnsafeRefCount: number
}>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_ERROR_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /\bat\s+.+:\d+:\d+\b/i,
  /stack[-_ ]trace/i,
  /raw[-_ ](?:diagnostic|file|log|patch|payload|prompt|provider|shell|source|stack|stderr|stdout|trace|transcript)/i,
  /private[-_ ](?:content|diagnostic|repo|source|workspace)/i,
  /provider[-_ ]payload/i,
  /shell[-_ ](?:log|output|transcript)/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const authority: ForgeErrorRecoveryAuthority = {
  acceptedOutcomeAuthority: false,
  automaticRetryAuthority: false,
  deploymentAuthority: false,
  publicClaimAuthority: false,
  runtimeMutationAuthority: false,
  settlementAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_ERROR_MARKERS.some(marker => marker.test(trimmed))
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

const safePublicMessage = (
  value: string | null | undefined,
): MessageBundle => {
  if (value === null || value === undefined) {
    return { message: null, omittedUnsafeRefCount: 0 }
  }

  const trimmed = value.trim()
  const safe =
    trimmed.length > 0 &&
    trimmed.length <= 220 &&
    !/[\r\n]/.test(trimmed) &&
    !PRIVATE_ERROR_MARKERS.some(marker => marker.test(trimmed))

  return safe
    ? { message: trimmed, omittedUnsafeRefCount: 0 }
    : { message: null, omittedUnsafeRefCount: 1 }
}

const blockerRef = (workOrderRef: string, suffix: string): string =>
  `forge-error-recovery-blocker:${workOrderRef}:${suffix}`

const fallbackErrorRef = (workOrderRef: string, suffix: string): string =>
  `error.public.${workOrderRef}.${suffix}`

const fallbackRecoveryRef = (workOrderRef: string): string =>
  `error-recovery.public.${workOrderRef}.derived`

const fallbackEventRef = (workOrderRef: string, suffix: string): string =>
  `error-recovery-event.public.${workOrderRef}.${suffix}`

const defaultRetryability = (
  category: AutopilotWorkErrorCategory,
): AutopilotWorkErrorRetryability => {
  switch (category) {
    case 'NetworkTransient':
    case 'ProviderOverloaded':
    case 'ProviderRateLimited':
      return 'retryable'
    case 'ApprovalUnavailable':
    case 'ContextTooLarge':
    case 'ExternalAdapterFailed':
    case 'InputInvalid':
    case 'ModelStreamTimeout':
    case 'PermissionDenied':
    case 'ProcessTimeout':
    case 'ResumeConflict':
    case 'ToolExecutionFailed':
    case 'ToolValidationFailed':
      return 'conditional'
    case 'ArtifactWriteFailed':
    case 'ContextAssemblyFailed':
    case 'InternalBug':
    case 'InvariantViolation':
    case 'ModelOutputInvalid':
    case 'ModelRequestFailed':
    case 'NetworkPermanent':
    case 'ProcessKilled':
    case 'ProviderAuthFailed':
    case 'StorageCorrupt':
    case 'StorageReadFailed':
    case 'StorageWriteFailed':
    case 'TaskFailed':
    case 'WorkspaceBoundaryViolation':
      return 'not_retryable'
  }
}

const defaultRecoveryStrategy = (
  category: AutopilotWorkErrorCategory,
): AutopilotWorkErrorRecoveryStrategy => {
  switch (category) {
    case 'ApprovalUnavailable':
    case 'InputInvalid':
    case 'PermissionDenied':
    case 'ProviderAuthFailed':
    case 'ResumeConflict':
      return 'ask_user'
    case 'ContextTooLarge':
      return 'compact_context'
    case 'ExternalAdapterFailed':
    case 'ProviderOverloaded':
      return 'alternate_adapter'
    case 'ModelStreamTimeout':
    case 'NetworkTransient':
    case 'ProviderRateLimited':
      return 'backoff_retry'
    case 'ToolExecutionFailed':
    case 'ToolValidationFailed':
      return 'structured_tool_error'
    case 'ProcessTimeout':
      return 'preserve_partial'
    case 'InternalBug':
    case 'InvariantViolation':
    case 'StorageCorrupt':
    case 'WorkspaceBoundaryViolation':
      return 'stop_fail_closed'
    case 'ArtifactWriteFailed':
    case 'ContextAssemblyFailed':
    case 'ModelOutputInvalid':
    case 'ModelRequestFailed':
    case 'NetworkPermanent':
    case 'ProcessKilled':
    case 'StorageReadFailed':
    case 'StorageWriteFailed':
    case 'TaskFailed':
      return 'none'
  }
}

const defaultSeverity = (
  category: AutopilotWorkErrorCategory,
): AutopilotWorkErrorSeverity =>
  category === 'InternalBug' ||
  category === 'InvariantViolation' ||
  category === 'StorageCorrupt' ||
  category === 'WorkspaceBoundaryViolation'
    ? 'fatal'
    : category === 'ApprovalUnavailable' ||
        category === 'InputInvalid' ||
        category === 'PermissionDenied' ||
        category === 'ProviderRateLimited'
      ? 'warning'
      : 'error'

const normalizeError = (
  error: AutopilotWorkErrorRecoveryError,
): Readonly<{
  error: ForgeErrorRecoveryErrorItem | null
  omittedUnsafeRefCount: number
}> => {
  const errorRef = safeOptionalRef(error.errorRef)
  const causeRef = safeOptionalRef(error.causeRef)
  const diagnosticRef = safeOptionalRef(error.diagnosticRef)
  const originServiceRef = safeOptionalRef(error.originServiceRef)
  const relatedRefs = safeRefs(error.relatedRefs)
  const publicMessage = safePublicMessage(error.publicMessage)
  const omittedUnsafeRefCount =
    errorRef.omittedUnsafeRefCount +
    causeRef.omittedUnsafeRefCount +
    diagnosticRef.omittedUnsafeRefCount +
    originServiceRef.omittedUnsafeRefCount +
    relatedRefs.omittedUnsafeRefCount +
    publicMessage.omittedUnsafeRefCount

  return errorRef.ref === null
    ? { error: null, omittedUnsafeRefCount }
    : {
        error: {
          category: error.category,
          causeRef: causeRef.ref,
          diagnosticRef: diagnosticRef.ref,
          errorRef: errorRef.ref,
          occurredAt: error.occurredAt ?? null,
          originServiceRef: originServiceRef.ref,
          publicMessage: publicMessage.message,
          recoveryStrategy:
            error.recoveryStrategy ?? defaultRecoveryStrategy(error.category),
          redactionClass: error.redactionClass ?? 'private_ref',
          relatedRefs: relatedRefs.refs,
          retryability: error.retryability ?? defaultRetryability(error.category),
          severity: error.severity ?? defaultSeverity(error.category),
        },
        omittedUnsafeRefCount,
      }
}

const normalizeEvent = (
  event: AutopilotWorkRecoveryEvent,
): Readonly<{
  event: ForgeErrorRecoveryEventItem | null
  omittedUnsafeRefCount: number
}> => {
  if (!event.publicSafe) {
    return { event: null, omittedUnsafeRefCount: 1 }
  }

  const eventRef = safeOptionalRef(event.eventRef)
  const errorRef = safeOptionalRef(event.errorRef)
  const blockerRefs = safeRefs(event.blockerRefs)
  const receiptRefs = safeRefs(event.receiptRefs)
  const omittedUnsafeRefCount =
    eventRef.omittedUnsafeRefCount +
    errorRef.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    receiptRefs.omittedUnsafeRefCount

  return eventRef.ref === null
    ? { event: null, omittedUnsafeRefCount }
    : {
        event: {
          blockerRefs: blockerRefs.refs,
          errorRef: errorRef.ref,
          eventRef: eventRef.ref,
          kind: event.kind,
          occurredAt: event.occurredAt,
          recoveryStrategy: event.recoveryStrategy ?? null,
          receiptRefs: receiptRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const errorNeedsFailClosed = (error: ForgeErrorRecoveryErrorItem): boolean =>
  error.recoveryStrategy === 'stop_fail_closed' ||
  error.category === 'InternalBug' ||
  error.category === 'InvariantViolation' ||
  error.category === 'StorageCorrupt' ||
  error.category === 'WorkspaceBoundaryViolation'

const hasIdempotencyEvidence = (error: ForgeErrorRecoveryErrorItem): boolean =>
  error.relatedRefs.some(ref =>
    /^(idem|idempotency|idempotent)[.:/-]/i.test(ref)
  )

const retrySafetyBlockers = (
  workOrderRef: string,
  errors: ReadonlyArray<ForgeErrorRecoveryErrorItem>,
): ReadonlyArray<string> =>
  errors
    .filter(
      error =>
        error.retryability === 'retryable' &&
        error.recoveryStrategy === 'backoff_retry' &&
        !hasIdempotencyEvidence(error),
    )
    .map(error =>
      blockerRef(
        workOrderRef,
        `mutation-retry-safety-unproven:${error.errorRef}`,
      )
    )

const failClosedBlockers = (
  workOrderRef: string,
  errors: ReadonlyArray<ForgeErrorRecoveryErrorItem>,
  events: ReadonlyArray<ForgeErrorRecoveryEventItem>,
): ReadonlyArray<string> =>
  errors.some(errorNeedsFailClosed) ||
  events.some(event => event.kind === 'run.failed_closed')
    ? [blockerRef(workOrderRef, 'terminal-fail-closed')]
    : []

const recoveryStatus = (
  errors: ReadonlyArray<ForgeErrorRecoveryErrorItem>,
  events: ReadonlyArray<ForgeErrorRecoveryEventItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeErrorRecoveryStatus => {
  if (
    blockerRefs.some(ref => ref.endsWith(':terminal-fail-closed')) ||
    events.some(event => event.kind === 'run.failed_closed')
  ) {
    return 'failed_closed'
  }

  if (errors.length === 0 && events.length === 0) {
    return 'clear'
  }

  if (
    blockerRefs.length > 0 ||
    events.some(event => event.kind === 'recovery.failed')
  ) {
    return 'blocked'
  }

  if (events.some(event => event.kind === 'recovery.succeeded')) {
    return 'recovered'
  }

  if (
    events.some(event =>
      [
        'recovery.alternate_adapter_selected',
        'recovery.compaction_attempted',
        'recovery.partial_result_preserved',
        'recovery.retry_scheduled',
        'recovery.started',
        'recovery.user_input_requested',
      ].includes(event.kind)
    )
  ) {
    return 'recovering'
  }

  return 'blocked'
}

export const projectForgeErrorRecovery = (
  input: ForgeErrorRecoveryInput,
): ForgeErrorRecoveryView => {
  const recoveryRef = safeOptionalRef(
    input.recoveryRef ?? fallbackRecoveryRef(input.workOrderRef),
  )
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedErrors = (input.errors ?? []).map(normalizeError)
  const normalizedEvents = (input.events ?? []).map(normalizeEvent)
  const errors = normalizedErrors.flatMap(result =>
    result.error === null ? [] : [result.error]
  )
  const events = normalizedEvents
    .flatMap(result => (result.event === null ? [] : [result.event]))
    .sort((left, right) => left.occurredAt.localeCompare(right.occurredAt))
  const omittedUnsafeRefCount =
    recoveryRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    normalizedErrors.reduce((count, result) => count + result.omittedUnsafeRefCount, 0) +
    normalizedEvents.reduce((count, result) => count + result.omittedUnsafeRefCount, 0)
  const safeRecoveryRef =
    recoveryRef.ref ?? `unsafe-error-recovery.${input.workOrderRef}`
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...events.flatMap(event => event.blockerRefs),
      ...retrySafetyBlockers(input.workOrderRef, errors),
      ...failClosedBlockers(input.workOrderRef, errors, events),
      ...(recoveryRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-recovery-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-error-recovery-material-omitted')]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    errors,
    events,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    recoveryRef: safeRecoveryRef,
    status: recoveryStatus(errors, events, blockerRefs),
    workOrderRef: input.workOrderRef,
  }
}

const relatedRefsForWork = (
  work: AutopilotWorkProjection,
  events: ReadonlyArray<AutopilotWorkEvent> | null | undefined,
): ReadonlyArray<string> => [
  work.clientRequestRef,
  work.eventStreamRef,
  work.statusUrlRef,
  ...work.taskRefs,
  ...work.nextAction.callerActionRefs,
  ...work.nextAction.reasonRefs,
  ...((events ?? []).flatMap(event => [event.eventRef, ...event.taskRefs])),
  ...(work.executionCloseout?.blockerRefs ?? []),
  ...(work.executionCloseout?.closeoutRefs ?? []),
]

const fallbackForState = (
  work: AutopilotWorkProjection,
  events: ReadonlyArray<AutopilotWorkEvent> | null | undefined,
): Pick<ForgeErrorRecoveryInput, 'blockerRefs' | 'errors' | 'events'> => {
  const relatedRefs = relatedRefsForWork(work, events)
  const common = {
    diagnosticRef: null,
    occurredAt: work.updatedAt,
    originServiceRef: 'forge.autopilot.run_projection',
    relatedRefs,
    redactionClass: 'public' as const,
  }
  const missingEvidence = [
    blockerRef(work.workOrderRef, 'missing-error-recovery-evidence'),
  ]

  switch (work.state) {
    case 'invalid':
      return {
        blockerRefs: missingEvidence,
        errors: [
          {
            ...common,
            category: 'InternalBug',
            errorRef: fallbackErrorRef(work.workOrderRef, 'failed_closed'),
            publicMessage: 'Run failed closed.',
            recoveryStrategy: 'stop_fail_closed',
            retryability: 'not_retryable',
            severity: 'fatal',
          },
        ],
        events: [
          {
            errorRef: fallbackErrorRef(work.workOrderRef, 'failed_closed'),
            eventRef: fallbackEventRef(work.workOrderRef, 'failed_closed'),
            kind: 'run.failed_closed',
            occurredAt: work.updatedAt,
            publicSafe: true,
            recoveryStrategy: 'stop_fail_closed',
          },
        ],
      }
    case 'access_required':
      return {
        blockerRefs: missingEvidence,
        errors: [
          {
            ...common,
            category: 'PermissionDenied',
            errorRef: fallbackErrorRef(work.workOrderRef, 'access_required'),
            publicMessage: 'Access or approval is required.',
            recoveryStrategy: 'ask_user',
            retryability: 'conditional',
            severity: 'warning',
          },
        ],
        events: [
          {
            errorRef: fallbackErrorRef(work.workOrderRef, 'access_required'),
            eventRef: fallbackEventRef(work.workOrderRef, 'user_input_requested'),
            kind: 'recovery.user_input_requested',
            occurredAt: work.updatedAt,
            publicSafe: true,
            recoveryStrategy: 'ask_user',
          },
        ],
      }
    case 'blocked':
    case 'payment_required':
    case 'revision_required':
      return {
        blockerRefs: missingEvidence,
        errors: [
          {
            ...common,
            category:
              work.state === 'revision_required'
                ? 'ToolValidationFailed'
                : 'ApprovalUnavailable',
            errorRef: fallbackErrorRef(work.workOrderRef, work.state),
            publicMessage: 'User input or policy approval is required.',
            recoveryStrategy:
              work.state === 'revision_required'
                ? 'structured_tool_error'
                : 'ask_user',
            retryability: 'conditional',
            severity: 'warning',
          },
        ],
        events: [
          {
            errorRef: fallbackErrorRef(work.workOrderRef, work.state),
            eventRef: fallbackEventRef(work.workOrderRef, 'user_input_requested'),
            kind: 'recovery.user_input_requested',
            occurredAt: work.updatedAt,
            publicSafe: true,
            recoveryStrategy:
              work.state === 'revision_required'
                ? 'structured_tool_error'
                : 'ask_user',
          },
        ],
      }
    case 'rejected':
      return {
        blockerRefs: missingEvidence,
        errors: [
          {
            ...common,
            category: 'TaskFailed',
            errorRef: fallbackErrorRef(work.workOrderRef, 'rejected'),
            publicMessage: 'Run did not pass review.',
            recoveryStrategy: 'none',
            retryability: 'not_retryable',
            severity: 'error',
          },
        ],
        events: [
          {
            errorRef: fallbackErrorRef(work.workOrderRef, 'rejected'),
            eventRef: fallbackEventRef(work.workOrderRef, 'failed'),
            kind: 'recovery.failed',
            occurredAt: work.updatedAt,
            publicSafe: true,
          },
        ],
      }
    case 'accepted':
    case 'accepted_free_slice':
    case 'delivered':
    case 'paid_ready':
    case 'queued_or_running':
    case 'scheduled':
      return {}
  }
}

export const buildForgeErrorRecoveryInput = (
  work: AutopilotWorkProjection,
  events: ReadonlyArray<AutopilotWorkEvent> | null | undefined,
): ForgeErrorRecoveryInput => {
  if (work.errorRecovery !== undefined) {
    return {
      generatedAt: work.generatedAt,
      workOrderRef: work.workOrderRef,
      workState: work.state,
      ...(work.errorRecovery.blockerRefs === undefined
        ? {}
        : { blockerRefs: work.errorRecovery.blockerRefs }),
      ...(work.errorRecovery.errors === undefined
        ? {}
        : { errors: work.errorRecovery.errors }),
      ...(work.errorRecovery.events === undefined
        ? {}
        : { events: work.errorRecovery.events }),
      ...(work.errorRecovery.recoveryRef === undefined
        ? {}
        : { recoveryRef: work.errorRecovery.recoveryRef }),
    }
  }

  return {
    generatedAt: work.generatedAt,
    recoveryRef: fallbackRecoveryRef(work.workOrderRef),
    workOrderRef: work.workOrderRef,
    workState: work.state,
    ...fallbackForState(work, events),
  }
}

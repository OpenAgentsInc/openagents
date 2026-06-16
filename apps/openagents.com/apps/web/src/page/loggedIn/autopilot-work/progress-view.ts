import type {
  AutopilotWorkEvent,
  AutopilotWorkEventKind,
  AutopilotWorkProjection,
  AutopilotWorkState,
} from '../model'

export type ForgeRunProgressItemKind =
  | 'accepted'
  | 'blocked'
  | 'closeout'
  | 'delivered'
  | 'failed'
  | 'next_action'
  | 'queued'
  | 'rejected'
  | 'requested'
  | 'revision_required'
  | 'running'
  | 'scheduled'

export type ForgeRunProgressItemStatus =
  | 'active'
  | 'blocked'
  | 'completed'
  | 'failed'
  | 'pending'

export type ForgeRunProgressStatus =
  | 'blocked'
  | 'delivered'
  | 'failed'
  | 'pending'
  | 'reviewed'
  | 'running'

export type ForgeRunProgressItem = Readonly<{
  kind: ForgeRunProgressItemKind
  label: string
  occurredAt: string | null
  refs: ReadonlyArray<string>
  sequence: number
  status: ForgeRunProgressItemStatus
}>

export type ForgeRunProgressView = Readonly<{
  blockerRefs: ReadonlyArray<string>
  items: ReadonlyArray<ForgeRunProgressItem>
  omittedUnsafeRefCount: number
  status: ForgeRunProgressStatus
  workOrderRef: string
}>

type RefBundle = Readonly<{
  omittedUnsafeRefCount: number
  refs: ReadonlyArray<string>
}>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_PROGRESS_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:patch|file|source|shell|command|prompt|log)/i,
  /private[-_ ](?:repo|content|source)/i,
  /provider[-_ ]payload/i,
  /wallet|payment[-_ ](?:material|preimage|hash)/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:bearer|token|secret|mnemonic|preimage|invoice)\b/i,
]

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_PROGRESS_MARKERS.some(marker => marker.test(trimmed))
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

const blockerRef = (workOrderRef: string, suffix: string): string =>
  `forge-run-progress-blocker:${workOrderRef}:${suffix}`

const eventItemKind = (
  eventKind: AutopilotWorkEventKind,
  state: AutopilotWorkState,
): ForgeRunProgressItemKind => {
  if (eventKind === 'blocked' && state === 'invalid') {
    return 'failed'
  }

  return eventKind === 'needs_access' || eventKind === 'payment_required'
    ? 'blocked'
    : eventKind === 'settled'
      ? 'accepted'
      : eventKind
}

const eventItemStatus = (
  eventKind: AutopilotWorkEventKind,
  state: AutopilotWorkState,
): ForgeRunProgressItemStatus => {
  if (state === 'invalid' || eventKind === 'rejected') {
    return 'failed'
  }

  if (
    eventKind === 'blocked' ||
    eventKind === 'needs_access' ||
    eventKind === 'payment_required' ||
    eventKind === 'revision_required'
  ) {
    return 'blocked'
  }

  return eventKind === 'running' || eventKind === 'queued'
    ? 'active'
    : eventKind === 'scheduled'
      ? 'pending'
      : 'completed'
}

const labelForKind = (kind: ForgeRunProgressItemKind): string => {
  switch (kind) {
    case 'accepted':
      return 'Accepted'
    case 'blocked':
      return 'Blocked'
    case 'closeout':
      return 'Closeout evidence'
    case 'delivered':
      return 'Delivered'
    case 'failed':
      return 'Failed'
    case 'next_action':
      return 'Next action'
    case 'queued':
      return 'Queued'
    case 'rejected':
      return 'Rejected'
    case 'requested':
      return 'Requested'
    case 'revision_required':
      return 'Revision required'
    case 'running':
      return 'Running'
    case 'scheduled':
      return 'Scheduled'
  }
}

const terminalItemForState = (
  work: AutopilotWorkProjection,
): ForgeRunProgressItem | null => {
  const status = work.state
  const refs = safeRefs(work.nextAction.callerActionRefs, work.nextAction.reasonRefs)

  switch (status) {
    case 'accepted':
      return {
        kind: 'accepted',
        label: 'Accepted',
        occurredAt: work.updatedAt,
        refs: refs.refs,
        sequence: 900,
        status: 'completed',
      }
    case 'blocked':
    case 'access_required':
    case 'payment_required':
      return {
        kind: 'blocked',
        label: 'Blocked',
        occurredAt: work.updatedAt,
        refs: refs.refs,
        sequence: 900,
        status: 'blocked',
      }
    case 'delivered':
      return {
        kind: 'delivered',
        label: 'Delivered',
        occurredAt: work.updatedAt,
        refs: refs.refs,
        sequence: 900,
        status: 'completed',
      }
    case 'invalid':
      return {
        kind: 'failed',
        label: 'Failed',
        occurredAt: work.updatedAt,
        refs: refs.refs,
        sequence: 900,
        status: 'failed',
      }
    case 'queued_or_running':
    case 'paid_ready':
      return {
        kind: 'running',
        label: 'Running',
        occurredAt: work.updatedAt,
        refs: refs.refs,
        sequence: 900,
        status: 'active',
      }
    case 'rejected':
      return {
        kind: 'rejected',
        label: 'Rejected',
        occurredAt: work.updatedAt,
        refs: refs.refs,
        sequence: 900,
        status: 'failed',
      }
    case 'revision_required':
      return {
        kind: 'revision_required',
        label: 'Revision required',
        occurredAt: work.updatedAt,
        refs: refs.refs,
        sequence: 900,
        status: 'blocked',
      }
    case 'scheduled':
      return {
        kind: 'scheduled',
        label: 'Scheduled',
        occurredAt: work.updatedAt,
        refs: refs.refs,
        sequence: 900,
        status: 'pending',
      }
    case 'accepted_free_slice':
      return {
        kind: 'accepted',
        label: 'Accepted free slice',
        occurredAt: work.updatedAt,
        refs: refs.refs,
        sequence: 900,
        status: 'completed',
      }
  }
}

const progressStatusForWork = (
  work: AutopilotWorkProjection,
  blockerRefs: ReadonlyArray<string>,
): ForgeRunProgressStatus => {
  if (work.state === 'invalid' || work.state === 'rejected') {
    return 'failed'
  }

  if (
    blockerRefs.length > 0 ||
    work.state === 'blocked' ||
    work.state === 'access_required' ||
    work.state === 'payment_required' ||
    work.state === 'revision_required'
  ) {
    return 'blocked'
  }

  if (work.state === 'accepted' || work.state === 'accepted_free_slice') {
    return 'reviewed'
  }

  if (work.state === 'delivered') {
    return 'delivered'
  }

  return work.state === 'scheduled' ? 'pending' : 'running'
}

const closeoutEvidenceAllowed = (state: AutopilotWorkState): boolean =>
  state === 'accepted' ||
  state === 'accepted_free_slice' ||
  state === 'delivered' ||
  state === 'rejected' ||
  state === 'revision_required'

const eventItems = (
  events: ReadonlyArray<AutopilotWorkEvent>,
): Readonly<{ items: ReadonlyArray<ForgeRunProgressItem>; refs: RefBundle }> => {
  const sorted = [...events].sort((a, b) => a.sequence - b.sequence)
  const refs = safeRefs(
    ...sorted.map(event => [event.eventRef, ...event.taskRefs]),
  )

  return {
    refs,
    items: sorted
      .filter(event => event.publicSafe)
      .map(event => {
        const kind = eventItemKind(event.eventKind, event.state)

        return {
          kind,
          label: labelForKind(kind),
          occurredAt: event.occurredAt,
          refs: safeRefs([event.eventRef], event.taskRefs).refs,
          sequence: event.sequence,
          status: eventItemStatus(event.eventKind, event.state),
        }
      }),
  }
}

export const projectForgeRunProgress = (
  work: AutopilotWorkProjection,
  events: ReadonlyArray<AutopilotWorkEvent> | null | undefined,
): ForgeRunProgressView => {
  const requestedRefs = safeRefs(
    [work.clientRequestRef, work.statusUrlRef],
    work.taskRefs,
  )
  const closeoutRefs = safeRefs(
    work.executionCloseout?.closeoutRefs,
    work.executionCloseout?.proofRefs,
    work.executionCloseout?.resultRefs,
  )
  const nextActionRefs = safeRefs(
    work.nextAction.callerActionRefs,
    work.nextAction.reasonRefs,
  )
  const lifecycle = eventItems(events ?? [])
  const terminalItem = terminalItemForState(work)
  const hasLifecycleEvidence = lifecycle.items.length > 0
  const hasTerminalEvidence =
    terminalItem === null ||
    lifecycle.items.some(item => item.kind === terminalItem.kind)
  const showCloseoutEvidence =
    work.executionCloseout !== null && closeoutEvidenceAllowed(work.state)
  const baseItems: ReadonlyArray<ForgeRunProgressItem> = [
    {
      kind: 'requested',
      label: 'Requested',
      occurredAt: work.createdAt,
      refs: requestedRefs.refs,
      sequence: 0,
      status: 'completed',
    },
    ...lifecycle.items,
    ...(terminalItem === null || hasTerminalEvidence ? [] : [terminalItem]),
    ...(!showCloseoutEvidence
      ? []
      : [
          {
            kind: 'closeout' as const,
            label: 'Closeout evidence',
            occurredAt: work.updatedAt,
            refs: closeoutRefs.refs,
            sequence: 950,
            status: 'completed' as const,
          },
        ]),
    {
      kind: 'next_action',
      label: 'Next action',
      occurredAt: null,
      refs: nextActionRefs.refs,
      sequence: 1000,
      status:
        work.state === 'delivered' ||
        work.state === 'accepted' ||
        work.state === 'accepted_free_slice'
          ? 'completed'
          : work.state === 'blocked' ||
              work.state === 'access_required' ||
              work.state === 'payment_required' ||
              work.state === 'revision_required'
            ? 'blocked'
            : work.state === 'invalid' || work.state === 'rejected'
              ? 'failed'
              : work.state === 'scheduled'
                ? 'pending'
                : 'active',
    },
  ]
  const omittedUnsafeRefCount =
    requestedRefs.omittedUnsafeRefCount +
    closeoutRefs.omittedUnsafeRefCount +
    nextActionRefs.omittedUnsafeRefCount +
    lifecycle.refs.omittedUnsafeRefCount
  const blockerRefs = safeRefs(
    [
      ...(hasLifecycleEvidence
        ? []
        : [blockerRef(work.workOrderRef, 'missing-lifecycle-events')]),
      ...(work.executionCloseout === null &&
      ['accepted', 'delivered', 'rejected', 'revision_required'].includes(work.state)
        ? [blockerRef(work.workOrderRef, 'missing-closeout-evidence')]
        : []),
      ...(omittedUnsafeRefCount > 0
        ? [blockerRef(work.workOrderRef, 'unsafe-progress-material-omitted')]
        : []),
    ],
  ).refs

  return {
    blockerRefs,
    items: baseItems,
    omittedUnsafeRefCount,
    status: progressStatusForWork(work, blockerRefs),
    workOrderRef: work.workOrderRef,
  }
}

import { Schema as S } from 'effect'

import {
  PublicProjectionStalenessContract,
  rebuiltOnTransitionStaleness,
} from './public-projection-staleness'

export const PACK_A_RUNTIME_PROJECTION_STALENESS =
  rebuiltOnTransitionStaleness(0, [
    'pack_a_runtime_event_appended',
    'pack_a_task_state_transition',
    'pack_a_schedule_occurrence_transition',
  ])

const PackAEventVisibility = S.Literals(['public', 'operator', 'private'])

const PackAEventSubject = S.Struct({
  kind: S.Literals(['task', 'schedule']),
  ref: S.String,
  sequence: S.Number,
})

const PackAEventKind = S.Literals([
  'task.created',
  'task.started',
  'task.output_appended',
  'task.progress_recorded',
  'task.waiting_for_approval',
  'task.waiting_for_dependency',
  'task.artifact_recorded',
  'task.usage_recorded',
  'task.completed',
  'task.failed',
  'task.cancel_requested',
  'task.cancelled',
  'task.killed',
  'task.expired',
  'task.notification_enqueued',
  'task.notification_delivered',
  'schedule.created',
  'schedule.updated',
  'schedule.paused',
  'schedule.resumed',
  'schedule.deleted',
  'schedule.fired',
  'schedule.skipped',
  'schedule.failed',
  'schedule.cancelled',
  'schedule.continuation_queued',
])
export type PackAEventKind = typeof PackAEventKind.Type

const PackATaskEventPayload = S.Struct({
  taskRef: S.String,
  runRef: S.optional(S.String),
  scheduleRef: S.optional(S.String),
  outputRef: S.optional(S.String),
  artifactRef: S.optional(S.String),
  usageRef: S.optional(S.String),
  notificationRef: S.optional(S.String),
  cursor: S.optional(S.Number),
  truncated: S.optional(S.Boolean),
  redacted: S.optional(S.Boolean),
  terminalState: S.optional(S.Literals([
    'completed',
    'failed',
    'cancelled',
    'killed',
    'expired',
  ])),
  exitStatus: S.optional(S.Literals(['success', 'failed', 'cancelled', 'killed'])),
})

const PackAScheduleEventPayload = S.Struct({
  scheduleRef: S.String,
  occurrenceRef: S.optional(S.String),
  taskRef: S.optional(S.String),
  runRef: S.optional(S.String),
  ownerRef: S.optional(S.String),
  teamRef: S.optional(S.String),
  triggerKind: S.optional(S.Literals(['one_shot', 'recurring', 'continuation'])),
  timezone: S.optional(S.String),
  nextRunAt: S.optional(S.String),
  lastRunAt: S.optional(S.String),
  budgetPolicyRef: S.optional(S.String),
  permissionPolicyRef: S.optional(S.String),
  adapterPreferenceRef: S.optional(S.String),
  notificationPolicyRef: S.optional(S.String),
  retentionPolicyRef: S.optional(S.String),
  blockerRef: S.optional(S.String),
})

export const PackARuntimeEvent = S.Struct({
  schema: S.Literal('openagents.autopilot.pack_a.runtime_event.v1'),
  eventId: S.String,
  kind: PackAEventKind,
  subject: PackAEventSubject,
  generatedAt: S.String,
  visibility: PackAEventVisibility,
  redactionClass: S.Literals([
    'public_ref',
    'redacted_summary',
    'operator_summary',
    'private_ref',
  ]),
  refs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  task: S.optional(PackATaskEventPayload),
  schedule: S.optional(PackAScheduleEventPayload),
  summary: S.optional(S.String),
})
export type PackARuntimeEvent = typeof PackARuntimeEvent.Type

export const PackATaskProjection = S.Struct({
  schema: S.Literal('openagents.autopilot.pack_a.task_projection.v1'),
  taskRef: S.String,
  state: S.Literals([
    'queued',
    'running',
    'waiting',
    'completed',
    'failed',
    'cancelled',
    'killed',
    'expired',
  ]),
  generatedAt: S.String,
  staleness: PublicProjectionStalenessContract,
  eventCount: S.Number,
  runRef: S.optional(S.String),
  scheduleRef: S.optional(S.String),
  outputRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  usageRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  notificationRefs: S.Array(S.String),
  terminalState: S.optional(S.String),
  visibilitySplit: S.Struct({
    storedEventVisibilities: S.Array(S.String),
    projectedVisibility: S.Literal('public'),
  }),
  authority: S.Struct({
    acceptedWorkAuthority: S.Literal(false),
    payoutAuthority: S.Literal(false),
    publicClaimAuthority: S.Literal(false),
  }),
})
export type PackATaskProjection = typeof PackATaskProjection.Type

export const PackAScheduleProjection = S.Struct({
  schema: S.Literal('openagents.autopilot.pack_a.schedule_projection.v1'),
  scheduleRef: S.String,
  state: S.Literals(['active', 'paused', 'deleted', 'cancelled', 'failed']),
  generatedAt: S.String,
  staleness: PublicProjectionStalenessContract,
  eventCount: S.Number,
  ownerRef: S.optional(S.String),
  teamRef: S.optional(S.String),
  nextRunAt: S.optional(S.String),
  lastRunAt: S.optional(S.String),
  occurrenceRefs: S.Array(S.String),
  firedTaskRefs: S.Array(S.String),
  continuationTaskRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  authority: S.Struct({
    createsWorkAuthority: S.Literal(false),
    payoutAuthority: S.Literal(false),
    publicClaimAuthority: S.Literal(false),
  }),
})
export type PackAScheduleProjection = typeof PackAScheduleProjection.Type

export class PackARuntimeEventError extends Error {
  override readonly name = 'PackARuntimeEventError'
}

export interface PackARuntimeEventRepository {
  append(event: PackARuntimeEvent): Promise<void>
  eventsForSubject(
    subjectKind: PackARuntimeEvent['subject']['kind'],
    subjectRef: string,
  ): Promise<ReadonlyArray<PackARuntimeEvent>>
}

export class MemoryPackARuntimeEventRepository implements PackARuntimeEventRepository {
  readonly eventsBySubject = new Map<string, ReadonlyArray<PackARuntimeEvent>>()

  async append(event: PackARuntimeEvent): Promise<void> {
    const key = subjectKey(event.subject.kind, event.subject.ref)
    const previous = this.eventsBySubject.get(key) ?? []
    this.eventsBySubject.set(key, [...previous, event])
  }

  async eventsForSubject(
    subjectKind: PackARuntimeEvent['subject']['kind'],
    subjectRef: string,
  ): Promise<ReadonlyArray<PackARuntimeEvent>> {
    return this.eventsBySubject.get(subjectKey(subjectKind, subjectRef)) ?? []
  }
}

const subjectKey = (
  subjectKind: PackARuntimeEvent['subject']['kind'],
  subjectRef: string,
) => `${subjectKind}:${subjectRef}`

const unsafePublicMaterialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|local[_-]?path|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|key|repo|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|command|customer|email|invoice|log|payment|payload|prompt|provider|record|repo|runner|run[_-]?log|shell|source|state|target|text|trace|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|token[_-]?secret|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed))/i

export const decodePackARuntimeEvent = S.decodeUnknownSync(PackARuntimeEvent)

export const packARuntimeEventHasUnsafeMaterial = (
  event: PackARuntimeEvent,
): boolean =>
  event.visibility === 'public' &&
  unsafePublicMaterialPattern.test(JSON.stringify(event))

export const assertPackARuntimeEventSafe = (
  event: PackARuntimeEvent,
): PackARuntimeEvent => {
  if (packARuntimeEventHasUnsafeMaterial(event)) {
    throw new PackARuntimeEventError(
      'Pack A public runtime event contains raw/private material.',
    )
  }

  return event
}

export const appendPackARuntimeEvent = async (
  repository: PackARuntimeEventRepository,
  value: unknown,
): Promise<PackARuntimeEvent> => {
  const event = assertPackARuntimeEventSafe(decodePackARuntimeEvent(value))
  const existing = await repository.eventsForSubject(
    event.subject.kind,
    event.subject.ref,
  )
  const duplicateEvent = existing.some(candidate => candidate.eventId === event.eventId)

  if (duplicateEvent) {
    throw new PackARuntimeEventError('Pack A runtime event is already persisted.')
  }

  const expectedSequence = existing.length + 1

  if (event.subject.sequence !== expectedSequence) {
    throw new PackARuntimeEventError(
      `Pack A runtime event sequence must append at ${expectedSequence}.`,
    )
  }

  if (event.kind === 'schedule.fired') {
    const occurrenceRef = event.schedule?.occurrenceRef
    const duplicateOccurrence = occurrenceRef !== undefined &&
      existing.some(candidate =>
        candidate.kind === 'schedule.fired' &&
        candidate.schedule?.occurrenceRef === occurrenceRef,
      )

    if (duplicateOccurrence) {
      throw new PackARuntimeEventError(
        'Pack A schedule occurrence already fired.',
      )
    }
  }

  if (event.kind === 'task.notification_delivered') {
    const notificationRef = event.task?.notificationRef
    const duplicateNotification = notificationRef !== undefined &&
      existing.some(candidate =>
        candidate.kind === 'task.notification_delivered' &&
        candidate.task?.notificationRef === notificationRef,
      )

    if (duplicateNotification) {
      throw new PackARuntimeEventError(
        'Pack A task notification was already delivered.',
      )
    }
  }

  await repository.append(event)

  return event
}

export class TaskSupervisor {
  constructor(readonly repository: PackARuntimeEventRepository) {}

  async createTask(input: {
    readonly taskRef: string
    readonly runRef?: string
    readonly scheduleRef?: string
    readonly nowIso: string
    readonly refs?: ReadonlyArray<string>
  }): Promise<PackARuntimeEvent> {
    return this.appendTaskEvent('task.created', input)
  }

  async startTask(input: {
    readonly taskRef: string
    readonly runRef?: string
    readonly nowIso: string
    readonly refs?: ReadonlyArray<string>
  }): Promise<PackARuntimeEvent> {
    return this.appendTaskEvent('task.started', input)
  }

  async appendOutput(input: {
    readonly taskRef: string
    readonly outputRef: string
    readonly cursor: number
    readonly truncated: boolean
    readonly redacted: boolean
    readonly nowIso: string
  }): Promise<PackARuntimeEvent> {
    return this.appendTaskEvent('task.output_appended', input)
  }

  async recordArtifact(input: {
    readonly taskRef: string
    readonly artifactRef: string
    readonly nowIso: string
  }): Promise<PackARuntimeEvent> {
    return this.appendTaskEvent('task.artifact_recorded', input)
  }

  async recordUsage(input: {
    readonly taskRef: string
    readonly usageRef: string
    readonly nowIso: string
  }): Promise<PackARuntimeEvent> {
    return this.appendTaskEvent('task.usage_recorded', input)
  }

  async waitForApproval(input: {
    readonly taskRef: string
    readonly blockerRefs: ReadonlyArray<string>
    readonly nowIso: string
  }): Promise<PackARuntimeEvent> {
    return this.appendTaskEvent('task.waiting_for_approval', input)
  }

  async completeTask(input: {
    readonly taskRef: string
    readonly nowIso: string
    readonly exitStatus?: 'success'
  }): Promise<PackARuntimeEvent> {
    return this.appendTaskEvent('task.completed', {
      ...input,
      terminalState: 'completed',
      exitStatus: input.exitStatus ?? 'success',
    })
  }

  async failTask(input: {
    readonly taskRef: string
    readonly blockerRefs: ReadonlyArray<string>
    readonly nowIso: string
  }): Promise<PackARuntimeEvent> {
    return this.appendTaskEvent('task.failed', {
      ...input,
      terminalState: 'failed',
      exitStatus: 'failed',
    })
  }

  async cancelTask(input: {
    readonly taskRef: string
    readonly nowIso: string
  }): Promise<PackARuntimeEvent> {
    return this.appendTaskEvent('task.cancelled', {
      ...input,
      terminalState: 'cancelled',
      exitStatus: 'cancelled',
    })
  }

  async enqueueNotification(input: {
    readonly taskRef: string
    readonly notificationRef: string
    readonly nowIso: string
  }): Promise<PackARuntimeEvent> {
    return this.appendTaskEvent('task.notification_enqueued', input)
  }

  async deliverNotification(input: {
    readonly taskRef: string
    readonly notificationRef: string
    readonly nowIso: string
  }): Promise<PackARuntimeEvent> {
    return this.appendTaskEvent('task.notification_delivered', input)
  }

  private async appendTaskEvent(
    kind: PackAEventKind,
    input: {
      readonly taskRef: string
      readonly runRef?: string
      readonly scheduleRef?: string
      readonly outputRef?: string
      readonly artifactRef?: string
      readonly usageRef?: string
      readonly notificationRef?: string
      readonly cursor?: number
      readonly truncated?: boolean
      readonly redacted?: boolean
      readonly terminalState?: PackATaskProjection['state']
      readonly exitStatus?: 'success' | 'failed' | 'cancelled' | 'killed'
      readonly blockerRefs?: ReadonlyArray<string>
      readonly refs?: ReadonlyArray<string>
      readonly nowIso: string
    },
  ): Promise<PackARuntimeEvent> {
    const existing = await this.repository.eventsForSubject('task', input.taskRef)
    const sequence = existing.length + 1

    return appendPackARuntimeEvent(this.repository, {
      schema: 'openagents.autopilot.pack_a.runtime_event.v1',
      eventId: `event.public.pack_a.${input.taskRef}.${sequence}`,
      kind,
      subject: {
        kind: 'task',
        ref: input.taskRef,
        sequence,
      },
      generatedAt: input.nowIso,
      visibility: 'public',
      redactionClass: 'public_ref',
      refs: [...(input.refs ?? [])],
      blockerRefs: [...(input.blockerRefs ?? [])],
      task: {
        taskRef: input.taskRef,
        ...(input.runRef === undefined ? {} : { runRef: input.runRef }),
        ...(input.scheduleRef === undefined ? {} : { scheduleRef: input.scheduleRef }),
        ...(input.outputRef === undefined ? {} : { outputRef: input.outputRef }),
        ...(input.artifactRef === undefined ? {} : { artifactRef: input.artifactRef }),
        ...(input.usageRef === undefined ? {} : { usageRef: input.usageRef }),
        ...(input.notificationRef === undefined
          ? {}
          : { notificationRef: input.notificationRef }),
        ...(input.cursor === undefined ? {} : { cursor: input.cursor }),
        ...(input.truncated === undefined ? {} : { truncated: input.truncated }),
        ...(input.redacted === undefined ? {} : { redacted: input.redacted }),
        ...(input.terminalState === undefined
          ? {}
          : { terminalState: input.terminalState }),
        ...(input.exitStatus === undefined ? {} : { exitStatus: input.exitStatus }),
      },
    })
  }
}

export const appendScheduleEvent = async (
  repository: PackARuntimeEventRepository,
  input: {
    readonly kind: Extract<PackAEventKind, `schedule.${string}`>
    readonly scheduleRef: string
    readonly nowIso: string
    readonly occurrenceRef?: string
    readonly taskRef?: string
    readonly runRef?: string
    readonly ownerRef?: string
    readonly teamRef?: string
    readonly nextRunAt?: string
    readonly lastRunAt?: string
    readonly blockerRefs?: ReadonlyArray<string>
    readonly refs?: ReadonlyArray<string>
  },
): Promise<PackARuntimeEvent> => {
  const existing = await repository.eventsForSubject('schedule', input.scheduleRef)
  const sequence = existing.length + 1

  return appendPackARuntimeEvent(repository, {
    schema: 'openagents.autopilot.pack_a.runtime_event.v1',
    eventId: `event.public.pack_a.${input.scheduleRef}.${sequence}`,
    kind: input.kind,
    subject: {
      kind: 'schedule',
      ref: input.scheduleRef,
      sequence,
    },
    generatedAt: input.nowIso,
    visibility: 'public',
    redactionClass: 'public_ref',
    refs: [...(input.refs ?? [])],
    blockerRefs: [...(input.blockerRefs ?? [])],
    schedule: {
      scheduleRef: input.scheduleRef,
      ...(input.occurrenceRef === undefined
        ? {}
        : { occurrenceRef: input.occurrenceRef }),
      ...(input.taskRef === undefined ? {} : { taskRef: input.taskRef }),
      ...(input.runRef === undefined ? {} : { runRef: input.runRef }),
      ...(input.ownerRef === undefined ? {} : { ownerRef: input.ownerRef }),
      ...(input.teamRef === undefined ? {} : { teamRef: input.teamRef }),
      ...(input.nextRunAt === undefined ? {} : { nextRunAt: input.nextRunAt }),
      ...(input.lastRunAt === undefined ? {} : { lastRunAt: input.lastRunAt }),
    },
  })
}

const taskTerminalStates: ReadonlySet<PackATaskProjection['state']> =
  new Set(['completed', 'failed', 'cancelled', 'killed', 'expired'])

const legalTaskTransitions: ReadonlyMap<
  PackATaskProjection['state'],
  ReadonlySet<PackATaskProjection['state']>
> = new Map([
  ['queued', new Set(['running', 'waiting', 'cancelled', 'expired', 'failed'])],
  ['running', new Set(['waiting', 'completed', 'failed', 'cancelled', 'killed'])],
  ['waiting', new Set(['running', 'completed', 'failed', 'cancelled', 'killed'])],
  ['completed', new Set()],
  ['failed', new Set()],
  ['cancelled', new Set()],
  ['killed', new Set()],
  ['expired', new Set()],
])

const taskStateForEvent = (
  kind: PackAEventKind,
): PackATaskProjection['state'] | undefined => {
  if (kind === 'task.created') {
    return 'queued'
  }
  if (kind === 'task.started') {
    return 'running'
  }
  if (
    kind === 'task.waiting_for_approval' ||
    kind === 'task.waiting_for_dependency'
  ) {
    return 'waiting'
  }
  if (kind === 'task.completed') {
    return 'completed'
  }
  if (kind === 'task.failed') {
    return 'failed'
  }
  if (kind === 'task.cancelled') {
    return 'cancelled'
  }
  if (kind === 'task.killed') {
    return 'killed'
  }
  if (kind === 'task.expired') {
    return 'expired'
  }

  return undefined
}

const assertTaskTransition = (
  from: PackATaskProjection['state'],
  to: PackATaskProjection['state'],
): PackATaskProjection['state'] => {
  if (from === to && !taskTerminalStates.has(from)) {
    return to
  }
  if (legalTaskTransitions.get(from)?.has(to) === true) {
    return to
  }

  throw new PackARuntimeEventError(`Illegal Pack A task transition: ${from} -> ${to}`)
}

export const projectPackATask = (
  events: ReadonlyArray<PackARuntimeEvent>,
  nowIso: string,
): PackATaskProjection => {
  const publicEvents = events.filter(event => event.visibility === 'public')
  const taskRef = publicEvents[0]?.task?.taskRef ?? publicEvents[0]?.subject.ref

  if (taskRef === undefined) {
    throw new PackARuntimeEventError('Pack A task projection requires events.')
  }

  const initial = {
    state: 'queued' as PackATaskProjection['state'],
    outputRefs: new Set<string>(),
    artifactRefs: new Set<string>(),
    usageRefs: new Set<string>(),
    blockerRefs: new Set<string>(),
    notificationRefs: new Set<string>(),
    storedEventVisibilities: new Set<string>(),
    runRef: undefined as string | undefined,
    scheduleRef: undefined as string | undefined,
    terminalState: undefined as string | undefined,
  }

  const reduced = events.reduce((accumulator, event) => {
    accumulator.storedEventVisibilities.add(event.visibility)
    return accumulator
  }, initial)

  const replayed = publicEvents.reduce((accumulator, event) => {
    const nextState = taskStateForEvent(event.kind)
    const task = event.task

    if (nextState !== undefined) {
      accumulator.state = assertTaskTransition(accumulator.state, nextState)
    }
    if (task?.runRef !== undefined) {
      accumulator.runRef = task.runRef
    }
    if (task?.scheduleRef !== undefined) {
      accumulator.scheduleRef = task.scheduleRef
    }
    if (task?.outputRef !== undefined) {
      accumulator.outputRefs.add(task.outputRef)
    }
    if (task?.artifactRef !== undefined) {
      accumulator.artifactRefs.add(task.artifactRef)
    }
    if (task?.usageRef !== undefined) {
      accumulator.usageRefs.add(task.usageRef)
    }
    if (task?.notificationRef !== undefined) {
      accumulator.notificationRefs.add(task.notificationRef)
    }
    if (task?.terminalState !== undefined) {
      accumulator.terminalState = task.terminalState
    }
    event.blockerRefs.forEach(ref => accumulator.blockerRefs.add(ref))

    return accumulator
  }, reduced)

  const projection: PackATaskProjection = {
    schema: 'openagents.autopilot.pack_a.task_projection.v1',
    taskRef,
    state: replayed.state,
    generatedAt: nowIso,
    staleness: PACK_A_RUNTIME_PROJECTION_STALENESS,
    eventCount: publicEvents.length,
    ...(replayed.runRef === undefined ? {} : { runRef: replayed.runRef }),
    ...(replayed.scheduleRef === undefined ? {} : { scheduleRef: replayed.scheduleRef }),
    outputRefs: [...replayed.outputRefs],
    artifactRefs: [...replayed.artifactRefs],
    usageRefs: [...replayed.usageRefs],
    blockerRefs: [...replayed.blockerRefs],
    notificationRefs: [...replayed.notificationRefs],
    ...(replayed.terminalState === undefined
      ? {}
      : { terminalState: replayed.terminalState }),
    visibilitySplit: {
      storedEventVisibilities: [...replayed.storedEventVisibilities],
      projectedVisibility: 'public',
    },
    authority: {
      acceptedWorkAuthority: false,
      payoutAuthority: false,
      publicClaimAuthority: false,
    },
  }

  return S.decodeUnknownSync(PackATaskProjection)(projection)
}

export const projectPackASchedule = (
  events: ReadonlyArray<PackARuntimeEvent>,
  nowIso: string,
): PackAScheduleProjection => {
  const publicEvents = events.filter(event => event.visibility === 'public')
  const scheduleRef = publicEvents[0]?.schedule?.scheduleRef ?? publicEvents[0]?.subject.ref

  if (scheduleRef === undefined) {
    throw new PackARuntimeEventError('Pack A schedule projection requires events.')
  }

  const initial = {
    state: 'active' as PackAScheduleProjection['state'],
    occurrenceRefs: new Set<string>(),
    firedTaskRefs: new Set<string>(),
    continuationTaskRefs: new Set<string>(),
    blockerRefs: new Set<string>(),
    receiptRefs: new Set<string>(),
    ownerRef: undefined as string | undefined,
    teamRef: undefined as string | undefined,
    nextRunAt: undefined as string | undefined,
    lastRunAt: undefined as string | undefined,
  }

  const replayed = publicEvents.reduce((accumulator, event) => {
    const schedule = event.schedule

    if (event.kind === 'schedule.paused') {
      accumulator.state = 'paused'
    }
    if (event.kind === 'schedule.resumed' || event.kind === 'schedule.updated') {
      accumulator.state = 'active'
    }
    if (event.kind === 'schedule.deleted') {
      accumulator.state = 'deleted'
    }
    if (event.kind === 'schedule.cancelled') {
      accumulator.state = 'cancelled'
    }
    if (event.kind === 'schedule.failed') {
      accumulator.state = 'failed'
    }
    if (schedule?.ownerRef !== undefined) {
      accumulator.ownerRef = schedule.ownerRef
    }
    if (schedule?.teamRef !== undefined) {
      accumulator.teamRef = schedule.teamRef
    }
    if (schedule?.nextRunAt !== undefined) {
      accumulator.nextRunAt = schedule.nextRunAt
    }
    if (schedule?.lastRunAt !== undefined) {
      accumulator.lastRunAt = schedule.lastRunAt
    }
    if (schedule?.occurrenceRef !== undefined) {
      accumulator.occurrenceRefs.add(schedule.occurrenceRef)
      accumulator.receiptRefs.add(`receipt.public.pack_a.${schedule.occurrenceRef}`)
    }
    if (event.kind === 'schedule.fired' && schedule?.taskRef !== undefined) {
      accumulator.firedTaskRefs.add(schedule.taskRef)
    }
    if (
      event.kind === 'schedule.continuation_queued' &&
      schedule?.taskRef !== undefined
    ) {
      accumulator.continuationTaskRefs.add(schedule.taskRef)
    }
    event.blockerRefs.forEach(ref => accumulator.blockerRefs.add(ref))

    return accumulator
  }, initial)

  const projection: PackAScheduleProjection = {
    schema: 'openagents.autopilot.pack_a.schedule_projection.v1',
    scheduleRef,
    state: replayed.state,
    generatedAt: nowIso,
    staleness: PACK_A_RUNTIME_PROJECTION_STALENESS,
    eventCount: publicEvents.length,
    ...(replayed.ownerRef === undefined ? {} : { ownerRef: replayed.ownerRef }),
    ...(replayed.teamRef === undefined ? {} : { teamRef: replayed.teamRef }),
    ...(replayed.nextRunAt === undefined ? {} : { nextRunAt: replayed.nextRunAt }),
    ...(replayed.lastRunAt === undefined ? {} : { lastRunAt: replayed.lastRunAt }),
    occurrenceRefs: [...replayed.occurrenceRefs],
    firedTaskRefs: [...replayed.firedTaskRefs],
    continuationTaskRefs: [...replayed.continuationTaskRefs],
    blockerRefs: [...replayed.blockerRefs],
    receiptRefs: [...replayed.receiptRefs],
    authority: {
      createsWorkAuthority: false,
      payoutAuthority: false,
      publicClaimAuthority: false,
    },
  }

  return S.decodeUnknownSync(PackAScheduleProjection)(projection)
}

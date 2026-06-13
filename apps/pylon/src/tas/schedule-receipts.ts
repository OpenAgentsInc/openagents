export type ScheduleEventKind = "scheduled" | "fired" | "skipped" | "continued"

type ScheduleEventBase = {
  scheduleId: string
  receiptRef?: string
}

export type ScheduleScheduledEvent = ScheduleEventBase & {
  kind: "scheduled"
  scheduleRef: string
  nextRunRef: string
}

export type ScheduleFiredEvent = ScheduleEventBase & {
  kind: "fired"
  fireKey: string
  runRef: string
  duplicate?: boolean
}

export type ScheduleSkippedEvent = ScheduleEventBase & {
  kind: "skipped"
  fireKey: string
  blockerRef: string
}

export type ScheduleContinuedEvent = ScheduleEventBase & {
  kind: "continued"
  fireKey: string
  continuationRef: string
  runRef: string
}

export type ScheduleEvent =
  | ScheduleScheduledEvent
  | ScheduleFiredEvent
  | ScheduleSkippedEvent
  | ScheduleContinuedEvent

export type ScheduleReceipt = {
  kind: ScheduleEventKind
  scheduleRef: string
  receiptRef: string | null
  fireKeyRef: string | null
  nextRunRef: string | null
  runRef: string | null
  blockerRef: string | null
  continuationRef: string | null
  duplicate: boolean
}

export type ScheduleFiringState = {
  readonly fireKeysByScheduleId: ReadonlyMap<string, ReadonlySet<string>>
}

export type RecordFiringInput = {
  scheduleId: string
  fireKey: string
}

export type RecordFiringResult = {
  state: ScheduleFiringState
  recorded: boolean
  duplicate: boolean
}

export function createScheduleFiringState(): ScheduleFiringState {
  return { fireKeysByScheduleId: new Map() }
}

export function recordFiring(
  state: ScheduleFiringState,
  input: RecordFiringInput,
): RecordFiringResult {
  const scheduleFireKeys = state.fireKeysByScheduleId.get(input.scheduleId)
  if (scheduleFireKeys?.has(input.fireKey)) {
    return {
      state,
      recorded: false,
      duplicate: true,
    }
  }

  const nextScheduleFireKeys = new Set(scheduleFireKeys ?? [])
  nextScheduleFireKeys.add(input.fireKey)

  const fireKeysByScheduleId = new Map(state.fireKeysByScheduleId)
  fireKeysByScheduleId.set(input.scheduleId, nextScheduleFireKeys)

  return {
    state: { fireKeysByScheduleId },
    recorded: true,
    duplicate: false,
  }
}

export function buildScheduleReceipt(event: ScheduleEvent): ScheduleReceipt {
  return {
    kind: event.kind,
    scheduleRef: event.scheduleId,
    receiptRef: event.receiptRef ?? null,
    fireKeyRef: "fireKey" in event ? event.fireKey : null,
    nextRunRef: "nextRunRef" in event ? event.nextRunRef : null,
    runRef: "runRef" in event ? event.runRef : null,
    blockerRef: "blockerRef" in event ? event.blockerRef : null,
    continuationRef: "continuationRef" in event ? event.continuationRef : null,
    duplicate: "duplicate" in event ? event.duplicate === true : false,
  }
}

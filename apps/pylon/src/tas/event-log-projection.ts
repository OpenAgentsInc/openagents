export type ProjectionAudience = "public" | "pylon" | "api" | "companion"

export type StructuredEventKind =
  | "model.delta"
  | "tool.proposed"
  | "tool.result"
  | "approval.requested"
  | "approval.granted"
  | "approval.denied"
  | "file.edited"
  | "shell.executed"
  | "artifact.created"
  | "receipt.created"
  | "status.changed"
  | "error.raised"
  | "run.cancelled"
  | "context.compacted"
  | (string & {})

export type StructuredEvent = Readonly<{
  eventId: string
  sequence: number
  kind: StructuredEventKind
  subjectRef: string
  at: string
  projectionLevel: ProjectionAudience
  detailRef?: string
}>

export type StructuredEventLog = Readonly<{
  events: readonly StructuredEvent[]
  lastSequence: number
  eventIds: ReadonlySet<string>
}>

export type AppendEventResult = Readonly<{
  log: StructuredEventLog
  appended: boolean
  reason: "appended" | "duplicate" | "sequence_gap" | "out_of_order"
}>

export type ProjectedStructuredEvent = Readonly<{
  eventId: string
  sequence: number
  kind: StructuredEventKind
  subjectRef: string
  at: string
  projectionLevel: ProjectionAudience
  detailRef?: string
}>

const AUDIENCE_RANK: Record<ProjectionAudience, number> = {
  public: 0,
  companion: 1,
  api: 2,
  pylon: 3,
}

export function createStructuredEventLog(
  events: readonly StructuredEvent[] = [],
): StructuredEventLog {
  return events.reduce((log, event) => appendStructuredEvent(log, event).log, emptyStructuredEventLog())
}

export function emptyStructuredEventLog(): StructuredEventLog {
  return { events: [], lastSequence: 0, eventIds: new Set() }
}

export function appendStructuredEvent(
  log: StructuredEventLog,
  event: StructuredEvent,
): AppendEventResult {
  if (log.eventIds.has(event.eventId)) {
    return { log, appended: false, reason: "duplicate" }
  }
  if (event.sequence <= log.lastSequence) {
    return { log, appended: false, reason: "out_of_order" }
  }
  if (event.sequence !== log.lastSequence + 1) {
    return { log, appended: false, reason: "sequence_gap" }
  }

  return {
    log: {
      events: [...log.events, event],
      lastSequence: event.sequence,
      eventIds: new Set([...log.eventIds, event.eventId]),
    },
    appended: true,
    reason: "appended",
  }
}

export function projectForAudience(
  events: readonly StructuredEvent[],
  audience: ProjectionAudience,
): readonly ProjectedStructuredEvent[] {
  return [...events]
    .sort((left, right) => left.sequence - right.sequence || left.eventId.localeCompare(right.eventId))
    .filter((event) => isVisibleToAudience(event, audience))
    .map((event) => projectEvent(event, audience))
}

function isVisibleToAudience(event: StructuredEvent, audience: ProjectionAudience): boolean {
  return AUDIENCE_RANK[event.projectionLevel] <= AUDIENCE_RANK[audience]
}

function projectEvent(
  event: StructuredEvent,
  audience: ProjectionAudience,
): ProjectedStructuredEvent {
  const base = {
    eventId: event.eventId,
    sequence: event.sequence,
    kind: event.kind,
    subjectRef: event.subjectRef,
    at: event.at,
    projectionLevel: event.projectionLevel,
  }

  if (audience === "public" || event.detailRef === undefined) {
    return base
  }

  return { ...base, detailRef: event.detailRef }
}

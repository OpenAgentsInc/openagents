import { Schema as S } from "effect"

export const PUBLIC_ACTIVITY_TIMELINE_SCHEMA_VERSION =
  "openagents.public_activity_timeline.v1"

export const PUBLIC_ACTIVITY_TIMELINE_STALENESS_CONTRACT_VERSION =
  "projection_staleness.v1"

export const PublicActivityTimelineEventKind = S.Literals([
  "pylon_registered",
  "pylon_heartbeat",
  "wallet_ready",
  "assignment_ready",
  "window_opened",
  "window_closed",
  "work_claimed",
  "trace_submitted",
  "verification_queued",
  "verification_verified",
  "verification_rejected",
  "khala_inference_served",
  "settlement_recorded",
  "real_bitcoin_moved",
  "forum_topic_created",
  "forum_posted",
  "artanis_tick",
  "capacity_snapshot",
  "projection_gap",
])
export type PublicActivityTimelineEventKind =
  typeof PublicActivityTimelineEventKind.Type

export const publicActivityTimelineEventKinds: ReadonlyArray<PublicActivityTimelineEventKind> = [
  "pylon_registered",
  "pylon_heartbeat",
  "wallet_ready",
  "assignment_ready",
  "window_opened",
  "window_closed",
  "work_claimed",
  "trace_submitted",
  "verification_queued",
  "verification_verified",
  "verification_rejected",
  "khala_inference_served",
  "settlement_recorded",
  "real_bitcoin_moved",
  "forum_topic_created",
  "forum_posted",
  "artanis_tick",
  "capacity_snapshot",
  "projection_gap",
]

export const PublicActivityTimelineSourceKind = S.Literals([
  "pylon_api",
  "pylon_presence",
  "training_window",
  "training_trace",
  "training_verification",
  "inference_receipt",
  "settlement_receipt",
  "forum",
  "artanis",
  "capacity_funnel",
  "projection_gap",
])
export type PublicActivityTimelineSourceKind =
  typeof PublicActivityTimelineSourceKind.Type

export const publicActivityTimelineSourceKinds: ReadonlyArray<PublicActivityTimelineSourceKind> = [
  "pylon_api",
  "pylon_presence",
  "training_window",
  "training_trace",
  "training_verification",
  "inference_receipt",
  "settlement_receipt",
  "forum",
  "artanis",
  "capacity_funnel",
  "projection_gap",
]

export const PublicActivityTimelineSourceLagStatus = S.Literals([
  "current",
  "stale",
  "unavailable",
  "projection_gap",
])
export type PublicActivityTimelineSourceLagStatus =
  typeof PublicActivityTimelineSourceLagStatus.Type

export const PublicActivityTimelineComposition = S.Literals([
  "live_at_read",
  "rebuilt_on_transition",
  "stored_snapshot",
])
export type PublicActivityTimelineComposition =
  typeof PublicActivityTimelineComposition.Type

export const PublicActivityTimelineStaleness = S.Struct({
  composition: PublicActivityTimelineComposition,
  contractVersion: S.Literal(
    PUBLIC_ACTIVITY_TIMELINE_STALENESS_CONTRACT_VERSION,
  ),
  maxStalenessSeconds: S.Number,
  rebuildsOn: S.Array(S.String),
})
export type PublicActivityTimelineStaleness =
  typeof PublicActivityTimelineStaleness.Type

export const PublicActivityTimelineSourceLag = S.Struct({
  sourceKind: PublicActivityTimelineSourceKind,
  status: PublicActivityTimelineSourceLagStatus,
  latestSourceEventAt: S.NullOr(S.String),
  observedAt: S.String,
  lagSeconds: S.NullOr(S.Number),
  maxStalenessSeconds: S.Number,
  sourceRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
})
export type PublicActivityTimelineSourceLag =
  typeof PublicActivityTimelineSourceLag.Type

export const PublicActivityTimelineRange = S.Struct({
  from: S.String,
  to: S.String,
  since: S.NullOr(S.String),
  limit: S.Number,
  filterKinds: S.Array(PublicActivityTimelineEventKind),
})
export type PublicActivityTimelineRange = typeof PublicActivityTimelineRange.Type

export const PublicActivityTimelineEvent = S.Struct({
  eventRef: S.String,
  cursor: S.String,
  ts: S.String,
  kind: PublicActivityTimelineEventKind,
  sourceKind: PublicActivityTimelineSourceKind,
  actorRef: S.optional(S.String),
  targetRef: S.optional(S.String),
  runRef: S.optional(S.String),
  windowRef: S.optional(S.String),
  refs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  amountSats: S.optional(S.Number),
  realBitcoinMoved: S.optional(S.Boolean),
  state: S.optional(S.String),
  text: S.String,
})
export type PublicActivityTimelineEvent = typeof PublicActivityTimelineEvent.Type

export const PublicActivityTimelineEnvelope = S.Struct({
  schemaVersion: S.Literal(PUBLIC_ACTIVITY_TIMELINE_SCHEMA_VERSION),
  generatedAt: S.String,
  staleness: PublicActivityTimelineStaleness,
  nextCursor: S.NullOr(S.String),
  range: S.optional(PublicActivityTimelineRange),
  sourceLag: S.Array(PublicActivityTimelineSourceLag),
  events: S.Array(PublicActivityTimelineEvent),
})
export type PublicActivityTimelineEnvelope =
  typeof PublicActivityTimelineEnvelope.Type

export const publicActivityTimelineLiveAtReadStaleness = (
  rebuildsOn: ReadonlyArray<string>,
): PublicActivityTimelineStaleness => ({
  composition: "live_at_read",
  contractVersion: PUBLIC_ACTIVITY_TIMELINE_STALENESS_CONTRACT_VERSION,
  maxStalenessSeconds: 0,
  rebuildsOn: [...rebuildsOn],
})

export const publicActivityTimelineStoredSnapshotStaleness = (
  maxStalenessSeconds: number,
  rebuildsOn: ReadonlyArray<string>,
): PublicActivityTimelineStaleness => ({
  composition: "stored_snapshot",
  contractVersion: PUBLIC_ACTIVITY_TIMELINE_STALENESS_CONTRACT_VERSION,
  maxStalenessSeconds,
  rebuildsOn: [...rebuildsOn],
})

export const publicActivityTimelineCursorForEvent = (
  event: Pick<PublicActivityTimelineEvent, "eventRef" | "sourceKind" | "ts">,
): string => `${event.ts}:${event.sourceKind}:${event.eventRef}`

export const publicActivityTimelineEventCompare = (
  left: Pick<PublicActivityTimelineEvent, "eventRef" | "sourceKind" | "ts">,
  right: Pick<PublicActivityTimelineEvent, "eventRef" | "sourceKind" | "ts">,
): number =>
  left.ts.localeCompare(right.ts) ||
  left.sourceKind.localeCompare(right.sourceKind) ||
  left.eventRef.localeCompare(right.eventRef)

export const orderPublicActivityTimelineEvents = <
  Event extends Pick<PublicActivityTimelineEvent, "eventRef" | "sourceKind" | "ts">,
>(
  events: ReadonlyArray<Event>,
): ReadonlyArray<Event> => [...events].sort(publicActivityTimelineEventCompare)

const unsafePublicActivityMaterialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer\s+[A-Za-z0-9._-]+|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|local[_-]?path|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|key|repo|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|command|customer|email|invoice|log|payment|payload|prompt|provider|record|repo|runner|run[_-]?log|shell|source|state|target|text|trace|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|token[_-]?secret|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed))/i

export const publicActivityTimelineHasUnsafeMaterial = (value: unknown): boolean =>
  unsafePublicActivityMaterialPattern.test(JSON.stringify(value))

const receiptSourceRefPattern =
  /(^receipt\.|\/receipts\/|nexus-pylon\/receipts\/|settlement\.receipt\.)/i

export const publicActivityTimelineEventHasReceiptSource = (
  event: PublicActivityTimelineEvent,
): boolean => event.sourceRefs.some(ref => receiptSourceRefPattern.test(ref))

export const assertPublicActivityTimelineEventSafe = (
  input: unknown,
): PublicActivityTimelineEvent => {
  const event = decodePublicActivityTimelineEvent(input)

  if (publicActivityTimelineHasUnsafeMaterial(event)) {
    throw new Error("Public activity timeline event contains raw/private material")
  }

  const expectedCursor = publicActivityTimelineCursorForEvent(event)
  if (event.cursor !== expectedCursor) {
    throw new Error(
      `Public activity timeline event cursor mismatch: expected ${expectedCursor}`,
    )
  }

  if (event.sourceRefs.length === 0 && event.blockerRefs.length === 0) {
    throw new Error(
      "Public activity timeline event must carry sourceRefs or blockerRefs",
    )
  }

  if (event.kind === "projection_gap" && event.blockerRefs.length === 0) {
    throw new Error(
      "Public activity timeline projection_gap events must carry blockerRefs",
    )
  }

  if (
    event.kind === "khala_inference_served" &&
    (event.sourceKind !== "inference_receipt" ||
      !publicActivityTimelineEventHasReceiptSource(event))
  ) {
    throw new Error(
      "Public activity timeline khala_inference_served events require an inference receipt source ref",
    )
  }

  if (
    event.realBitcoinMoved === true &&
    !publicActivityTimelineEventHasReceiptSource(event)
  ) {
    throw new Error(
      "Public activity timeline realBitcoinMoved:true requires a public receipt source ref",
    )
  }

  if (
    event.kind === "real_bitcoin_moved" &&
    (event.realBitcoinMoved !== true || !publicActivityTimelineEventHasReceiptSource(event))
  ) {
    throw new Error(
      "Public activity timeline real_bitcoin_moved events require receipt-backed realBitcoinMoved:true",
    )
  }

  return event
}

export const assertPublicActivityTimelineEnvelopeSafe = (
  input: unknown,
): PublicActivityTimelineEnvelope => {
  const envelope = decodePublicActivityTimelineEnvelope(input)

  if (publicActivityTimelineHasUnsafeMaterial(envelope)) {
    throw new Error("Public activity timeline envelope contains raw/private material")
  }

  const ordered = orderPublicActivityTimelineEvents(envelope.events)
  for (let index = 0; index < envelope.events.length; index += 1) {
    if (envelope.events[index]?.cursor !== ordered[index]?.cursor) {
      throw new Error("Public activity timeline events are not cursor ordered")
    }
  }

  for (const event of envelope.events) {
    assertPublicActivityTimelineEventSafe(event)
  }

  for (const lag of envelope.sourceLag) {
    if (
      (lag.status === "stale" ||
        lag.status === "unavailable" ||
        lag.status === "projection_gap") &&
      lag.sourceRefs.length === 0 &&
      lag.blockerRefs.length === 0
    ) {
      throw new Error(
        "Public activity timeline source lag must expose sourceRefs or blockerRefs when not current",
      )
    }
  }

  return envelope
}

export const decodePublicActivityTimelineEvent = S.decodeUnknownSync(
  PublicActivityTimelineEvent,
)
export const decodePublicActivityTimelineEnvelope = S.decodeUnknownSync(
  PublicActivityTimelineEnvelope,
)

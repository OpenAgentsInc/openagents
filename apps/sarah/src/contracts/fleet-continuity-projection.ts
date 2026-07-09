import { Match, Schema } from "effect"

/**
 * FC-3's pure browser projection boundary. The caller supplies observations
 * and a clock value; this module grants no transport or fleet authority.
 * Media and conversation remain independent so a failed video lease cannot
 * end text conversation or remove fleet controls.
 */
export const FC3_PROGRESS_CADENCE_MS = 15_000
export const FC3_FRESHNESS_TIMEOUT_MS = 30_000

export const ConversationObservation = Schema.Union([
  Schema.Struct({ status: Schema.Literal("idle") }),
  Schema.Struct({ status: Schema.Literal("connecting") }),
  Schema.Struct({ status: Schema.Literal("text_live") }),
  Schema.Struct({ status: Schema.Literal("busy") }),
  Schema.Struct({ status: Schema.Literal("reconnecting") }),
  Schema.Struct({ status: Schema.Literal("ended") }),
  Schema.Struct({ status: Schema.Literal("failed") }),
])
export type ConversationObservation = typeof ConversationObservation.Type

const LiveMediaLease = Schema.Struct({
  transportLeaseRef: Schema.NonEmptyString,
  transportExpiresAtMs: Schema.Number,
  lastFrameAtMs: Schema.Number,
})

export const MediaObservation = Schema.Union([
  Schema.Struct({ status: Schema.Literal("not_requested") }),
  Schema.Struct({ status: Schema.Literal("queued") }),
  Schema.Struct({ status: Schema.Literal("connecting") }),
  Schema.Struct({ status: Schema.Literal("live"), lease: LiveMediaLease }),
  Schema.Struct({
    status: Schema.Literal("stale"),
    lastFrameAtMs: Schema.Number,
  }),
  Schema.Struct({ status: Schema.Literal("unavailable") }),
  Schema.Struct({ status: Schema.Literal("evicted") }),
  Schema.Struct({ status: Schema.Literal("ended") }),
])
export type MediaObservation = typeof MediaObservation.Type

export const WorkStreamProgressObservation = Schema.Union([
  Schema.Struct({ status: Schema.Literal("not_started") }),
  Schema.Struct({
    status: Schema.Literal("awaiting_first"),
    workUnitRef: Schema.NonEmptyString,
    startedAtMs: Schema.Number,
  }),
  Schema.Struct({
    status: Schema.Literal("active"),
    workUnitRef: Schema.NonEmptyString,
    lastFreshAtMs: Schema.Number,
  }),
  Schema.Struct({
    status: Schema.Literal("blocked"),
    workUnitRef: Schema.NonEmptyString,
    blockerRef: Schema.NonEmptyString,
  }),
  Schema.Struct({
    status: Schema.Literal("closed"),
    workUnitRef: Schema.NonEmptyString,
  }),
])
export type WorkStreamProgressObservation =
  typeof WorkStreamProgressObservation.Type

export type MediaPresentation =
  | Readonly<{
      status: "not_requested"
      frame: "empty"
      badge: "off"
    }>
  | Readonly<{
      status: "queued"
      frame: "empty"
      badge: "queued"
    }>
  | Readonly<{
      status: "connecting"
      frame: "empty"
      badge: "connecting"
    }>
  | Readonly<{
      status: "live"
      frame: "moving"
      badge: "live"
      lease: Readonly<{
        transportLeaseRef: string
        transportExpiresAtMs: number
        lastFrameAtMs: number
      }>
    }>
  | Readonly<{
      status: "stale"
      frame: "frozen"
      badge: "reconnecting"
      lastFrameAtMs: number
      reason:
        | "upstream_stale"
        | "frame_stale"
        | "transport_lease_expired"
        | "transport_and_frame_stale"
    }>
  | Readonly<{
      status: "unavailable"
      frame: "empty"
      badge: "unavailable"
    }>
  | Readonly<{
      status: "evicted"
      frame: "empty"
      badge: "evicted"
    }>
  | Readonly<{
      status: "ended"
      frame: "empty"
      badge: "ended"
    }>

export type WorkStreamProgressPresentation =
  | Readonly<{ status: "not_started" }>
  | Readonly<{
      status: "waiting"
      workUnitRef: string
      ageMs: number
      staleAtMs: number
    }>
  | Readonly<{
      status: "running"
      workUnitRef: string
      ageMs: number
      nextProgressExpectedAtMs: number
      staleAtMs: number
    }>
  | Readonly<{
      status: "stalled"
      workUnitRef: string
      ageMs: number
      reason: "first_progress_timeout" | "progress_stale"
      reconnect: true
    }>
  | Readonly<{
      status: "blocked"
      workUnitRef: string
      blockerRef: string
    }>
  | Readonly<{
      status: "closed"
      workUnitRef: string
    }>

export type ContinuationPresentation =
  | Readonly<{
      status: "full_media"
      textControl: "available"
      fleetControl: "available"
    }>
  | Readonly<{
      status: "text_continuation_reconnect"
      textControl: "available"
      fleetControl: "available"
      message: "Video reconnecting. Keep working in text."
      action: "reconnect_media"
    }>
  | Readonly<{
      status: "text_only"
      textControl: "available"
      fleetControl: "available"
    }>
  | Readonly<{
      status: "inactive"
      textControl: "unavailable"
      fleetControl: "unavailable"
    }>

export type FleetContinuityProjection = Readonly<{
  conversation: ConversationObservation
  media: MediaPresentation
  continuation: ContinuationPresentation
  progress: WorkStreamProgressPresentation
}>

const elapsedMs = (nowMs: number, observedAtMs: number): number =>
  Math.max(0, nowMs - observedAtMs)

const staleReason = (
  frameFresh: boolean,
  transportFresh: boolean,
): Extract<MediaPresentation, { status: "stale" }>["reason"] => {
  if (!frameFresh && !transportFresh) {
    return "transport_and_frame_stale"
  }
  if (!transportFresh) {
    return "transport_lease_expired"
  }
  return "frame_stale"
}

const projectMedia = (
  media: MediaObservation,
  nowMs: number,
): MediaPresentation =>
  Match.value(media).pipe(
    Match.discriminatorsExhaustive("status")({
      not_requested: (): MediaPresentation => ({
        status: "not_requested",
        frame: "empty",
        badge: "off",
      }),
      queued: (): MediaPresentation => ({
        status: "queued",
        frame: "empty",
        badge: "queued",
      }),
      connecting: (): MediaPresentation => ({
        status: "connecting",
        frame: "empty",
        badge: "connecting",
      }),
      live: (liveMedia): MediaPresentation => {
        const frameFresh =
          elapsedMs(nowMs, liveMedia.lease.lastFrameAtMs) <
          FC3_FRESHNESS_TIMEOUT_MS
        const transportFresh = liveMedia.lease.transportExpiresAtMs > nowMs
        if (frameFresh && transportFresh) {
          return {
            status: "live",
            frame: "moving",
            badge: "live",
            lease: liveMedia.lease,
          }
        }
        return {
          status: "stale",
          frame: "frozen",
          badge: "reconnecting",
          lastFrameAtMs: liveMedia.lease.lastFrameAtMs,
          reason: staleReason(frameFresh, transportFresh),
        }
      },
      stale: (staleMedia): MediaPresentation => ({
        status: "stale",
        frame: "frozen",
        badge: "reconnecting",
        lastFrameAtMs: staleMedia.lastFrameAtMs,
        reason: "upstream_stale",
      }),
      unavailable: (): MediaPresentation => ({
        status: "unavailable",
        frame: "empty",
        badge: "unavailable",
      }),
      evicted: (): MediaPresentation => ({
        status: "evicted",
        frame: "empty",
        badge: "evicted",
      }),
      ended: (): MediaPresentation => ({
        status: "ended",
        frame: "empty",
        badge: "ended",
      }),
    }),
  )

const projectProgress = (
  progress: WorkStreamProgressObservation,
  nowMs: number,
): WorkStreamProgressPresentation =>
  Match.value(progress).pipe(
    Match.discriminatorsExhaustive("status")({
      not_started: (): WorkStreamProgressPresentation => ({
        status: "not_started",
      }),
      awaiting_first: (awaitingProgress): WorkStreamProgressPresentation => {
        const ageMs = elapsedMs(nowMs, awaitingProgress.startedAtMs)
        if (ageMs >= FC3_FRESHNESS_TIMEOUT_MS) {
          return {
            status: "stalled",
            workUnitRef: awaitingProgress.workUnitRef,
            ageMs,
            reason: "first_progress_timeout",
            reconnect: true,
          }
        }
        return {
          status: "waiting",
          workUnitRef: awaitingProgress.workUnitRef,
          ageMs,
          staleAtMs: awaitingProgress.startedAtMs + FC3_FRESHNESS_TIMEOUT_MS,
        }
      },
      active: (activeProgress): WorkStreamProgressPresentation => {
        const ageMs = elapsedMs(nowMs, activeProgress.lastFreshAtMs)
        if (ageMs >= FC3_FRESHNESS_TIMEOUT_MS) {
          return {
            status: "stalled",
            workUnitRef: activeProgress.workUnitRef,
            ageMs,
            reason: "progress_stale",
            reconnect: true,
          }
        }
        return {
          status: "running",
          workUnitRef: activeProgress.workUnitRef,
          ageMs,
          nextProgressExpectedAtMs:
            activeProgress.lastFreshAtMs + FC3_PROGRESS_CADENCE_MS,
          staleAtMs:
            activeProgress.lastFreshAtMs + FC3_FRESHNESS_TIMEOUT_MS,
        }
      },
      blocked: (blockedProgress): WorkStreamProgressPresentation => ({
        status: "blocked",
        workUnitRef: blockedProgress.workUnitRef,
        blockerRef: blockedProgress.blockerRef,
      }),
      closed: (closedProgress): WorkStreamProgressPresentation => ({
        status: "closed",
        workUnitRef: closedProgress.workUnitRef,
      }),
    }),
  )

const conversationKeepsControls = (
  conversation: ConversationObservation,
): boolean =>
  conversation.status === "text_live" ||
  conversation.status === "busy" ||
  conversation.status === "reconnecting"

const projectContinuation = (
  conversation: ConversationObservation,
  media: MediaPresentation,
): ContinuationPresentation => {
  if (media.status === "live" && conversationKeepsControls(conversation)) {
    return {
      status: "full_media",
      textControl: "available",
      fleetControl: "available",
    }
  }
  if (media.status === "stale" && conversation.status === "text_live") {
    return {
      status: "text_continuation_reconnect",
      textControl: "available",
      fleetControl: "available",
      message: "Video reconnecting. Keep working in text.",
      action: "reconnect_media",
    }
  }
  if (conversationKeepsControls(conversation)) {
    return {
      status: "text_only",
      textControl: "available",
      fleetControl: "available",
    }
  }
  return {
    status: "inactive",
    textControl: "unavailable",
    fleetControl: "unavailable",
  }
}

export function fleetContinuityProjection(
  input: Readonly<{
    conversation: ConversationObservation
    media: MediaObservation
    progress: WorkStreamProgressObservation
  }>,
  nowMs: number,
): FleetContinuityProjection {
  const media = projectMedia(input.media, nowMs)
  return {
    conversation: input.conversation,
    media,
    continuation: projectContinuation(input.conversation, media),
    progress: projectProgress(input.progress, nowMs),
  }
}

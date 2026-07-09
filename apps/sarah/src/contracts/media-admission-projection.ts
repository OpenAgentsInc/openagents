import { Match, Schema } from "effect"

import {
  ConversationObservation,
  MediaObservation,
  WorkStreamProgressObservation,
  fleetContinuityProjection,
  type FleetContinuityProjection,
} from "./fleet-continuity-projection.ts"

/**
 * A realtime slot may improve the conversation, but it may not hold the user
 * in an indefinite queue. At this boundary every longer deadline is clamped.
 */
export const SARAH_MEDIA_MAX_QUEUE_WAIT_MS = 30_000

const NonNegativeInteger = Schema.Number.check(
  Schema.isFinite(),
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)
const PositiveInteger = Schema.Number.check(
  Schema.isFinite(),
  Schema.isInt(),
  Schema.isGreaterThan(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)
const NonNegativeNumber = Schema.Number.check(
  Schema.isFinite(),
  Schema.isGreaterThanOrEqualTo(0),
)
const MediaPublicRef = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)

const CostNotMeasured = Schema.Struct({
  measurementStatus: Schema.Literal("not_measured"),
  costClass: Schema.Literal("not_measured"),
  marginalCostPerActiveMinuteUsd: Schema.Null,
})

export const MediaAdmissionCostObservation = Schema.Struct({
  preRendered: Schema.Union([
    CostNotMeasured,
    Schema.Struct({
      measurementStatus: Schema.Literal("reported"),
      costClass: Schema.Literal("amortized_pre_rendered"),
      marginalCostPerActiveMinuteUsd: NonNegativeNumber,
    }),
  ]),
  realtime: Schema.Union([
    CostNotMeasured,
    Schema.Struct({
      measurementStatus: Schema.Literal("reported"),
      costClass: Schema.Literal("metered_realtime"),
      marginalCostPerActiveMinuteUsd: NonNegativeNumber,
    }),
  ]),
  offlineOnly: Schema.Union([
    CostNotMeasured,
    Schema.Struct({
      measurementStatus: Schema.Literal("reported"),
      costClass: Schema.Literal("offline_batch"),
      marginalCostPerActiveMinuteUsd: NonNegativeNumber,
    }),
  ]),
})
export type MediaAdmissionCostObservation =
  typeof MediaAdmissionCostObservation.Type

export const PreRenderedMediaObservation = Schema.Union([
  Schema.Struct({ status: Schema.Literal("not_available") }),
  Schema.Struct({
    status: Schema.Literal("available"),
    takeRef: MediaPublicRef,
    source: Schema.Literals(["opener", "semantic_cache"]),
  }),
  Schema.Struct({
    status: Schema.Literal("unavailable"),
    reason: Schema.Literals([
      "asset_missing",
      "playback_failed",
      "policy_blocked",
    ]),
  }),
])
export type PreRenderedMediaObservation =
  typeof PreRenderedMediaObservation.Type

const RealtimeQueueFields = {
  requestRef: MediaPublicRef,
  requestedAtMs: NonNegativeInteger,
  deadlineAtMs: NonNegativeInteger,
} as const

export const RealtimeVideoAdmissionObservation = Schema.Union([
  Schema.Struct({
    status: Schema.Literal("available"),
    ...RealtimeQueueFields,
    admittedAtMs: NonNegativeInteger,
    admissionLeaseRef: MediaPublicRef,
    admissionLeaseExpiresAtMs: NonNegativeInteger,
  }),
  Schema.Struct({
    status: Schema.Literal("queued"),
    ...RealtimeQueueFields,
    queuePosition: Schema.NullOr(PositiveInteger),
  }),
  Schema.Struct({
    status: Schema.Literal("text_only"),
    reason: Schema.Literals([
      "not_requested",
      "user_selected",
      "media_failed",
    ]),
    switchedAtMs: NonNegativeInteger,
  }),
  Schema.Struct({
    status: Schema.Literal("unavailable"),
    reason: Schema.Literals([
      "not_armed",
      "capacity_unavailable",
      "policy_blocked",
      "provider_failed",
    ]),
    observedAtMs: NonNegativeInteger,
  }),
])
export type RealtimeVideoAdmissionObservation =
  typeof RealtimeVideoAdmissionObservation.Type

export const MediaRecoveryObservation = Schema.Union([
  Schema.Struct({ status: Schema.Literal("not_reported") }),
  Schema.Struct({
    status: Schema.Literal("reported"),
    attemptCount: NonNegativeInteger,
    recoveredCount: NonNegativeInteger,
    abandonedCount: NonNegativeInteger,
    lastRecoveredAtMs: Schema.NullOr(NonNegativeInteger),
  }),
])
export type MediaRecoveryObservation = typeof MediaRecoveryObservation.Type

export const MediaAdmissionProjectionInput = Schema.Struct({
  continuity: Schema.Struct({
    conversation: ConversationObservation,
    media: MediaObservation,
    progress: WorkStreamProgressObservation,
  }),
  preRendered: PreRenderedMediaObservation,
  realtime: RealtimeVideoAdmissionObservation,
  costs: MediaAdmissionCostObservation,
  recovery: MediaRecoveryObservation,
})
export type MediaAdmissionProjectionInput =
  typeof MediaAdmissionProjectionInput.Type

export type SarahMediaAdmissionProjectionErrorCode =
  | "sarah_media_admission_invalid_clock"
  | "sarah_media_admission_invalid_input"
  | "sarah_media_admission_invalid_temporal_order"
  | "sarah_media_admission_invalid_recovery_counters"

/** Fixed public-safe failure: never includes rejected values or schema text. */
export class SarahMediaAdmissionProjectionError extends Error {
  readonly code: SarahMediaAdmissionProjectionErrorCode

  constructor(code: SarahMediaAdmissionProjectionErrorCode) {
    super(code)
    this.name = "SarahMediaAdmissionProjectionError"
    this.code = code
  }
}

export type PreRenderedMediaPresentation =
  | Readonly<{
      status: "not_available"
      inputPolicy: "never_blocks_text"
      inputDelayMs: 0
    }>
  | Readonly<{
      status: "available"
      takeRef: string
      source: "opener" | "semantic_cache"
      inputPolicy: "never_blocks_text"
      inputDelayMs: 0
    }>
  | Readonly<{
      status: "unavailable"
      reason: "asset_missing" | "playback_failed" | "policy_blocked"
      inputPolicy: "never_blocks_text"
      inputDelayMs: 0
    }>

type RealtimeTextOnlyReason =
  | "not_requested"
  | "user_selected"
  | "media_failed"
  | "queue_expired"
  | "admission_lease_expired"

export type RealtimeVideoAdmissionPresentation =
  | Readonly<{
      status: "available"
      requestRef: string
      queueWaitMs: number
      queueDeadlineAtMs: number
      reservation: "leased"
      lease: Readonly<{
        admissionLeaseRef: string
        expiresAtMs: number
        remainingMs: number
      }>
    }>
  | Readonly<{
      status: "queued"
      requestRef: string
      queueWaitMs: number
      queueDeadlineAtMs: number
      queuePosition: number | null
      expiresTo: "text_only"
      reservation: "none"
      lease: null
    }>
  | Readonly<{
      status: "text_only"
      reason: RealtimeTextOnlyReason
      requestRef: string | null
      queueWaitMs: number | null
      queueDeadlineAtMs: number | null
      switchedAtMs: number
      reservation: "none"
      lease: null
    }>
  | Readonly<{
      status: "unavailable"
      reason:
        | "not_armed"
        | "capacity_unavailable"
        | "policy_blocked"
        | "provider_failed"
      observedAtMs: number
      queueWaitMs: null
      queueDeadlineAtMs: null
      reservation: "none"
      lease: null
    }>

export type MediaRecoveryPresentation = Readonly<{
  state:
    | "not_needed"
    | "waiting_for_admission"
    | "reconnecting"
    | "continued_in_text"
    | "recovered"
  action: "none" | "continue_in_text" | "reconnect_media"
  telemetry: MediaRecoveryObservation
}>

export type MediaAdmissionProjection = Readonly<{
  schema: "sarah.media_admission_projection.v1"
  text: Readonly<{
    floor: "text"
    delayedByMedia: false
    textControl: "available" | "unavailable"
    fleetControl: "available" | "unavailable"
  }>
  preRendered: PreRenderedMediaPresentation
  realtime: RealtimeVideoAdmissionPresentation
  continuity: FleetContinuityProjection
  telemetry: Readonly<{
    costs: MediaAdmissionCostObservation
    queue: Readonly<{
      requestRef: string | null
      waitMs: number | null
      deadlineAtMs: number | null
      expired: boolean
    }>
    leases: Readonly<{
      admission: Extract<
        RealtimeVideoAdmissionPresentation,
        { status: "available" }
      >["lease"] | null
      transport: Readonly<{
        transportLeaseRef: string
        expiresAtMs: number
        lastFrameAtMs: number
        state: "fresh" | "stale"
      }> | null
    }>
    recovery: MediaRecoveryPresentation
    textFallback: boolean
  }>
}>

const elapsedMs = (nowMs: number, observedAtMs: number): number =>
  Math.max(0, nowMs - observedAtMs)

const boundedQueueDeadlineAtMs = (
  requestedAtMs: number,
  deadlineAtMs: number,
): number =>
  Math.min(
    deadlineAtMs,
    requestedAtMs + SARAH_MEDIA_MAX_QUEUE_WAIT_MS,
  )

const projectPreRendered = (
  observation: PreRenderedMediaObservation,
): PreRenderedMediaPresentation =>
  Match.value(observation).pipe(
    Match.discriminatorsExhaustive("status")({
      not_available: (): PreRenderedMediaPresentation => ({
        status: "not_available",
        inputPolicy: "never_blocks_text",
        inputDelayMs: 0,
      }),
      available: (available): PreRenderedMediaPresentation => ({
        status: "available",
        takeRef: available.takeRef,
        source: available.source,
        inputPolicy: "never_blocks_text",
        inputDelayMs: 0,
      }),
      unavailable: (unavailable): PreRenderedMediaPresentation => ({
        status: "unavailable",
        reason: unavailable.reason,
        inputPolicy: "never_blocks_text",
        inputDelayMs: 0,
      }),
    }),
  )

const queueExpiredPresentation = (
  realtime: Extract<
    RealtimeVideoAdmissionObservation,
    { status: "available" | "queued" }
  >,
  nowMs: number,
  reason: "queue_expired" | "admission_lease_expired",
): Extract<RealtimeVideoAdmissionPresentation, { status: "text_only" }> => {
  const queueDeadlineAtMs = boundedQueueDeadlineAtMs(
    realtime.requestedAtMs,
    realtime.deadlineAtMs,
  )
  return {
    status: "text_only",
    reason,
    requestRef: realtime.requestRef,
    // An available observation has a real admission timestamp. Preserve that
    // actual wait even when a late admission violated the bounded UI deadline;
    // queued-only observations have no admission truth and stop at the bound.
    queueWaitMs:
      realtime.status === "available"
        ? elapsedMs(realtime.admittedAtMs, realtime.requestedAtMs)
        : Math.min(
            elapsedMs(nowMs, realtime.requestedAtMs),
            Math.max(0, queueDeadlineAtMs - realtime.requestedAtMs),
          ),
    queueDeadlineAtMs,
    switchedAtMs:
      reason === "queue_expired"
        ? queueDeadlineAtMs
        : "admissionLeaseExpiresAtMs" in realtime
          ? realtime.admissionLeaseExpiresAtMs
          : nowMs,
    reservation: "none",
    lease: null,
  }
}

const projectRealtime = (
  realtime: RealtimeVideoAdmissionObservation,
  nowMs: number,
): RealtimeVideoAdmissionPresentation =>
  Match.value(realtime).pipe(
    Match.discriminatorsExhaustive("status")({
      available: (available): RealtimeVideoAdmissionPresentation => {
        const queueDeadlineAtMs = boundedQueueDeadlineAtMs(
          available.requestedAtMs,
          available.deadlineAtMs,
        )
        if (available.admittedAtMs >= queueDeadlineAtMs) {
          return queueExpiredPresentation(
            available,
            nowMs,
            "queue_expired",
          )
        }
        if (available.admissionLeaseExpiresAtMs <= nowMs) {
          return queueExpiredPresentation(
            available,
            nowMs,
            "admission_lease_expired",
          )
        }
        return {
          status: "available",
          requestRef: available.requestRef,
          queueWaitMs: Math.min(
            elapsedMs(available.admittedAtMs, available.requestedAtMs),
            SARAH_MEDIA_MAX_QUEUE_WAIT_MS,
          ),
          queueDeadlineAtMs,
          reservation: "leased",
          lease: {
            admissionLeaseRef: available.admissionLeaseRef,
            expiresAtMs: available.admissionLeaseExpiresAtMs,
            remainingMs: available.admissionLeaseExpiresAtMs - nowMs,
          },
        }
      },
      queued: (queued): RealtimeVideoAdmissionPresentation => {
        const queueDeadlineAtMs = boundedQueueDeadlineAtMs(
          queued.requestedAtMs,
          queued.deadlineAtMs,
        )
        if (nowMs >= queueDeadlineAtMs) {
          return queueExpiredPresentation(queued, nowMs, "queue_expired")
        }
        return {
          status: "queued",
          requestRef: queued.requestRef,
          queueWaitMs: elapsedMs(nowMs, queued.requestedAtMs),
          queueDeadlineAtMs,
          queuePosition: queued.queuePosition,
          expiresTo: "text_only",
          reservation: "none",
          lease: null,
        }
      },
      text_only: (textOnly): RealtimeVideoAdmissionPresentation => ({
        status: "text_only",
        reason: textOnly.reason,
        requestRef: null,
        queueWaitMs: null,
        queueDeadlineAtMs: null,
        switchedAtMs: textOnly.switchedAtMs,
        reservation: "none",
        lease: null,
      }),
      unavailable: (unavailable): RealtimeVideoAdmissionPresentation => ({
        status: "unavailable",
        reason: unavailable.reason,
        observedAtMs: unavailable.observedAtMs,
        queueWaitMs: null,
        queueDeadlineAtMs: null,
        reservation: "none",
        lease: null,
      }),
    }),
  )

const mediaObservationForAdmission = (
  media: MediaObservation,
  realtime: RealtimeVideoAdmissionPresentation,
): MediaObservation => {
  if (realtime.status === "available") return media
  if (realtime.status === "queued") return { status: "queued" }
  if (realtime.status === "unavailable") return { status: "unavailable" }
  return { status: "not_requested" }
}

const projectRecovery = (
  realtime: RealtimeVideoAdmissionPresentation,
  continuity: FleetContinuityProjection,
  telemetry: MediaRecoveryObservation,
): MediaRecoveryPresentation => {
  if (continuity.continuation.status === "text_continuation_reconnect") {
    return {
      state: "reconnecting",
      action: "reconnect_media",
      telemetry,
    }
  }
  if (realtime.status === "queued") {
    return {
      state: "waiting_for_admission",
      action: "continue_in_text",
      telemetry,
    }
  }
  if (realtime.status === "text_only" || realtime.status === "unavailable") {
    if (
      realtime.status === "text_only" &&
      (realtime.reason === "not_requested" ||
        realtime.reason === "user_selected")
    ) {
      return { state: "not_needed", action: "none", telemetry }
    }
    return {
      state: "continued_in_text",
      action: "none",
      telemetry,
    }
  }
  if (
    telemetry.status === "reported" &&
    telemetry.recoveredCount > 0 &&
    telemetry.lastRecoveredAtMs !== null
  ) {
    return { state: "recovered", action: "none", telemetry }
  }
  return { state: "not_needed", action: "none", telemetry }
}

const failProjection = (
  code: SarahMediaAdmissionProjectionErrorCode,
): never => {
  throw new SarahMediaAdmissionProjectionError(code)
}

const decodeProjectionClock = (nowMs: unknown): number => {
  try {
    return Schema.decodeUnknownSync(NonNegativeInteger)(nowMs)
  } catch {
    return failProjection("sarah_media_admission_invalid_clock")
  }
}

const decodeProjectionInput = (
  input: unknown,
): MediaAdmissionProjectionInput => {
  try {
    return Schema.decodeUnknownSync(MediaAdmissionProjectionInput)(input)
  } catch {
    return failProjection("sarah_media_admission_invalid_input")
  }
}

const isBoundedTimestamp = (value: number): boolean =>
  Number.isFinite(value) &&
  Number.isSafeInteger(value) &&
  value >= 0

const assertTemporalCoherence = (
  input: MediaAdmissionProjectionInput,
  nowMs: number,
): void => {
  const realtime = input.realtime
  if (realtime.status === "available") {
    if (
      realtime.requestedAtMs > nowMs ||
      realtime.deadlineAtMs < realtime.requestedAtMs ||
      realtime.admittedAtMs < realtime.requestedAtMs ||
      realtime.admittedAtMs > nowMs ||
      realtime.admissionLeaseExpiresAtMs <= realtime.admittedAtMs
    ) {
      failProjection("sarah_media_admission_invalid_temporal_order")
    }
  } else if (realtime.status === "queued") {
    if (
      realtime.requestedAtMs > nowMs ||
      realtime.deadlineAtMs < realtime.requestedAtMs
    ) {
      failProjection("sarah_media_admission_invalid_temporal_order")
    }
  } else if (
    (realtime.status === "text_only" && realtime.switchedAtMs > nowMs) ||
    (realtime.status === "unavailable" && realtime.observedAtMs > nowMs)
  ) {
    failProjection("sarah_media_admission_invalid_temporal_order")
  }

  const media = input.continuity.media
  if (
    (media.status === "live" &&
      (!isBoundedTimestamp(media.lease.transportExpiresAtMs) ||
        !isBoundedTimestamp(media.lease.lastFrameAtMs) ||
        media.lease.lastFrameAtMs > nowMs ||
        media.lease.transportExpiresAtMs < media.lease.lastFrameAtMs)) ||
    (media.status === "stale" &&
      (!isBoundedTimestamp(media.lastFrameAtMs) ||
        media.lastFrameAtMs > nowMs))
  ) {
    failProjection("sarah_media_admission_invalid_temporal_order")
  }

  const progress = input.continuity.progress
  if (
    (progress.status === "awaiting_first" &&
      (!isBoundedTimestamp(progress.startedAtMs) ||
        progress.startedAtMs > nowMs)) ||
    (progress.status === "active" &&
      (!isBoundedTimestamp(progress.lastFreshAtMs) ||
        progress.lastFreshAtMs > nowMs))
  ) {
    failProjection("sarah_media_admission_invalid_temporal_order")
  }

  if (
    input.recovery.status === "reported" &&
    input.recovery.lastRecoveredAtMs !== null &&
    input.recovery.lastRecoveredAtMs > nowMs
  ) {
    failProjection("sarah_media_admission_invalid_temporal_order")
  }
}

const assertRecoveryCoherence = (
  recovery: MediaRecoveryObservation,
): void => {
  if (recovery.status === "not_reported") return
  const countersOverflowAttempts =
    recovery.abandonedCount > recovery.attemptCount ||
    recovery.recoveredCount >
      recovery.attemptCount - recovery.abandonedCount
  const recoveredTimestampContradictsCount =
    (recovery.recoveredCount === 0) !==
    (recovery.lastRecoveredAtMs === null)
  if (countersOverflowAttempts || recoveredTimestampContradictsCount) {
    failProjection("sarah_media_admission_invalid_recovery_counters")
  }
}

/**
 * Composes FC-3 continuity rather than restating its frame/transport laws.
 * Admission decides whether realtime media may reach that projector; the
 * continuity projector remains the sole authority for LIVE versus stale.
 */
export function projectSarahMediaAdmission(
  unsafeInput: unknown,
  unsafeNowMs: unknown,
): MediaAdmissionProjection {
  const nowMs = decodeProjectionClock(unsafeNowMs)
  const input = decodeProjectionInput(unsafeInput)
  assertTemporalCoherence(input, nowMs)
  assertRecoveryCoherence(input.recovery)
  const realtime = projectRealtime(input.realtime, nowMs)
  const continuity = fleetContinuityProjection(
    {
      ...input.continuity,
      media: mediaObservationForAdmission(input.continuity.media, realtime),
    },
    nowMs,
  )
  const recovery = projectRecovery(realtime, continuity, input.recovery)
  const transportLease =
    realtime.status === "available" &&
    input.continuity.media.status === "live"
      ? {
          transportLeaseRef:
            input.continuity.media.lease.transportLeaseRef,
          expiresAtMs: input.continuity.media.lease.transportExpiresAtMs,
          lastFrameAtMs: input.continuity.media.lease.lastFrameAtMs,
          state: continuity.media.status === "live"
            ? ("fresh" as const)
            : ("stale" as const),
        }
      : null

  return {
    schema: "sarah.media_admission_projection.v1",
    text: {
      floor: "text",
      delayedByMedia: false,
      textControl: continuity.continuation.textControl,
      fleetControl: continuity.continuation.fleetControl,
    },
    preRendered: projectPreRendered(input.preRendered),
    realtime,
    continuity,
    telemetry: {
      costs: input.costs,
      queue: {
        requestRef:
          realtime.status === "available" || realtime.status === "queued"
            ? realtime.requestRef
            : realtime.status === "text_only"
              ? realtime.requestRef
              : null,
        waitMs: realtime.queueWaitMs,
        deadlineAtMs: realtime.queueDeadlineAtMs,
        expired:
          realtime.status === "text_only" &&
          realtime.reason === "queue_expired",
      },
      leases: {
        admission: realtime.status === "available" ? realtime.lease : null,
        transport: transportLease,
      },
      recovery,
      textFallback:
        realtime.status === "text_only" ||
        realtime.status === "unavailable" ||
        continuity.media.status !== "live",
    },
  }
}

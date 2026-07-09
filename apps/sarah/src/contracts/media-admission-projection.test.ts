import { describe, expect, test } from "bun:test"
import { Schema } from "effect"

import {
  MediaAdmissionProjectionInput,
  SARAH_MEDIA_MAX_QUEUE_WAIT_MS,
  SarahMediaAdmissionProjectionError,
  projectSarahMediaAdmission,
  type MediaAdmissionProjectionInput as MediaAdmissionProjectionInputType,
} from "./media-admission-projection.ts"
import { FC3_FRESHNESS_TIMEOUT_MS } from "./fleet-continuity-projection.ts"

const NOW = 2_000_000

const baseInput = (
  overrides: Partial<MediaAdmissionProjectionInputType> = {},
): MediaAdmissionProjectionInputType => ({
  continuity: {
    conversation: { status: "text_live" },
    media: { status: "not_requested" },
    progress: { status: "not_started" },
  },
  preRendered: { status: "not_available" },
  realtime: {
    status: "text_only",
    reason: "not_requested",
    switchedAtMs: NOW,
  },
  costs: {
    preRendered: {
      measurementStatus: "not_measured",
      costClass: "not_measured",
      marginalCostPerActiveMinuteUsd: null,
    },
    realtime: {
      measurementStatus: "reported",
      costClass: "metered_realtime",
      marginalCostPerActiveMinuteUsd: 0.42,
    },
    offlineOnly: {
      measurementStatus: "reported",
      costClass: "offline_batch",
      marginalCostPerActiveMinuteUsd: 0.07,
    },
  },
  recovery: { status: "not_reported" },
  ...overrides,
})

describe("#8610 typed Sarah media admission", () => {
  test("the decoded contract keeps text as the floor and pre-rendered takes never delay input", () => {
    const input = Schema.decodeUnknownSync(MediaAdmissionProjectionInput)(
      baseInput({
        preRendered: {
          status: "available",
          takeRef: "take.opener.hello.v3",
          source: "opener",
        },
        realtime: {
          status: "queued",
          requestRef: "media.request.1",
          requestedAtMs: NOW - 4_000,
          deadlineAtMs: NOW + 20_000,
          queuePosition: 2,
        },
      }),
    )
    const projection = projectSarahMediaAdmission(input, NOW)

    expect(projection.text).toEqual({
      floor: "text",
      delayedByMedia: false,
      textControl: "available",
      fleetControl: "available",
    })
    expect(projection.preRendered).toEqual({
      status: "available",
      takeRef: "take.opener.hello.v3",
      source: "opener",
      inputPolicy: "never_blocks_text",
      inputDelayMs: 0,
    })
    expect(projection.realtime).toMatchObject({
      status: "queued",
      queueWaitMs: 4_000,
      reservation: "none",
      lease: null,
      expiresTo: "text_only",
    })
    expect(projection.continuity.media.status).toBe("queued")
    expect(projection.telemetry.recovery).toMatchObject({
      state: "waiting_for_admission",
      action: "continue_in_text",
    })
  })

  test("queue deadlines clamp and expire deterministically to text with no invisible reservation", () => {
    const requestedAtMs = NOW - SARAH_MEDIA_MAX_QUEUE_WAIT_MS
    const projection = projectSarahMediaAdmission(
      baseInput({
        continuity: {
          conversation: { status: "text_live" },
          // A contradictory upstream LIVE observation cannot bypass admission.
          media: {
            status: "live",
            lease: {
              transportLeaseRef: "transport.hidden.1",
              transportExpiresAtMs: NOW + 60_000,
              lastFrameAtMs: NOW,
            },
          },
          progress: { status: "not_started" },
        },
        realtime: {
          status: "queued",
          requestRef: "media.request.expiring",
          requestedAtMs,
          deadlineAtMs: NOW + 600_000,
          queuePosition: 1,
        },
      }),
      NOW,
    )

    expect(projection.realtime).toEqual({
      status: "text_only",
      reason: "queue_expired",
      requestRef: "media.request.expiring",
      queueWaitMs: SARAH_MEDIA_MAX_QUEUE_WAIT_MS,
      queueDeadlineAtMs: NOW,
      switchedAtMs: NOW,
      reservation: "none",
      lease: null,
    })
    expect(projection.continuity.media.status).toBe("not_requested")
    expect(projection.continuity.media.status).not.toBe("live")
    expect(projection.telemetry.leases.admission).toBeNull()
    expect(projection.telemetry.leases.transport).toBeNull()
    expect(projection.telemetry.queue).toEqual({
      requestRef: "media.request.expiring",
      waitMs: SARAH_MEDIA_MAX_QUEUE_WAIT_MS,
      deadlineAtMs: NOW,
      expired: true,
    })
    expect(projection.text.textControl).toBe("available")
    expect(projection.text.fleetControl).toBe("available")
  })

  test("late available admission reports actual admission wait while still expiring at the bounded deadline", () => {
    const requestedAtMs = NOW - 40_000
    const projection = projectSarahMediaAdmission(
      baseInput({
        realtime: {
          status: "available",
          requestRef: "media.request.late",
          requestedAtMs,
          deadlineAtMs: NOW + 600_000,
          admittedAtMs: NOW - 5_000,
          admissionLeaseRef: "admission.late.1",
          admissionLeaseExpiresAtMs: NOW + 60_000,
        },
      }),
      NOW,
    )

    expect(projection.realtime).toEqual({
      status: "text_only",
      reason: "queue_expired",
      requestRef: "media.request.late",
      // Actual provider admission took 35s; the user-facing queue ended at 30s.
      queueWaitMs: 35_000,
      queueDeadlineAtMs: requestedAtMs + SARAH_MEDIA_MAX_QUEUE_WAIT_MS,
      switchedAtMs: requestedAtMs + SARAH_MEDIA_MAX_QUEUE_WAIT_MS,
      reservation: "none",
      lease: null,
    })
    expect(projection.telemetry.leases.admission).toBeNull()
  })

  test("available realtime exposes measured costs and both admission and fresh transport leases", () => {
    const projection = projectSarahMediaAdmission(
      baseInput({
        continuity: {
          conversation: { status: "text_live" },
          media: {
            status: "live",
            lease: {
              transportLeaseRef: "transport.live.1",
              transportExpiresAtMs: NOW + 45_000,
              lastFrameAtMs: NOW - 1_000,
            },
          },
          progress: { status: "not_started" },
        },
        realtime: {
          status: "available",
          requestRef: "media.request.live",
          requestedAtMs: NOW - 5_000,
          deadlineAtMs: NOW + 10_000,
          admittedAtMs: NOW - 2_000,
          admissionLeaseRef: "admission.live.1",
          admissionLeaseExpiresAtMs: NOW + 15_000,
        },
      }),
      NOW,
    )

    expect(projection.realtime).toMatchObject({
      status: "available",
      queueWaitMs: 3_000,
      reservation: "leased",
      lease: {
        admissionLeaseRef: "admission.live.1",
        expiresAtMs: NOW + 15_000,
        remainingMs: 15_000,
      },
    })
    expect(projection.continuity.media.status).toBe("live")
    expect(projection.telemetry.leases.transport).toEqual({
      transportLeaseRef: "transport.live.1",
      expiresAtMs: NOW + 45_000,
      lastFrameAtMs: NOW - 1_000,
      state: "fresh",
    })
    expect(projection.telemetry.costs.realtime).toEqual({
      measurementStatus: "reported",
      costClass: "metered_realtime",
      marginalCostPerActiveMinuteUsd: 0.42,
    })
  })

  test("a stale frame makes LIVE impossible while preserving explicit reconnect and controls", () => {
    const projection = projectSarahMediaAdmission(
      baseInput({
        continuity: {
          conversation: { status: "text_live" },
          media: {
            status: "live",
            lease: {
              transportLeaseRef: "transport.stale.1",
              transportExpiresAtMs: NOW + 60_000,
              lastFrameAtMs: NOW - FC3_FRESHNESS_TIMEOUT_MS,
            },
          },
          progress: { status: "not_started" },
        },
        realtime: {
          status: "available",
          requestRef: "media.request.stale",
          requestedAtMs: NOW - 5_000,
          deadlineAtMs: NOW + 5_000,
          admittedAtMs: NOW - 4_000,
          admissionLeaseRef: "admission.stale.1",
          admissionLeaseExpiresAtMs: NOW + 60_000,
        },
        recovery: {
          status: "reported",
          attemptCount: 1,
          recoveredCount: 0,
          abandonedCount: 0,
          lastRecoveredAtMs: null,
        },
      }),
      NOW,
    )

    expect(projection.continuity.media).toMatchObject({
      status: "stale",
      frame: "frozen",
      badge: "reconnecting",
      reason: "frame_stale",
    })
    expect(projection.continuity.media.status).not.toBe("live")
    expect(projection.telemetry.leases.transport?.state).toBe("stale")
    expect(projection.telemetry.recovery).toMatchObject({
      state: "reconnecting",
      action: "reconnect_media",
      telemetry: { status: "reported", attemptCount: 1 },
    })
    expect(projection.text).toMatchObject({
      delayedByMedia: false,
      textControl: "available",
      fleetControl: "available",
    })
  })

  test("media failure remains text-only and cannot disable fleet or text controls", () => {
    const projection = projectSarahMediaAdmission(
      baseInput({
        continuity: {
          conversation: { status: "busy" },
          media: { status: "unavailable" },
          progress: {
            status: "active",
            workUnitRef: "work.media.failure",
            lastFreshAtMs: NOW,
          },
        },
        preRendered: {
          status: "unavailable",
          reason: "playback_failed",
        },
        realtime: {
          status: "unavailable",
          reason: "provider_failed",
          observedAtMs: NOW,
        },
      }),
      NOW,
    )

    expect(projection.preRendered).toMatchObject({
      status: "unavailable",
      inputPolicy: "never_blocks_text",
      inputDelayMs: 0,
    })
    expect(projection.realtime).toMatchObject({
      status: "unavailable",
      reservation: "none",
      lease: null,
    })
    expect(projection.continuity.continuation).toEqual({
      status: "text_only",
      textControl: "available",
      fleetControl: "available",
    })
    expect(projection.text).toEqual({
      floor: "text",
      delayedByMedia: false,
      textControl: "available",
      fleetControl: "available",
    })
    expect(projection.telemetry.textFallback).toBe(true)
  })

  test("runtime decode rejects non-finite or unbounded numeric input with fixed public-safe errors", () => {
    for (const invalidClock of [Infinity, Number.MAX_SAFE_INTEGER + 1]) {
      expect(() =>
        projectSarahMediaAdmission(baseInput(), invalidClock),
      ).toThrow("sarah_media_admission_invalid_clock")
    }

    const invalidCost = baseInput({
      costs: {
        ...baseInput().costs,
        realtime: {
          measurementStatus: "reported",
          costClass: "metered_realtime",
          marginalCostPerActiveMinuteUsd: Infinity,
        },
      },
    })
    try {
      projectSarahMediaAdmission(invalidCost, NOW)
      throw new Error("expected invalid input")
    } catch (error) {
      expect(error).toBeInstanceOf(SarahMediaAdmissionProjectionError)
      expect((error as SarahMediaAdmissionProjectionError).code).toBe(
        "sarah_media_admission_invalid_input",
      )
      expect((error as Error).message).toBe(
        "sarah_media_admission_invalid_input",
      )
      expect((error as Error).message).not.toContain("Infinity")
    }

  })

  test("runtime decode rejects incoherent realtime, continuity, and recovery timestamps", () => {
    const invalidRealtime = [
      {
        status: "queued" as const,
        requestRef: "media.request.bad.deadline",
        requestedAtMs: NOW,
        deadlineAtMs: NOW - 1,
        queuePosition: 1,
      },
      {
        status: "available" as const,
        requestRef: "media.request.bad.admitted",
        requestedAtMs: NOW - 10,
        deadlineAtMs: NOW + 10,
        admittedAtMs: NOW - 11,
        admissionLeaseRef: "admission.bad.admitted",
        admissionLeaseExpiresAtMs: NOW + 20,
      },
      {
        status: "available" as const,
        requestRef: "media.request.bad.lease",
        requestedAtMs: NOW - 10,
        deadlineAtMs: NOW + 10,
        admittedAtMs: NOW - 5,
        admissionLeaseRef: "admission.bad.lease",
        admissionLeaseExpiresAtMs: NOW - 5,
      },
    ]
    for (const realtime of invalidRealtime) {
      expect(() =>
        projectSarahMediaAdmission(baseInput({ realtime }), NOW),
      ).toThrow("sarah_media_admission_invalid_temporal_order")
    }

    expect(() =>
      projectSarahMediaAdmission(
        baseInput({
          continuity: {
            conversation: { status: "text_live" },
            media: {
              status: "live",
              lease: {
                transportLeaseRef: "transport.bad.future",
                transportExpiresAtMs: NOW + 20,
                lastFrameAtMs: NOW + 1,
              },
            },
            progress: { status: "not_started" },
          },
        }),
        NOW,
      ),
    ).toThrow("sarah_media_admission_invalid_temporal_order")

    const invalidContinuityNumbers = [
      {
        conversation: { status: "text_live" as const },
        media: {
          status: "live" as const,
          lease: {
            transportLeaseRef: "transport.invalid.infinity",
            transportExpiresAtMs: Infinity,
            lastFrameAtMs: NOW,
          },
        },
        progress: { status: "not_started" as const },
      },
      {
        conversation: { status: "text_live" as const },
        media: { status: "stale" as const, lastFrameAtMs: -Infinity },
        progress: { status: "not_started" as const },
      },
      {
        conversation: { status: "busy" as const },
        media: { status: "unavailable" as const },
        progress: {
          status: "active" as const,
          workUnitRef: "work.invalid.negative",
          lastFreshAtMs: -1,
        },
      },
    ]
    for (const continuity of invalidContinuityNumbers) {
      expect(() =>
        projectSarahMediaAdmission(baseInput({ continuity }), NOW),
      ).toThrow("sarah_media_admission_invalid_temporal_order")
    }

    expect(() =>
      projectSarahMediaAdmission(
        baseInput({
          recovery: {
            status: "reported",
            attemptCount: 1,
            recoveredCount: 1,
            abandonedCount: 0,
            lastRecoveredAtMs: NOW + 1,
          },
        }),
        NOW,
      ),
    ).toThrow("sarah_media_admission_invalid_temporal_order")
  })

  test("runtime decode rejects recovery totals and timestamps that contradict attempts", () => {
    const invalidRecoveries = [
      {
        status: "reported" as const,
        attemptCount: 1,
        recoveredCount: 1,
        abandonedCount: 1,
        lastRecoveredAtMs: NOW,
      },
      {
        status: "reported" as const,
        attemptCount: 1,
        recoveredCount: 0,
        abandonedCount: 0,
        lastRecoveredAtMs: NOW,
      },
      {
        status: "reported" as const,
        attemptCount: 1,
        recoveredCount: 1,
        abandonedCount: 0,
        lastRecoveredAtMs: null,
      },
    ]
    for (const recovery of invalidRecoveries) {
      expect(() =>
        projectSarahMediaAdmission(baseInput({ recovery }), NOW),
      ).toThrow("sarah_media_admission_invalid_recovery_counters")
    }
  })
})

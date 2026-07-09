import { describe, expect, test } from "bun:test"
import { Schema } from "effect"

import {
  ConversationObservation,
  FC3_FRESHNESS_TIMEOUT_MS,
  FC3_PROGRESS_CADENCE_MS,
  MediaObservation,
  WorkStreamProgressObservation,
  fleetContinuityProjection,
  type MediaPresentation,
} from "./fleet-continuity-projection.ts"

const NOW = 1_000_000

describe("FC-3 fleet continuity projection", () => {
  test("conversation and media observations have independent typed state machines", () => {
    for (const status of [
      "idle",
      "connecting",
      "text_live",
      "busy",
      "reconnecting",
      "ended",
      "failed",
    ] as const) {
      expect(
        Schema.decodeUnknownSync(ConversationObservation)({ status }),
      ).toEqual({ status })
    }
    for (const status of [
      "not_requested",
      "queued",
      "connecting",
      "unavailable",
      "evicted",
      "ended",
    ] as const) {
      expect(Schema.decodeUnknownSync(MediaObservation)({ status })).toEqual({
        status,
      })
    }
    expect(
      Schema.decodeUnknownSync(MediaObservation)({
        status: "stale",
        lastFrameAtMs: NOW,
      }),
    ).toEqual({ status: "stale", lastFrameAtMs: NOW })
    expect(() =>
      Schema.decodeUnknownSync(ConversationObservation)({
        status: "live",
      }),
    ).toThrow()
    expect(() =>
      Schema.decodeUnknownSync(MediaObservation)({
        status: "text_live",
      }),
    ).toThrow()
  })

  test("projects media LIVE only while both frame and transport lease are fresh", () => {
    const projection = fleetContinuityProjection(
      {
        conversation: { status: "text_live" },
        media: {
          status: "live",
          lease: {
            transportLeaseRef: "lease:media:1",
            transportExpiresAtMs: NOW + 1,
            lastFrameAtMs: NOW - FC3_FRESHNESS_TIMEOUT_MS + 1,
          },
        },
        progress: { status: "not_started" },
      },
      NOW,
    )

    expect(projection.media).toEqual({
      status: "live",
      frame: "moving",
      badge: "live",
      lease: {
        transportLeaseRef: "lease:media:1",
        transportExpiresAtMs: NOW + 1,
        lastFrameAtMs: NOW - FC3_FRESHNESS_TIMEOUT_MS + 1,
      },
    })
    expect(projection.continuation.status).toBe("full_media")
  })

  test("expires a frame at 30 seconds without killing text or fleet controls", () => {
    const projection = fleetContinuityProjection(
      {
        conversation: { status: "text_live" },
        media: {
          status: "live",
          lease: {
            transportLeaseRef: "lease:media:2",
            transportExpiresAtMs: NOW + 60_000,
            lastFrameAtMs: NOW - FC3_FRESHNESS_TIMEOUT_MS,
          },
        },
        progress: { status: "not_started" },
      },
      NOW,
    )

    expect(projection.media).toEqual({
      status: "stale",
      frame: "frozen",
      badge: "reconnecting",
      lastFrameAtMs: NOW - FC3_FRESHNESS_TIMEOUT_MS,
      reason: "frame_stale",
    })
    expect(projection.continuation).toEqual({
      status: "text_continuation_reconnect",
      textControl: "available",
      fleetControl: "available",
      message: "Video reconnecting. Keep working in text.",
      action: "reconnect_media",
    })
  })

  test("an expired transport lease cannot retain a LIVE badge", () => {
    const projection = fleetContinuityProjection(
      {
        conversation: { status: "text_live" },
        media: {
          status: "live",
          lease: {
            transportLeaseRef: "lease:media:3",
            transportExpiresAtMs: NOW,
            lastFrameAtMs: NOW,
          },
        },
        progress: { status: "not_started" },
      },
      NOW,
    )

    expect(projection.media.status).toBe("stale")
    expect(projection.media.badge).toBe("reconnecting")
    expect(projection.media.frame).toBe("frozen")
  })

  test("the media presentation union cannot represent frozen LIVE", () => {
    type FrozenLive = Extract<
      MediaPresentation,
      { status: "live"; frame: "frozen" }
    >
    const frozenLiveIsImpossible: FrozenLive extends never ? true : false = true
    expect(frozenLiveIsImpossible).toBe(true)
  })

  test("projects first-progress and active-progress timeouts as stalled at 30 seconds", () => {
    const awaiting = Schema.decodeUnknownSync(WorkStreamProgressObservation)({
      status: "awaiting_first",
      workUnitRef: "work:alpha",
      startedAtMs: NOW - FC3_FRESHNESS_TIMEOUT_MS,
    })
    const active = Schema.decodeUnknownSync(WorkStreamProgressObservation)({
      status: "active",
      workUnitRef: "work:beta",
      lastFreshAtMs: NOW - FC3_FRESHNESS_TIMEOUT_MS,
    })

    expect(
      fleetContinuityProjection(
        {
          conversation: { status: "busy" },
          media: { status: "unavailable" },
          progress: awaiting,
        },
        NOW,
      ).progress,
    ).toEqual({
      status: "stalled",
      workUnitRef: "work:alpha",
      ageMs: FC3_FRESHNESS_TIMEOUT_MS,
      reason: "first_progress_timeout",
      reconnect: true,
    })
    expect(
      fleetContinuityProjection(
        {
          conversation: { status: "busy" },
          media: { status: "unavailable" },
          progress: active,
        },
        NOW,
      ).progress,
    ).toEqual({
      status: "stalled",
      workUnitRef: "work:beta",
      ageMs: FC3_FRESHNESS_TIMEOUT_MS,
      reason: "progress_stale",
      reconnect: true,
    })
  })

  test("a fresh active stream carries both 15-second cadence and 30-second deadline", () => {
    const lastFreshAtMs = NOW - 2_000
    const progress = fleetContinuityProjection(
      {
        conversation: { status: "busy" },
        media: { status: "unavailable" },
        progress: {
          status: "active",
          workUnitRef: "work:gamma",
          lastFreshAtMs,
        },
      },
      NOW,
    ).progress

    expect(progress).toEqual({
      status: "running",
      workUnitRef: "work:gamma",
      ageMs: 2_000,
      nextProgressExpectedAtMs: lastFreshAtMs + FC3_PROGRESS_CADENCE_MS,
      staleAtMs: lastFreshAtMs + FC3_FRESHNESS_TIMEOUT_MS,
    })
  })
})

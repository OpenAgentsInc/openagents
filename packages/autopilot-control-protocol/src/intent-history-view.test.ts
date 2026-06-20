import { describe, expect, test } from "bun:test"

import { projectIntentHistory, type IntentHistoryView } from "./intent-history-view.js"

describe("intent history view projection", () => {
  test("projects a direct intent projection with status history", () => {
    expect(projectIntentHistory({
      intentId: "intent.public.0001",
      status: "completed",
      statusHistory: [
        {
          status: "queued",
          observedAt: "2026-06-13T12:00:00.000Z",
        },
        {
          status: "running",
          observedAt: "2026-06-13T12:00:05.000Z",
        },
        {
          status: "completed",
          observedAt: "2026-06-13T12:00:20.000Z",
        },
      ],
    })).toEqual({
      intentId: "intent.public.0001",
      status: "completed",
      steps: [
        {
          status: "queued",
          observedAt: "2026-06-13T12:00:00.000Z",
        },
        {
          status: "running",
          observedAt: "2026-06-13T12:00:05.000Z",
        },
        {
          status: "completed",
          observedAt: "2026-06-13T12:00:20.000Z",
        },
      ],
      durationMs: 20_000,
    } satisfies IntentHistoryView)
  })

  test("projects snake case aliases", () => {
    expect(projectIntentHistory({
      intent_id: "intent.public.0002",
      state: "failed",
      status_history: [
        {
          state: "queued",
          observed_at: "2026-06-13T12:01:00.000Z",
        },
        {
          state: "failed",
          observed_at: "2026-06-13T12:01:07.500Z",
        },
      ],
    })).toEqual({
      intentId: "intent.public.0002",
      status: "failed",
      steps: [
        {
          status: "queued",
          observedAt: "2026-06-13T12:01:00.000Z",
        },
        {
          status: "failed",
          observedAt: "2026-06-13T12:01:07.500Z",
        },
      ],
      durationMs: 7_500,
    } satisfies IntentHistoryView)
  })

  test("reads nested result projection records", () => {
    expect(projectIntentHistory({
      result: {
        projection: {
          id: "intent.public.nested",
          status: "running",
          statusHistory: [
            {
              status: "queued",
              timestamp: "2026-06-13T12:02:00.000Z",
            },
            {
              status: "running",
              timestamp: "2026-06-13T12:02:03.000Z",
            },
          ],
        },
      },
    })).toEqual({
      intentId: "intent.public.nested",
      status: "running",
      steps: [
        {
          status: "queued",
          observedAt: "2026-06-13T12:02:00.000Z",
        },
        {
          status: "running",
          observedAt: "2026-06-13T12:02:03.000Z",
        },
      ],
      durationMs: 3_000,
    } satisfies IntentHistoryView)
  })

  test("falls back to the last step status when current status is missing", () => {
    expect(projectIntentHistory({
      ref: "intent.public.statusless",
      statusHistory: [
        {
          status: "queued",
          observedAt: "2026-06-13T12:03:00.000Z",
        },
        {
          status: "waiting_for_owner",
          observedAt: "2026-06-13T12:03:11.000Z",
        },
      ],
    })).toEqual({
      intentId: "intent.public.statusless",
      status: "waiting_for_owner",
      steps: [
        {
          status: "queued",
          observedAt: "2026-06-13T12:03:00.000Z",
        },
        {
          status: "waiting_for_owner",
          observedAt: "2026-06-13T12:03:11.000Z",
        },
      ],
      durationMs: 11_000,
    } satisfies IntentHistoryView)
  })

  test("skips malformed history entries defensively", () => {
    expect(projectIntentHistory({
      intentId: "intent.public.partial",
      status: "running",
      statusHistory: [
        null,
        "bad",
        {
          status: "missing-time",
        },
        {
          observedAt: "2026-06-13T12:04:00.000Z",
        },
        {
          status: "running",
          observedAt: "2026-06-13T12:04:04.000Z",
        },
      ],
    })).toEqual({
      intentId: "intent.public.partial",
      status: "running",
      steps: [
        {
          status: "running",
          observedAt: "2026-06-13T12:04:04.000Z",
        },
      ],
      durationMs: null,
    } satisfies IntentHistoryView)
  })

  test("returns a closed projection for bad input", () => {
    expect(projectIntentHistory(undefined)).toEqual({
      intentId: "",
      status: "unknown",
      steps: [],
      durationMs: null,
    } satisfies IntentHistoryView)
    expect(projectIntentHistory(null)).toEqual({
      intentId: "",
      status: "unknown",
      steps: [],
      durationMs: null,
    } satisfies IntentHistoryView)
    expect(projectIntentHistory("not-json")).toEqual({
      intentId: "",
      status: "unknown",
      steps: [],
      durationMs: null,
    } satisfies IntentHistoryView)
  })

  test("keeps duration nullable for invalid or reversed timestamps", () => {
    expect(projectIntentHistory({
      intentId: "intent.public.invalid-time",
      statusHistory: [
        {
          status: "queued",
          observedAt: "not-a-date",
        },
        {
          status: "running",
          observedAt: "2026-06-13T12:05:00.000Z",
        },
      ],
    }).durationMs).toBeNull()

    expect(projectIntentHistory({
      intentId: "intent.public.reversed",
      statusHistory: [
        {
          status: "running",
          observedAt: "2026-06-13T12:06:00.000Z",
        },
        {
          status: "queued",
          observedAt: "2026-06-13T12:05:00.000Z",
        },
      ],
    }).durationMs).toBeNull()
  })
})

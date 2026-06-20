import { describe, expect, test } from "bun:test"

import {
  buildSessionDetail,
  type SessionDetailViewModel,
} from "./session-detail-view-model.js"

describe("session detail view model", () => {
  test("builds an idle detail for a session with no events or artifact", () => {
    expect(buildSessionDetail({
      sessionRef: "session.public.empty",
      events: [],
      artifact: null,
    })).toEqual({
      sessionRef: "session.public.empty",
      state: "idle",
      eventCount: 0,
      lastActivity: "",
      hasArtifact: false,
      outcome: null,
    } satisfies SessionDetailViewModel)
  })

  test("reports running state and last activity for non-terminal events", () => {
    expect(buildSessionDetail({
      sessionRef: "session.public.running",
      events: [
        {
          phase: "started",
          messageText: "Started session",
          observedAt: "2026-06-13T14:00:00.000Z",
        },
        {
          phase: "thinking",
          messageText: "Reading context",
          observedAt: "2026-06-13T14:01:00.000Z",
        },
      ],
      artifact: null,
    })).toEqual({
      sessionRef: "session.public.running",
      state: "running",
      eventCount: 2,
      lastActivity: "2026-06-13T14:01:00.000Z",
      hasArtifact: false,
      outcome: null,
    } satisfies SessionDetailViewModel)
  })

  test("reports completed state from the session state reducer", () => {
    expect(buildSessionDetail({
      sessionRef: "session.public.completed",
      events: [
        {
          phase: "started",
          messageText: "Started session",
          observedAt: "2026-06-13T14:00:00.000Z",
        },
        {
          phase: "completed",
          messageText: "Finished session",
          observedAt: "2026-06-13T14:03:00.000Z",
        },
      ],
      artifact: null,
    })).toMatchObject({
      sessionRef: "session.public.completed",
      state: "completed",
      eventCount: 2,
      lastActivity: "2026-06-13T14:03:00.000Z",
    } satisfies Partial<SessionDetailViewModel>)
  })

  test("preserves terminal reducer behavior when later cleanup events arrive", () => {
    expect(buildSessionDetail({
      sessionRef: "session.public.failed-cleanup",
      events: [
        {
          phase: "failed",
          messageText: "Command failed",
          observedAt: "2026-06-13T14:04:00.000Z",
        },
        {
          phase: "cleanup",
          messageText: "Collected logs",
          observedAt: "2026-06-13T14:05:00.000Z",
        },
      ],
      artifact: null,
    })).toEqual({
      sessionRef: "session.public.failed-cleanup",
      state: "failed",
      eventCount: 2,
      lastActivity: "2026-06-13T14:05:00.000Z",
      hasArtifact: false,
      outcome: null,
    } satisfies SessionDetailViewModel)
  })

  test("marks an artifact present and carries its outcome", () => {
    expect(buildSessionDetail({
      sessionRef: "session.public.artifact",
      events: [
        {
          phase: "completed",
          messageText: "Produced artifact",
          observedAt: "2026-06-13T14:06:00.000Z",
        },
      ],
      artifact: {
        kind: "diff",
        outcome: "accepted",
      },
    })).toEqual({
      sessionRef: "session.public.artifact",
      state: "completed",
      eventCount: 1,
      lastActivity: "2026-06-13T14:06:00.000Z",
      hasArtifact: true,
      outcome: "accepted",
    } satisfies SessionDetailViewModel)
  })

  test("marks an artifact present even when its outcome is pending", () => {
    expect(buildSessionDetail({
      sessionRef: "session.public.pending-artifact",
      events: [
        {
          phase: "cancelled",
          messageText: "Owner cancelled",
          observedAt: "2026-06-13T14:07:00.000Z",
        },
      ],
      artifact: {
        kind: "summary",
        outcome: null,
      },
    })).toEqual({
      sessionRef: "session.public.pending-artifact",
      state: "cancelled",
      eventCount: 1,
      lastActivity: "2026-06-13T14:07:00.000Z",
      hasArtifact: true,
      outcome: null,
    } satisfies SessionDetailViewModel)
  })
})

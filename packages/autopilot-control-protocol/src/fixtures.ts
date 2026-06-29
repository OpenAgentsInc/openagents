// Shared node fixtures (CL-1) so every client tests against the same node
// behavior without a live node. Refs only — no secrets, paths, or raw payloads.

import { CONTROL_HEALTH_CAPABILITIES, CONTROL_SCHEMA_TAG } from "./control.js"
import type { SessionEvent, SessionSummary } from "./control.js"

export const healthFixture = {
  ok: true,
  schema: CONTROL_SCHEMA_TAG,
  capabilities: [...CONTROL_HEALTH_CAPABILITIES],
}

export const sessionListFixture: SessionSummary[] = [
  {
    sessionRef: "session.pylon.codex_composer.fixture0001",
    adapter: "codex",
    state: "running",
    objectiveRef: "objective.fixture.abc123",
    accountRefHash: "account.pylon.codex.fixturehash01",
    lastProgressRef: "progress.fixture.0001",
    updatedAt: "2026-06-13T12:00:00.000Z",
  },
  {
    sessionRef: "session.pylon.claude_composer.fixture0002",
    adapter: "claude_agent",
    state: "completed",
    objectiveRef: "objective.fixture.def456",
    accountRefHash: null,
    updatedAt: "2026-06-13T12:01:00.000Z",
  },
]

// An ordered event stream for one session: start → progress → decision
// requested → decision resolved → completed. Sequences are strictly increasing.
export const sessionEventStreamFixture: SessionEvent[] = [
  {
    schema: CONTROL_SCHEMA_TAG,
    sessionRef: "session.pylon.codex_composer.fixture0001",
    eventId: "evt.0001",
    sequence: 1,
    phase: "started",
    projectionLevel: "public_safe",
    observedAt: "2026-06-13T12:00:00.000Z",
  },
  {
    schema: CONTROL_SCHEMA_TAG,
    sessionRef: "session.pylon.codex_composer.fixture0001",
    eventId: "evt.0002",
    sequence: 2,
    phase: "progress",
    projectionLevel: "public_safe",
    observedAt: "2026-06-13T12:00:05.000Z",
    detailRef: "progress.fixture.0002",
  },
  {
    schema: CONTROL_SCHEMA_TAG,
    sessionRef: "session.pylon.codex_composer.fixture0001",
    eventId: "evt.0003",
    sequence: 3,
    phase: "decision_requested",
    projectionLevel: "public_safe",
    observedAt: "2026-06-13T12:00:10.000Z",
    detailRef: "decision.fixture.req01",
  },
  {
    schema: CONTROL_SCHEMA_TAG,
    sessionRef: "session.pylon.codex_composer.fixture0001",
    eventId: "evt.0004",
    sequence: 4,
    phase: "decision_resolved",
    projectionLevel: "public_safe",
    observedAt: "2026-06-13T12:00:20.000Z",
    detailRef: "decision.fixture.req01",
  },
  {
    schema: CONTROL_SCHEMA_TAG,
    sessionRef: "session.pylon.codex_composer.fixture0001",
    eventId: "evt.0005",
    sequence: 5,
    phase: "completed",
    projectionLevel: "public_safe",
    observedAt: "2026-06-13T12:00:30.000Z",
  },
]

export const decisionRequestFixture = {
  requestId: "decision.fixture.req01",
  actionRef: "action.fixture.approve_pr",
  expiresAtMs: 1_900_000_000_000,
}

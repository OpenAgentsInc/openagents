import { describe, expect, test } from "bun:test"

import { postAssignmentRunnerStatusEvent } from "./assignment.js"

describe("assignment status producer", () => {
  test("posts public-safe agent runner status events to the operator spine", async () => {
    const requests: Array<{ body: Record<string, unknown>; headers: Headers; url: string }> = []
    const fetchImpl = (async (
      url: Parameters<typeof fetch>[0],
      init: Parameters<typeof fetch>[1],
    ) => {
      requests.push({
        body: JSON.parse(String(init?.body)) as Record<string, unknown>,
        headers: new Headers(init?.headers),
        url: String(url),
      })
      return new Response(JSON.stringify({ ok: true }), { status: 200 })
    }) as unknown as typeof fetch

    const posted = await postAssignmentRunnerStatusEvent({
      agentToken: "agent-token-fixture",
      baseUrl: "https://openagents.example",
      event: {
        schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
        event: "assignment_run.accepted",
        observedAt: "2026-07-01T12:00:00.000Z",
        assignmentRef: "assignment.public.fixture",
        leaseRef: "lease.public.fixture",
        statusRef: "assignment.accepted.fixture",
      },
      fetch: fetchImpl,
      pylonRef: "pylon.public.fixture",
    })

    expect(posted).toBe(true)
    expect(requests).toHaveLength(1)
    expect(requests[0]?.url).toBe("https://openagents.example/api/operator/pro/status")
    expect(requests[0]?.headers.get("authorization")).toBe("Bearer agent-token-fixture")
    expect(requests[0]?.body).toMatchObject({
      schemaVersion: "openagents.pylon.agent_runner_status_event.v1",
      state: "queued",
      runnerKind: "local_command",
      stateStartedAt: "2026-07-01T12:00:00.000Z",
      updatedAt: "2026-07-01T12:00:00.000Z",
    })
    expect(String(requests[0]?.body.eventRef)).toStartWith("event.public.pylon.runner_status.")
    expect(String(requests[0]?.body.assignmentRef)).toStartWith("assignment.public.pylon.")
    expect(JSON.stringify(requests[0]?.body)).not.toContain("lease.public.fixture")
  })

  test("skips status publication when no agent credential is available", async () => {
    let called = false
    const posted = await postAssignmentRunnerStatusEvent({
      baseUrl: "https://openagents.example",
      event: {
        schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
        event: "assignment_run.poll_complete",
        observedAt: "2026-07-01T12:00:00.000Z",
      },
      fetch: (async () => {
        called = true
        return new Response(null, { status: 204 })
      }) as unknown as typeof fetch,
      pylonRef: "pylon.public.fixture",
    })

    expect(posted).toBe(false)
    expect(called).toBe(false)
  })
})

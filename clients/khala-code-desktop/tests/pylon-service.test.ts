import { describe, expect, test } from "bun:test"
import { Effect, Stream } from "effect"
import type { PylonAssignmentRunLifecycleEvent } from "@openagentsinc/agent-runtime-schema"

import {
  makePylonService,
  PylonService,
  PylonServiceStub,
  type PylonServiceCommandRunnerInput,
} from "../src/bun/pylon-service"

const lifecycleEvent = (
  event: PylonAssignmentRunLifecycleEvent["event"] = "assignment_run.runtime_progress",
): PylonAssignmentRunLifecycleEvent => ({
  schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
  assignmentRef: "assignment.test",
  event,
  observedAt: "2026-07-01T12:00:00.000Z",
  phase: "runtime_active",
  status: "running",
})

describe("PylonService", () => {
  test("decodes command lifecycle NDJSON into isolated streams", async () => {
    const event = lifecycleEvent()
    const service = makePylonService({
      runner: async (input) => {
        await input.onStderrLine?.(JSON.stringify(event))
        return {
          exitCode: 0,
          stderr: "",
          stdout: "{}",
          timedOut: false,
        }
      },
    })
    const first = Effect.runPromise(service.lifecycle.pipe(Stream.runHead))
    const second = Effect.runPromise(service.lifecycle.pipe(Stream.runHead))

    const result = await Effect.runPromise(service.request({ args: ["khala", "request", "--json"] }))
    const [firstEvent, secondEvent] = await Promise.all([first, second])

    expect(result.lifecycle).toEqual([event])
    expect(firstEvent._tag).toBe("Some")
    expect(secondEvent._tag).toBe("Some")
    if (firstEvent._tag === "Some") expect(firstEvent.value).toEqual(event)
    if (secondEvent._tag === "Some") expect(secondEvent.value).toEqual(event)
  })

  test("runs one assignment through pylon khala request and maps closeout result", async () => {
    const event = lifecycleEvent("assignment_run.completed")
    const captured: PylonServiceCommandRunnerInput[] = []
    const service = makePylonService({
      env: { OPENAGENTS_PYLON_APP_PATH: "/tmp/pylon-app", PYLON_HOME: "/tmp/pylon-home" },
      runner: async (input) => {
        captured.push(input)
        await input.onStderrLine?.(JSON.stringify(event))
        return {
          exitCode: 0,
          stderr: "",
          stdout: JSON.stringify({
            assignmentRef: "assignment.test",
            assignmentLifecycleEvents: [event],
            assignmentRun: {
              closeout: { status: "accepted" },
              ok: true,
            },
            autoRun: {
              ok: true,
            },
          }),
          timedOut: false,
        }
      },
    })

    const result = await Effect.runPromise(service.runAssignment({
      accountRef: "codex-2",
      baseUrl: "https://openagents.test",
      fixture: true,
      objective: "Run the fixture assignment.",
      pylonRef: "pylon.owner",
      workerKind: "codex",
    }))

    const command = captured[0]
    expect(command).toBeDefined()
    expect(command?.cmd).toContain("src/index.ts")
    expect(command?.cmd).toContain("khala")
    expect(command?.cmd).toContain("request")
    expect(command?.cmd).toContain("--account-ref")
    expect(command?.cmd).toContain("codex-2")
    expect(command?.cmd).toContain("--lifecycle-ndjson")
    expect(command?.env?.PYLON_HOME).toBe("/tmp/pylon-home")
    expect(result).toMatchObject({
      assignmentRef: "assignment.test",
      status: "completed",
    })
    expect(result.lifecycle).toEqual([event])
  })

  test("provides a fixture stub layer for supervisor tests", async () => {
    const service = await Effect.runPromise(PylonService.pipe(Effect.provide(PylonServiceStub())))
    const eventPromise = Effect.runPromise(service.lifecycle.pipe(Stream.runHead))
    const assignment = await Effect.runPromise(service.runAssignment({
      fixture: true,
      objective: "Run a stub assignment.",
    }))
    const event = await eventPromise

    expect(assignment.status).toBe("completed")
    expect(event._tag).toBe("Some")
    if (event._tag === "Some") {
      expect(event.value.schema).toBe("openagents.pylon.assignment_run_lifecycle_event.v0.1")
    }
  })
})

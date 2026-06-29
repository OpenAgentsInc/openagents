import { describe, expect, test } from "bun:test"
import {
  buildLiveWorkerLoopSmokeOptions,
  redactSmokeText,
  runLiveWorkerLoopSmoke,
} from "../src/live-worker-loop-smoke"

const jsonResponse = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), {
    headers: { "Content-Type": "application/json" },
    status,
  })

describe("live worker loop smoke", () => {
  test("builds token-bound options without exposing secrets", () => {
    const options = buildLiveWorkerLoopSmokeOptions(
      {
        OPENAGENTS_AGENT_TOKEN: "oa_agent_secret_123",
        OPENAGENTS_ADMIN_API_TOKEN: "admin-secret",
        OPENAGENTS_BASE_URL: "https://openagents.com",
        PYLON_LIVE_SMOKE_PYLON_REF: "pylon.test.live_smoke",
      },
      new Date("2026-06-09T18:00:00.000Z"),
    )

    expect(options).toMatchObject({
      baseUrl: "https://openagents.com",
      createAssignment: true,
      pylonRef: "pylon.test.live_smoke",
    })
    expect(
      redactSmokeText(
        "Authorization: Bearer oa_agent_secret_123 OPENAGENTS_ADMIN_API_TOKEN=admin-secret",
      ),
    ).toBe("Authorization: Bearer <redacted> OPENAGENTS_ADMIN_API_TOKEN=<redacted>")
  })

  test("runs the full no-spend live contract when admin assignment dispatch is available", async () => {
    const paths: string[] = []
    const bodies: unknown[] = []
    const fetch: typeof globalThis.fetch = async (request, init) => {
      const url = new URL(request instanceof Request ? request.url : String(request))
      paths.push(`${init?.method ?? "GET"} ${url.pathname}`)
      if (typeof init?.body === "string" && init.body.trim()) {
        bodies.push(JSON.parse(init.body))
      }

      if (url.pathname.endsWith("/assignments")) {
        return jsonResponse({
          assignments: [
            {
              assignmentRef: "assignment.public.live_worker_loop_smoke.test",
              state: "offered",
            },
          ],
        })
      }

      return jsonResponse({ ok: true }, url.pathname.includes("/operator/") ? 201 : 200)
    }

    const result = await runLiveWorkerLoopSmoke({
      adminToken: "admin-secret",
      agentToken: "oa_agent_secret_123",
      baseUrl: "https://openagents.com",
      createAssignment: true,
      fetch,
      now: () => new Date("2026-06-09T18:00:00.000Z"),
      pylonRef: "pylon.test.live_smoke",
    })

    expect(result.status).toBe("passed")
    expect(result.blockerRefs).toEqual([])
    expect(result.assignmentRef).toBe("assignment.public.live_worker_loop_smoke.test")
    expect(result.stepRefs).toContain("smoke.pylon.operator_closeout")
    expect(bodies.find((body) => (body as { jobKind?: unknown }).jobKind === "validation")).toMatchObject({
      acceptanceCriteriaRefs: ["acceptance.public.pylon_runtime_gate.bounded_fixture_test_passes"],
      codingAssignment: {
        runtimeGate: {
          fixtureRef: "fixture.public.pylon.codex_runtime.sum_repair.v1",
          schema: "openagents.pylon.runtime_gate.v0.3",
        },
      },
      resultExpectationRefs: ["result.public.pylon_runtime_gate.fixture_repair_passed"],
      taskRefs: ["task.public.pylon_runtime_gate.fixture_repair"],
    })
    expect(paths).toEqual([
      "POST /api/pylons/register",
      "POST /api/pylons/pylon.test.live_smoke/heartbeat",
      "POST /api/pylons/pylon.test.live_smoke/wallet-readiness",
      "POST /api/operator/pylons/assignments",
      "GET /api/pylons/pylon.test.live_smoke/assignments",
      "POST /api/pylons/pylon.test.live_smoke/assignments/assignment.public.live_worker_loop_smoke.test/accept",
      "POST /api/pylons/pylon.test.live_smoke/assignments/assignment.public.live_worker_loop_smoke.test/progress",
      "POST /api/pylons/pylon.test.live_smoke/assignments/assignment.public.live_worker_loop_smoke.test/artifacts",
      "POST /api/operator/pylons/assignments/assignment.public.live_worker_loop_smoke.test/closeout",
    ])
  })

  test("reports partial status when assignment dispatch is intentionally skipped", async () => {
    const fetch: typeof globalThis.fetch = async (request) => {
      const url = new URL(request instanceof Request ? request.url : String(request))

      if (url.pathname.endsWith("/assignments")) {
        return jsonResponse({ assignments: [] })
      }

      return jsonResponse({ ok: true })
    }

    const result = await runLiveWorkerLoopSmoke({
      agentToken: "oa_agent_secret_123",
      baseUrl: "https://openagents.com",
      createAssignment: false,
      fetch,
      now: () => new Date("2026-06-09T18:00:00.000Z"),
      pylonRef: "pylon.test.live_smoke",
    })

    expect(result.status).toBe("partial")
    expect(result.stepRefs).toEqual([
      "smoke.pylon.register",
      "smoke.pylon.heartbeat",
      "smoke.pylon.wallet_readiness",
      "smoke.pylon.assignments_read",
    ])
    expect(result.skippedRefs).toContain("skip.pylon.assignment_create.admin_token_missing")
    expect(result.blockerRefs).toContain("blocker.pylon.live_worker_loop.no_assignment_available")
  })
})

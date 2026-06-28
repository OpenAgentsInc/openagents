import { describe, expect, test } from "bun:test"
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { dirname, join } from "node:path"

import { formatFleetOperatorDashboard, formatKhalaSpawnCapabilityAnswer, runKhalaCli } from "./cli.js"

describe("Khala CLI spawn capability answer", () => {
  test("answers the original subprocess capability question with the reviewed CLI path", () => {
    const answer = formatKhalaSpawnCapabilityAnswer()
    const lower = answer.toLowerCase()

    expect(answer).toContain("Yes.")
    expect(answer).toContain("/spawn 5")
    expect(answer).toContain('khala spawn --count 5 --objective "audit X"')
    expect(lower).toContain("supervised khala child workers")
    expect(lower).toContain("public/browser chat")
    expect(lower).toContain("cannot execute local workers on your machine")
    expect(lower).not.toContain("capability we don't yet expose")
    expect(lower).not.toContain("we do not yet expose")
  })
})

describe("Khala CLI info diagnostics", () => {
  test("does not print raw agent tokens or token-bearing trace URLs", async () => {
    const dir = mkdtempSync(join(tmpdir(), "khala-info-test-"))
    const tokenPath = join(dir, "agent-token")
    mkdirSync(dirname(tokenPath), { recursive: true })
    writeFileSync(tokenPath, "oa_agent_secret_for_info_test\n", { mode: 0o600 })

    const originalWrite = process.stdout.write
    let stdout = ""
    process.stdout.write = ((chunk: string | Uint8Array) => {
      stdout += String(chunk)
      return true
    }) as typeof process.stdout.write
    try {
      const exitCode = await runKhalaCli(["info"], {
        KHALA_TOKEN_PATH: tokenPath,
      })
      expect(exitCode).toBe(0)
    } finally {
      process.stdout.write = originalWrite
    }

    expect(stdout).toContain("Current Khala session:")
    expect(stdout).toContain("agent token configured")
    expect(stdout).toContain("token redacted")
    expect(stdout).not.toContain("oa_agent_secret_for_info_test")
    expect(stdout).not.toContain("token=")
  })

  test("uses the stored login token for --api when no env or flag token is present", async () => {
    const dir = mkdtempSync(join(tmpdir(), "khala-api-token-test-"))
    const tokenPath = join(dir, "agent-token")
    mkdirSync(dirname(tokenPath), { recursive: true })
    writeFileSync(tokenPath, "oa_agent_stored_api_test\n", { mode: 0o600 })

    const authHeaders: Array<string | null> = []
    const server = Bun.serve({
      port: 0,
      fetch: async request => {
        authHeaders.push(request.headers.get("authorization"))
        const body = await request.json() as { readonly messages?: ReadonlyArray<{ readonly content?: string }> }
        const newest = String(body.messages?.at(-1)?.content ?? "")
        const text = newest.includes("Blueprint route selector")
          ? '{"route":"chat","reason":"test"}'
          : "stored api token ok"
        return new Response([
          `data: ${JSON.stringify({ id: "chat_test", model: "openagents/khala", choices: [{ delta: { content: text } }] })}`,
          'data: {"id":"chat_test","model":"openagents/khala","choices":[],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}',
          "data: [DONE]",
          "",
        ].join("\n\n"), {
          headers: { "content-type": "text/event-stream" },
        })
      },
    })

    const originalWrite = process.stdout.write
    let stdout = ""
    process.stdout.write = ((chunk: string | Uint8Array) => {
      stdout += String(chunk)
      return true
    }) as typeof process.stdout.write
    try {
      const exitCode = await runKhalaCli([
        "--api",
        "--headless",
        "--json",
        "--base-url",
        `http://127.0.0.1:${server.port}`,
        "--prompt",
        "hello",
      ], {
        KHALA_TOKEN_PATH: tokenPath,
        KHALA_CODEX_AUTO: "off",
      })
      expect(exitCode).toBe(0)
    } finally {
      process.stdout.write = originalWrite
      server.stop(true)
    }

    expect(authHeaders).toContain("Bearer oa_agent_stored_api_test")
    expect(JSON.parse(stdout).text).toBe("stored api token ok")
  })

  test("uses the stored login token for pylon spawn when no env or flag token is present", async () => {
    const dir = mkdtempSync(join(tmpdir(), "khala-pylon-token-test-"))
    const tokenPath = join(dir, "agent-token")
    mkdirSync(dirname(tokenPath), { recursive: true })
    writeFileSync(tokenPath, "oa_agent_stored_pylon_test\n", { mode: 0o600 })

    const authHeaders: Array<string | null> = []
    const toolNames: Array<string> = []
    const server = Bun.serve({
      port: 0,
      fetch: async request => {
        authHeaders.push(request.headers.get("authorization"))
        const body = await request.json() as {
          readonly id?: string
          readonly params?: {
            readonly arguments?: Record<string, unknown>
            readonly name?: string
          }
        }
        toolNames.push(body.params?.name ?? "")
        expect(body.params?.arguments?.objective).toBe("stored pylon token smoke")
        return Response.json({
          jsonrpc: "2.0",
          id: body.id,
          result: {
            structuredContent: {
              ok: true,
              spawnRef: "spawn.public.khala_coding.stored_pylon_token_test",
              requestedCount: 1,
              assignedCount: 1,
              children: [{
                ok: true,
                workerRef: "worker.public.khala_coding.spawn.01",
                slotIndex: 0,
                assignmentRef: "assignment.public.khala_coding.stored_pylon_token_test",
                durableRequestId: "chatcmpl_stored_pylon_token_test",
                pylonRef: "pylon.owner.codex",
                state: "accepted",
              }],
            },
          },
        })
      },
    })

    const originalWrite = process.stdout.write
    let stdout = ""
    process.stdout.write = ((chunk: string | Uint8Array) => {
      stdout += String(chunk)
      return true
    }) as typeof process.stdout.write
    try {
      const exitCode = await runKhalaCli([
        "--json",
        "spawn",
        "--strategy",
        "pylon",
        "--count",
        "1",
        "--fixture",
        "--base-url",
        `http://127.0.0.1:${server.port}`,
        "--objective",
        "stored pylon token smoke",
      ], {
        KHALA_TOKEN_PATH: tokenPath,
      })
      expect(exitCode).toBe(0)
    } finally {
      process.stdout.write = originalWrite
      server.stop(true)
    }

    expect(authHeaders).toContain("Bearer oa_agent_stored_pylon_test")
    expect(toolNames).toEqual(["khala.spawn"])
    const parsed = JSON.parse(stdout)
    expect(parsed.runRef).toBe("spawn.public.khala_coding.stored_pylon_token_test")
    expect(parsed.state).toBe("completed")
  })
})

describe("Khala CLI fleet live status", () => {
  test("polls the operator fleet endpoint once under test and renders the five dashboard blocks", async () => {
    const authHeaders: Array<string | null> = []
    const server = Bun.serve({
      port: 0,
      fetch: request => {
        authHeaders.push(request.headers.get("authorization"))
        expect(new URL(request.url).pathname).toBe("/api/operator/fleet/status")
        return Response.json({
          generatedAt: "2026-06-27T12:00:00.000Z",
          blocks: {
            pace: { burnRateTokensPerMinute: 1200, paceToFloor: "ahead" },
            fleet: { concurrency: 5, inFlightIssues: 3 },
            watchdog: { state: "healthy", leases: 2, alerts: 0 },
            glm: { readiness: "ready", replicas: 10 },
            artanis: { loopHealth: "moving", goals: 4, recentDecisions: 7 },
          },
        })
      },
    })

    const originalWrite = process.stdout.write
    let stdout = ""
    process.stdout.write = ((chunk: string | Uint8Array) => {
      stdout += String(chunk)
      return true
    }) as typeof process.stdout.write
    try {
      const exitCode = await runKhalaCli([
        "fleet",
        "status",
        "--live",
        "--base-url",
        `http://127.0.0.1:${server.port}`,
      ], {
        OPENAGENTS_AGENT_TOKEN: "oa_agent_live_status_test",
        KHALA_FLEET_STATUS_LIVE_ONCE: "1",
      })
      expect(exitCode).toBe(0)
    } finally {
      process.stdout.write = originalWrite
      server.stop(true)
    }

    expect(authHeaders).toEqual(["Bearer oa_agent_live_status_test"])
    expect(stdout).toContain("Khala fleet live")
    expect(stdout).toContain("Pace")
    expect(stdout).toContain("Fleet")
    expect(stdout).toContain("Watchdog")
    expect(stdout).toContain("GLM")
    expect(stdout).toContain("Artanis")
    expect(stdout).toContain("burn rate tokens per minute")
    expect(stdout).toContain("in flight issues")
  })

  test("formats a representative operator payload as stable terminal blocks", () => {
    const rendered = formatFleetOperatorDashboard({
      generatedAt: "2026-06-27T12:00:00.000Z",
      raw: {
        pace: { burnRate: 42, paceToFloor: "on-track" },
        fleet: { concurrency: 6, spread: "codex:4 codex-2:2" },
        watchdog: { state: "watching", alerts: 0 },
        glm: { readiness: "ready" },
        brain: { loopHealth: "active" },
      },
    })

    expect(rendered).toContain("source: /api/operator/fleet/status")
    expect(rendered).toContain("Pace")
    expect(rendered).toContain("Fleet")
    expect(rendered).toContain("Watchdog")
    expect(rendered).toContain("GLM")
    expect(rendered).toContain("Artanis")
    expect(rendered).toContain("polling every ~5s")
  })
})

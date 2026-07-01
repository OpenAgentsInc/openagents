import { describe, expect, test } from "bun:test"
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { dirname, join } from "node:path"
import { khalaSessionModelItems, readKhalaSessionRollout } from "@openagentsinc/khala-tools"

import { formatKhalaSpawnCapabilityAnswer, runKhalaCli } from "./cli.js"
import { normalizeTerminalMarkdownSpacing } from "./terminal.js"

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

describe("Khala CLI terminal rendering", () => {
  test("collapses excessive blank lines in streamed answers", () => {
    expect(normalizeTerminalMarkdownSpacing("A\n\n\n\nB\r\n\r\n\r\nC")).toBe("A\n\nB\n\nC")
  })
})

describe("Khala CLI info diagnostics", () => {
  test("renders the friendly fleet status onboarding text when no accounts are connected", async () => {
    const dir = mkdtempSync(join(tmpdir(), "khala-fleet-status-empty-test-"))

    const originalWrite = process.stdout.write
    let stdout = ""
    process.stdout.write = ((chunk: string | Uint8Array) => {
      stdout += String(chunk)
      return true
    }) as typeof process.stdout.write
    try {
      const exitCode = await runKhalaCli(["fleet", "status"], {
        PYLON_HOME: join(dir, ".openagents", "pylon"),
      })
      expect(exitCode).toBe(0)
    } finally {
      process.stdout.write = originalWrite
    }

    expect(stdout).toContain("Khala fleet:")
    expect(stdout).toContain("No harness accounts connected yet.")
    expect(stdout).toContain("khala fleet connect")
    expect(stdout).toContain("khala fleet connect --harness claude")
  })

  test("lists connected fleet accounts and readiness through the CLI alias", async () => {
    const dir = mkdtempSync(join(tmpdir(), "khala-fleet-status-test-"))
    const pylonHome = join(dir, ".openagents", "pylon")
    const codexHome = join(pylonHome, "accounts", "codex", "codex")
    const claudeHome = join(pylonHome, "accounts", "claude_agent", ".claude-claude")
    mkdirSync(codexHome, { recursive: true })
    mkdirSync(claudeHome, { recursive: true })
    writeFileSync(
      join(codexHome, "auth.json"),
      JSON.stringify({
        tokens: {
          id_token: [
            Buffer.from(JSON.stringify({ alg: "none" })).toString("base64url"),
            Buffer.from(JSON.stringify({ email: "fleet@example.com" })).toString("base64url"),
            "",
          ].join("."),
        },
      }),
    )
    writeFileSync(join(claudeHome, "claude-oauth-token"), "sk-ant-oat-cli-status\n")
    writeFileSync(
      join(pylonHome, "config.json"),
      JSON.stringify({
        dev: {
          accounts: [
            { ref: "codex", provider: "codex", home: codexHome },
            { ref: "codex-2", provider: "codex", home: join(pylonHome, "accounts", "codex", "codex-2") },
            { ref: "claude", provider: "claude_agent", home: claudeHome },
          ],
        },
      }),
    )

    const originalWrite = process.stdout.write
    let stdout = ""
    process.stdout.write = ((chunk: string | Uint8Array) => {
      stdout += String(chunk)
      return true
    }) as typeof process.stdout.write
    try {
      const exitCode = await runKhalaCli(["fleet", "list"], {
        PYLON_HOME: pylonHome,
      })
      expect(exitCode).toBe(0)
    } finally {
      process.stdout.write = originalWrite
    }

    expect(stdout).toContain("3 account(s), 2 ready")
    expect(stdout).toContain("ACCOUNT")
    expect(stdout).toContain("HARNESS")
    expect(stdout).toContain("READINESS")
    expect(stdout).toContain("EMAIL")
    expect(stdout).toContain("codex")
    expect(stdout).toContain("claude")
    expect(stdout).toContain("ready")
    expect(stdout).toContain("fleet@example.com")
    expect(stdout).toContain("codex-2")
    expect(stdout).toContain("credentials-missing")
  })

  test("routes Artanis approval-gate status through the owner console endpoint", async () => {
    const authHeaders: Array<string | null> = []
    const server = Bun.serve({
      port: 0,
      fetch: request => {
        authHeaders.push(request.headers.get("authorization"))
        expect(new URL(request.url).pathname).toBe("/api/operator/artanis/console")
        return Response.json({
          ledgerRef: "ledger.public.artanis.approval_gates",
          gateCount: 1,
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
        "--base-url",
        `http://127.0.0.1:${server.port}`,
        "artanis",
        "status",
      ], {
        OPENAGENTS_AGENT_TOKEN: "oa_agent_artanis_status_test",
      })
      expect(exitCode).toBe(0)
    } finally {
      process.stdout.write = originalWrite
      server.stop(true)
    }

    expect(authHeaders).toEqual(["Bearer oa_agent_artanis_status_test"])
    expect(JSON.parse(stdout).gateCount).toBe(1)
  })

  test("arms Artanis pylon dispatch through the approval-gate create endpoint", async () => {
    const requests: Array<{ body: unknown; path: string }> = []
    const server = Bun.serve({
      port: 0,
      fetch: async request => {
        requests.push({
          body: await request.json(),
          path: new URL(request.url).pathname,
        })
        expect(request.headers.get("authorization")).toBe("Bearer oa_agent_artanis_arm_test")
        return Response.json({
          ok: true,
          armedGate: {
            expiresAtDisplay: "soon",
            gateRef: "gate.public.artanis.arm_pylon_dispatch.test",
            kind: "pylon_job_dispatch",
            state: "approved",
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
        "--expires-hours",
        "12",
        "--base-url",
        `http://127.0.0.1:${server.port}`,
        "artanis",
        "arm-pylon-dispatch",
      ], {
        OPENAGENTS_AGENT_TOKEN: "oa_agent_artanis_arm_test",
      })
      expect(exitCode).toBe(0)
    } finally {
      process.stdout.write = originalWrite
      server.stop(true)
    }

    expect(requests).toEqual([{
      body: { expiresInHours: 12 },
      path: "/api/operator/artanis/approval-gates",
    }])
    expect(JSON.parse(stdout).armedGate.gateRef).toBe("gate.public.artanis.arm_pylon_dispatch.test")
  })

  test("approves a pending Artanis gate through the gated decision endpoint", async () => {
    const paths: Array<string> = []
    const server = Bun.serve({
      port: 0,
      fetch: request => {
        paths.push(new URL(request.url).pathname)
        expect(request.method).toBe("POST")
        expect(request.headers.get("authorization")).toBe("Bearer oa_agent_artanis_approve_test")
        return Response.json({
          ledgerRef: "ledger.public.artanis.approval_gates",
          gateCount: 2,
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
        "--base-url",
        `http://127.0.0.1:${server.port}`,
        "artanis",
        "approve",
        "gate.public.artanis.pending_test",
      ], {
        OPENAGENTS_AGENT_TOKEN: "oa_agent_artanis_approve_test",
      })
      expect(exitCode).toBe(0)
    } finally {
      process.stdout.write = originalWrite
      server.stop(true)
    }

    expect(paths).toEqual([
      "/api/operator/artanis/approval-gates/gate.public.artanis.pending_test/approve",
    ])
    expect(JSON.parse(stdout).gateCount).toBe(2)
  })

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

  test("renders the live fleet dashboard from the operator status endpoint", async () => {
    const authHeaders: Array<string | null> = []
    const server = Bun.serve({
      port: 0,
      fetch: request => {
        authHeaders.push(request.headers.get("authorization"))
        expect(new URL(request.url).pathname).toBe("/api/operator/fleet/state")
        return Response.json({
          pace: { burnRate: "900 tokens/min", paceToFloor: "on pace" },
          fleet: { concurrency: 2, inFlightIssues: ["#6429"] },
          watchdog: { state: "healthy", leases: 1 },
          glm: { status: "ready", readyReplicas: 8, totalReplicas: 8 },
          brain: { state: "running", goals: 3 },
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
        OPENAGENTS_AGENT_TOKEN: "oa_agent_live_dashboard_test",
        KHALA_FLEET_LIVE_MAX_POLLS: "1",
      })
      expect(exitCode).toBe(0)
    } finally {
      process.stdout.write = originalWrite
      server.stop(true)
    }

    expect(authHeaders).toEqual(["Bearer oa_agent_live_dashboard_test"])
    expect(stdout).toContain("Khala fleet live dashboard")
    expect(stdout).toContain("[Pace]")
    expect(stdout).toContain("burnRate: 900 tokens/min")
    expect(stdout).toContain("[Fleet]")
    expect(stdout).toContain("[Watchdog]")
    expect(stdout).toContain("[GLM]")
    expect(stdout).toContain("[Brain]")
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

  test("persists headless sessions and resumes or forks them with prior context", async () => {
    const dir = mkdtempSync(join(tmpdir(), "khala-session-cli-test-"))
    const sessionDir = join(dir, "state")
    const requests: Array<ReadonlyArray<{ readonly content?: string; readonly role?: string }>> = []
    const server = Bun.serve({
      port: 0,
      fetch: async request => {
        expect(new URL(request.url).pathname).toBe("/api/khala/chat")
        const body = await request.json() as { readonly messages?: ReadonlyArray<{ readonly content?: string; readonly role?: string }> }
        requests.push(body.messages ?? [])
        const newest = String(body.messages?.at(-1)?.content ?? "")
        const text = `answer:${newest}`
        return new Response([
          `event: delta\ndata: ${JSON.stringify({ text })}`,
          'event: done\ndata: {"done":true}',
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
      const firstExit = await runKhalaCli([
        "--headless",
        "--json",
        "--base-url",
        `http://127.0.0.1:${server.port}`,
        "--session-dir",
        sessionDir,
        "--prompt",
        "first",
      ], { KHALA_CODEX_AUTO: "off" })
      expect(firstExit).toBe(0)
      const first = JSON.parse(stdout) as { readonly sessionId: string; readonly text: string }
      expect(first.text).toBe("answer:first")

      stdout = ""
      const resumeExit = await runKhalaCli([
        "--json",
        "--base-url",
        `http://127.0.0.1:${server.port}`,
        "--session-dir",
        sessionDir,
        "--prompt",
        "second",
        "resume",
        first.sessionId,
      ], { KHALA_CODEX_AUTO: "off" })
      expect(resumeExit).toBe(0)
      const resumed = JSON.parse(stdout) as { readonly sessionId: string; readonly text: string }
      expect(resumed.sessionId).toBe(first.sessionId)
      expect(resumed.text).toBe("answer:second")

      stdout = ""
      const forkExit = await runKhalaCli([
        "--json",
        "--base-url",
        `http://127.0.0.1:${server.port}`,
        "--session-dir",
        sessionDir,
        "fork",
        first.sessionId,
      ], { KHALA_CODEX_AUTO: "off" })
      expect(forkExit).toBe(0)
      const forked = JSON.parse(stdout) as { readonly parentSessionId: string; readonly sessionId: string }
      expect(forked.parentSessionId).toBe(first.sessionId)
      expect(forked.sessionId).not.toBe(first.sessionId)

      const loaded = await readKhalaSessionRollout(sessionDir, first.sessionId)
      expect(khalaSessionModelItems(loaded).map(item => `${item.role}:${item.body}`)).toEqual([
        "user:first",
        "assistant:answer:first",
        "user:second",
        "assistant:answer:second",
      ])
    } finally {
      process.stdout.write = originalWrite
      server.stop(true)
    }

    expect(requests[1]).toEqual([
      { content: "first", role: "user" },
      { content: "answer:first", role: "assistant" },
      { content: "second", role: "user" },
    ])
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

  test("surfaces pylon spawn server validation errors instead of reporting network failure", async () => {
    const server = Bun.serve({
      port: 0,
      fetch: () =>
        Response.json({
          error: "pylon_api_validation_error",
          message: "Missing key [\"jobKind\"] after network reached server",
        }, { status: 400 }),
    })

    const originalWrite = process.stderr.write
    let stderr = ""
    process.stderr.write = ((chunk: string | Uint8Array) => {
      stderr += String(chunk)
      return true
    }) as typeof process.stderr.write
    try {
      const exitCode = await runKhalaCli([
        "spawn",
        "--strategy",
        "pylon",
        "--count",
        "1",
        "--fixture",
        "--base-url",
        `http://127.0.0.1:${server.port}`,
        "--objective",
        "validation smoke",
      ], {
        OPENAGENTS_AGENT_TOKEN: "oa_agent_pylon_validation_test",
      })
      expect(exitCode).toBe(1)
    } finally {
      process.stderr.write = originalWrite
      server.stop(true)
    }

    expect(stderr).toContain("remote khala.spawn failed (400)")
    expect(stderr).toContain("pylon_api_validation_error")
    expect(stderr).not.toContain("Could not reach Khala")
  })

  test("accepts claude_agent_task workflow for pylon spawn", async () => {
    const dir = mkdtempSync(join(tmpdir(), "khala-pylon-claude-workflow-test-"))
    const tokenPath = join(dir, "agent-token")
    mkdirSync(dirname(tokenPath), { recursive: true })
    writeFileSync(tokenPath, "oa_agent_stored_pylon_test\n", { mode: 0o600 })

    const workflows: Array<unknown> = []
    const server = Bun.serve({
      port: 0,
      fetch: async request => {
        const body = await request.json() as {
          readonly id?: string
          readonly params?: {
            readonly arguments?: Record<string, unknown>
          }
        }
        workflows.push(body.params?.arguments?.workflow)
        return Response.json({
          jsonrpc: "2.0",
          id: body.id,
          result: {
            structuredContent: {
              ok: true,
              spawnRef: "spawn.public.khala_coding.claude_workflow_test",
              requestedCount: 1,
              assignedCount: 1,
              children: [{
                ok: true,
                workerRef: "worker.public.khala_coding.spawn.01",
                slotIndex: 0,
                assignmentRef: "assignment.public.khala_coding.claude_workflow_test",
                durableRequestId: "chatcmpl_claude_workflow_test",
                pylonRef: "pylon.owner.claude",
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
        "--workflow",
        "claude_agent_task",
        "--count",
        "1",
        "--fixture",
        "--base-url",
        `http://127.0.0.1:${server.port}`,
        "--objective",
        "stored pylon claude workflow smoke",
      ], {
        KHALA_TOKEN_PATH: tokenPath,
      })
      expect(exitCode).toBe(0)
    } finally {
      process.stdout.write = originalWrite
      server.stop(true)
    }

    expect(workflows).toContain("claude_agent_task")
    expect(JSON.parse(stdout).state).toBe("completed")
  })
})

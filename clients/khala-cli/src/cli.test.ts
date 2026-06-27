import { describe, expect, test } from "bun:test"
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { dirname, join } from "node:path"

import { formatKhalaSpawnCapabilityAnswer, runKhalaCli } from "./cli.js"

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
  test("plans khala fleet run with stored fleet accounts and JSON output", async () => {
    const dir = mkdtempSync(join(tmpdir(), "khala-fleet-run-cli-"))
    const pylonHome = join(dir, "pylon")
    const accountHome = join(pylonHome, "accounts", "codex", "codex")
    mkdirSync(accountHome, { recursive: true })
    const header = Buffer.from(JSON.stringify({ alg: "none" })).toString("base64url")
    const payload = Buffer.from(JSON.stringify({ email: "fleet-cli@example.com" })).toString("base64url")
    writeFileSync(join(accountHome, "auth.json"), JSON.stringify({ tokens: { id_token: `${header}.${payload}.` } }))
    writeFileSync(
      join(pylonHome, "config.json"),
      JSON.stringify({ dev: { accounts: [{ ref: "codex", provider: "codex", home: accountHome }] } }),
    )
    const pylonScript = join(dir, "fake-pylon.js")
    writeFileSync(
      pylonScript,
      "if (process.argv.slice(2).join(' ') === 'provider go-online --json') console.log(JSON.stringify({pylonRef:'pylon.cli.test'})); else process.exit(2);\n",
    )

    const originalWrite = process.stdout.write
    let stdout = ""
    process.stdout.write = ((chunk: string | Uint8Array) => {
      stdout += String(chunk)
      return true
    }) as typeof process.stdout.write
    try {
      const exitCode = await runKhalaCli([
        "--json",
        "fleet",
        "run",
        "--dry-run",
        "--repo",
        "ExampleCo/example",
        "--commit",
        "0123456789abcdef0123456789abcdef01234567",
        "--verify",
        "bun test",
        "--issue",
        "6384",
      ], {
        KHALA_PYLON_COMMAND: `node ${pylonScript}`,
        PYLON_HOME: pylonHome,
      })
      expect(exitCode).toBe(0)
    } finally {
      process.stdout.write = originalWrite
    }

    const parsed = JSON.parse(stdout)
    expect(parsed.schema).toBe("openagents.khala.fleet_run.v0.1")
    expect(parsed.dryRun).toBe(true)
    expect(parsed.plan.pylonRef).toBe("pylon.cli.test")
    expect(parsed.plan.issues).toEqual([6384])
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

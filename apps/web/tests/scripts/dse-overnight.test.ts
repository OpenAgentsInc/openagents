import { describe, expect, it } from "vitest"

import { parseOvernightArgs, runOvernight } from "../../scripts/dse-overnight-lib"

const jsonOk = (body: unknown) =>
  new Response(JSON.stringify({ ok: true, ...(body as any) }), {
    status: 200,
    headers: { "content-type": "application/json; charset=utf-8" },
  })

describe("apps/web/scripts/dse-overnight", () => {
  it("parses defaults: localhost => verify on, e2e off", () => {
    const parsed = parseOvernightArgs(["--base-url", "http://localhost:3000"])
    expect(parsed.ok).toBe(true)
    if (!parsed.ok) return
    expect(parsed.options.verify).toBe(true)
    expect(parsed.options.runE2e).toBe(false)
  })

  it("parses defaults: openagents.com => verify off, e2e on", () => {
    const parsed = parseOvernightArgs(["--base-url", "https://openagents.com"])
    expect(parsed.ok).toBe(true)
    if (!parsed.ok) return
    expect(parsed.options.verify).toBe(false)
    expect(parsed.options.runE2e).toBe(true)
  })

  it("emits start -> events -> finish (success path)", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []

    const fetchFn = async (url: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
      calls.push({ url: String(url), init })
      if (String(url).endsWith("/api/dse/ops/run/start")) {
        return jsonOk({ existed: false, runId: "opsrun_1" })
      }
      if (String(url).endsWith("/api/dse/ops/run/event")) {
        return jsonOk({})
      }
      if (String(url).endsWith("/api/dse/ops/run/finish")) {
        return jsonOk({})
      }
      if (String(url).endsWith("/api/dse/examples/import")) {
        return jsonOk({ total: 2, inserted: 1, updated: 1 })
      }
      if (String(url).endsWith("/api/dse/compile")) {
        return jsonOk({
          signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
          jobHash: "sha256:job",
          datasetHash: "sha256:ds",
          datasetId: "convex:dseExamples:@openagents/autopilot/blueprint/SelectTool.v1",
          compiled_id: "sha256:compiled",
          existed: false,
        })
      }
      if (String(url).endsWith("/api/dse/exercise/thread/ensure")) {
        return jsonOk({ threadId: "thread_ops" })
      }
      if (String(url).endsWith("/api/dse/canary/start")) {
        return jsonOk({
          signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
          control_compiled_id: "sha256:control",
          canary_compiled_id: "sha256:compiled",
          rolloutPct: 20,
        })
      }
      if (String(url).endsWith("/api/dse/exercise/predict")) {
        return jsonOk({ signatureId: "@openagents/autopilot/blueprint/SelectTool.v1", threadId: "thread_ops", runId: "run_x", count: 20, okCount: 20, errorCount: 0, receiptIds: [] })
      }
      if (String(url).includes("/api/dse/canary/status")) {
        return jsonOk({
          canary: {
            signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
            enabled: true,
            control_compiled_id: "sha256:control",
            canary_compiled_id: "sha256:compiled",
            rolloutPct: 20,
            salt: "salt",
            okCount: 20,
            errorCount: 0,
            minSamples: 20,
            maxErrorRate: 0.2,
            createdAtMs: 1,
            updatedAtMs: 2,
          },
        })
      }
      if (String(url).endsWith("/api/dse/promote")) {
        return jsonOk({ signatureId: "@openagents/autopilot/blueprint/SelectTool.v1", from: "sha256:control", to: "sha256:compiled" })
      }
      if (String(url).endsWith("/api/dse/canary/stop")) {
        return jsonOk({ existed: true })
      }
      return new Response("not found", { status: 404 })
    }

    const runCommand = async () => ({
      ok: true,
      code: 0,
      stdout: "d2abf1187\n",
      stderr: "",
      durationMs: 1,
      timedOut: false,
    })

    const parsed = parseOvernightArgs(["--base-url", "https://example.com", "--no-verify", "--no-e2e"])
    expect(parsed.ok).toBe(true)
    if (!parsed.ok) return

    const summary = await runOvernight({
      options: parsed.options,
      env: { OA_DSE_ADMIN_SECRET: "secret" },
      fetchFn,
      runCommand,
      readTextFile: async () =>
        [
          JSON.stringify({
            exampleId: "ex1",
            inputJson: { message: "Hi", blueprintHint: { userHandle: "Ada", agentName: "Autopilot" } },
            expectedJson: { action: "none" },
            split: "train",
            tags: ["dataset:test"],
          }),
          JSON.stringify({
            exampleId: "ex2",
            inputJson: { message: "Call me Ada", blueprintHint: { userHandle: "Unknown", agentName: "Autopilot" } },
            expectedJson: { action: "tool", toolName: "user_update" },
            split: "holdout",
            tags: ["dataset:test"],
          }),
        ].join("\n"),
    })

    expect(summary.ok).toBe(true)
    expect(summary.runId).toBe("opsrun_1")

    const paths = calls.map((c) => new URL(c.url).pathname)
    expect(paths[0]).toBe("/api/dse/ops/run/start")
    expect(paths).toContain("/api/dse/ops/run/event")
    expect(paths).toContain("/api/dse/examples/import")
    expect(paths).toContain("/api/dse/compile")
    expect(paths).toContain("/api/dse/canary/start")
    expect(paths).toContain("/api/dse/promote")
    expect(paths[paths.length - 1]).toBe("/api/dse/ops/run/finish")

    // Ensure auth header is attached.
    for (const c of calls) {
      const h = (c.init?.headers ?? {}) as any
      expect(String(h.authorization ?? "")).toContain("Bearer secret")
    }
  })

  it("invokes prod e2e using the configured --e2e-grep value", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = []
    const cmdCalls: Array<{ cwd: string; command: string; args: string[]; env?: Record<string, string | undefined> }> = []

    const fetchFn = async (url: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
      calls.push({ url: String(url), init })
      if (String(url).endsWith("/api/dse/ops/run/start")) {
        return jsonOk({ existed: false, runId: "opsrun_e2e" })
      }
      if (String(url).endsWith("/api/dse/ops/run/event")) {
        return jsonOk({})
      }
      if (String(url).endsWith("/api/dse/ops/run/finish")) {
        return jsonOk({})
      }
      if (String(url).endsWith("/api/dse/examples/import")) {
        return jsonOk({ total: 2, inserted: 1, updated: 1 })
      }
      if (String(url).endsWith("/api/dse/compile")) {
        return jsonOk({
          signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
          jobHash: "sha256:job",
          datasetHash: "sha256:ds",
          datasetId: "convex:dseExamples:@openagents/autopilot/blueprint/SelectTool.v1",
          compiled_id: "sha256:compiled",
          existed: false,
        })
      }
      if (String(url).endsWith("/api/dse/exercise/thread/ensure")) {
        return jsonOk({ threadId: "thread_ops" })
      }
      if (String(url).endsWith("/api/dse/canary/start")) {
        return jsonOk({
          signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
          control_compiled_id: "sha256:control",
          canary_compiled_id: "sha256:compiled",
          rolloutPct: 20,
        })
      }
      if (String(url).endsWith("/api/dse/exercise/predict")) {
        return jsonOk({
          signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
          threadId: "thread_ops",
          runId: "run_x",
          count: 20,
          okCount: 20,
          errorCount: 0,
          receiptIds: [],
        })
      }
      if (String(url).includes("/api/dse/canary/status")) {
        return jsonOk({
          canary: {
            signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
            enabled: true,
            control_compiled_id: "sha256:control",
            canary_compiled_id: "sha256:compiled",
            rolloutPct: 20,
            salt: "salt",
            okCount: 20,
            errorCount: 0,
            minSamples: 20,
            maxErrorRate: 0.2,
            createdAtMs: 1,
            updatedAtMs: 2,
          },
        })
      }
      if (String(url).endsWith("/api/dse/promote")) {
        return jsonOk({ signatureId: "@openagents/autopilot/blueprint/SelectTool.v1", from: "sha256:control", to: "sha256:compiled" })
      }
      if (String(url).endsWith("/api/dse/canary/stop")) {
        return jsonOk({ existed: true })
      }
      return new Response("not found", { status: 404 })
    }

    const runCommand = async ({ cwd, command, args, env }: any) => {
      cmdCalls.push({ cwd, command, args, env })
      if (command === "git") {
        return { ok: true, code: 0, stdout: "sha\n", stderr: "", durationMs: 1, timedOut: false }
      }
      return { ok: true, code: 0, stdout: "ok\n", stderr: "", durationMs: 1, timedOut: false }
    }

    const parsed = parseOvernightArgs(["--base-url", "https://example.com", "--no-verify", "--e2e", "--e2e-grep", "mygrep"])
    expect(parsed.ok).toBe(true)
    if (!parsed.ok) return

    const summary = await runOvernight({
      options: parsed.options,
      env: { OA_DSE_ADMIN_SECRET: "secret", EFFUSE_TEST_E2E_BYPASS_SECRET: "e2e" },
      fetchFn,
      runCommand,
      readTextFile: async () =>
        [
          JSON.stringify({
            exampleId: "ex1",
            inputJson: { message: "Hi", blueprintHint: { userHandle: "Ada", agentName: "Autopilot" } },
            expectedJson: { action: "none" },
            split: "train",
            tags: ["dataset:test"],
          }),
          JSON.stringify({
            exampleId: "ex2",
            inputJson: { message: "Call me Ada", blueprintHint: { userHandle: "Unknown", agentName: "Autopilot" } },
            expectedJson: { action: "tool", toolName: "user_update" },
            split: "holdout",
            tags: ["dataset:test"],
          }),
        ].join("\n"),
    })

    expect(summary.ok).toBe(true)

    const e2e = cmdCalls.find((c) => c.command === "npm" && c.args.includes("test:e2e"))
    expect(e2e).toBeTruthy()
    expect(e2e?.args).toContain("--grep")
    const grepIdx = e2e?.args.findIndex((a) => a === "--grep") ?? -1
    expect(grepIdx).toBeGreaterThanOrEqual(0)
    expect(e2e?.args[grepIdx + 1]).toBe("mygrep")
    expect(e2e?.env?.EFFUSE_TEST_E2E_BYPASS_SECRET).toBe("e2e")

    // Ensure auth header is attached.
    for (const c of calls) {
      const h = (c.init?.headers ?? {}) as any
      expect(String(h.authorization ?? "")).toContain("Bearer secret")
    }
  })

  it("still finishes the ops run when a verify step fails", async () => {
    const calls: Array<{ url: string; body: any }> = []

    const fetchFn = async (url: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
      const bodyText = typeof init?.body === "string" ? init.body : ""
      const body = bodyText ? JSON.parse(bodyText) : null
      calls.push({ url: String(url), body })
      if (String(url).endsWith("/api/dse/ops/run/start")) {
        return jsonOk({ existed: false, runId: "opsrun_2" })
      }
      if (String(url).endsWith("/api/dse/ops/run/event")) {
        return jsonOk({})
      }
      if (String(url).endsWith("/api/dse/ops/run/finish")) {
        return jsonOk({})
      }
      return new Response("not found", { status: 404 })
    }

    let n = 0
    const runCommand = async () => {
      n++
      if (n === 1) {
        // git rev-parse
        return { ok: true, code: 0, stdout: "sha\n", stderr: "", durationMs: 1, timedOut: false }
      }
      // First verify command fails.
      return { ok: false, code: 1, stdout: "", stderr: "fail", durationMs: 1, timedOut: false }
    }

    const parsed = parseOvernightArgs(["--base-url", "http://localhost:3000", "--verify", "--no-e2e"])
    expect(parsed.ok).toBe(true)
    if (!parsed.ok) return

    const summary = await runOvernight({
      options: parsed.options,
      env: { OA_DSE_ADMIN_SECRET: "secret" },
      fetchFn,
      runCommand,
    })

    expect(summary.ok).toBe(false)

    const finishCall = calls.find((c) => new URL(c.url).pathname === "/api/dse/ops/run/finish")
    expect(finishCall).toBeTruthy()
    expect(finishCall?.body?.status).toBe("failed")
    expect(finishCall?.body?.runId).toBe("opsrun_2")
  })

  it("returns a machine-readable failure summary when ops run start fails", async () => {
    const fetchFn = async (url: RequestInfo | URL): Promise<Response> => {
      if (String(url).endsWith("/api/dse/ops/run/start")) {
        return new Response(JSON.stringify({ ok: false, error: "boom" }), {
          status: 500,
          headers: { "content-type": "application/json; charset=utf-8", "x-oa-request-id": "req_start_fail" },
        })
      }
      return new Response("not found", { status: 404 })
    }

    const runCommand = async () => ({
      ok: true,
      code: 0,
      stdout: "sha\n",
      stderr: "",
      durationMs: 1,
      timedOut: false,
    })

    const parsed = parseOvernightArgs(["--base-url", "https://example.com", "--no-verify", "--no-e2e"])
    expect(parsed.ok).toBe(true)
    if (!parsed.ok) return

    const summary = await runOvernight({
      options: parsed.options,
      env: { OA_DSE_ADMIN_SECRET: "secret" },
      fetchFn,
      runCommand,
    })

    expect(summary.ok).toBe(false)
    expect(summary.runId).toBe("")
    expect(summary.errors.join("\n")).toContain("req=req_start_fail")
  })

  it("stops canary and finishes the ops run when phase 5 promote fails", async () => {
    const calls: Array<{ url: string; body: any }> = []

    const fetchFn = async (url: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
      const bodyText = typeof init?.body === "string" ? init.body : ""
      const body = bodyText ? JSON.parse(bodyText) : null
      calls.push({ url: String(url), body })

      if (String(url).endsWith("/api/dse/ops/run/start")) return jsonOk({ existed: false, runId: "opsrun_3" })
      if (String(url).endsWith("/api/dse/ops/run/event")) return jsonOk({})
      if (String(url).endsWith("/api/dse/ops/run/finish")) return jsonOk({})

      if (String(url).endsWith("/api/dse/examples/import")) return jsonOk({ total: 2, inserted: 2, updated: 0 })
      if (String(url).endsWith("/api/dse/compile"))
        return jsonOk({
          signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
          jobHash: "sha256:job",
          datasetHash: "sha256:ds",
          datasetId: "convex:dseExamples:@openagents/autopilot/blueprint/SelectTool.v1",
          compiled_id: "sha256:compiled",
          existed: false,
        })
      if (String(url).endsWith("/api/dse/exercise/thread/ensure")) return jsonOk({ threadId: "thread_ops" })
      if (String(url).endsWith("/api/dse/canary/start"))
        return jsonOk({
          signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
          control_compiled_id: "sha256:control",
          canary_compiled_id: "sha256:compiled",
          rolloutPct: 20,
        })
      if (String(url).endsWith("/api/dse/exercise/predict"))
        return jsonOk({
          signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
          threadId: "thread_ops",
          runId: "run_x",
          count: 20,
          okCount: 20,
          errorCount: 0,
          receiptIds: [],
        })
      if (String(url).includes("/api/dse/canary/status"))
        return jsonOk({
          canary: {
            signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
            enabled: true,
            control_compiled_id: "sha256:control",
            canary_compiled_id: "sha256:compiled",
            rolloutPct: 20,
            salt: "salt",
            okCount: 20,
            errorCount: 0,
            minSamples: 20,
            maxErrorRate: 0.2,
            createdAtMs: 1,
            updatedAtMs: 2,
          },
        })

      if (String(url).endsWith("/api/dse/promote")) {
        return new Response(JSON.stringify({ ok: false, error: "boom" }), {
          status: 500,
          headers: { "content-type": "application/json; charset=utf-8", "x-oa-request-id": "req_promote_fail" },
        })
      }

      if (String(url).endsWith("/api/dse/canary/stop")) return jsonOk({ existed: true })

      return new Response("not found", { status: 404 })
    }

    const runCommand = async () => ({
      ok: true,
      code: 0,
      stdout: "sha\n",
      stderr: "",
      durationMs: 1,
      timedOut: false,
    })

    const parsed = parseOvernightArgs(["--base-url", "https://example.com", "--no-verify", "--no-e2e"])
    expect(parsed.ok).toBe(true)
    if (!parsed.ok) return

    const summary = await runOvernight({
      options: parsed.options,
      env: { OA_DSE_ADMIN_SECRET: "secret" },
      fetchFn,
      runCommand,
      readTextFile: async () =>
        JSON.stringify({
          exampleId: "ex1",
          inputJson: { message: "Hi", blueprintHint: { userHandle: "Ada", agentName: "Autopilot" } },
          expectedJson: { action: "none" },
          split: "train",
          tags: ["dataset:test"],
        }),
    })

    expect(summary.ok).toBe(false)
    expect(summary.errors.join("\n")).toContain("req=req_promote_fail")

    const stopCalled = calls.some((c) => new URL(c.url).pathname === "/api/dse/canary/stop")
    expect(stopCalled).toBe(true)

    const finishCall = calls.find((c) => new URL(c.url).pathname === "/api/dse/ops/run/finish")
    expect(finishCall).toBeTruthy()
    expect(finishCall?.body?.status).toBe("failed")
    expect(finishCall?.body?.runId).toBe("opsrun_3")
  })
})

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
    })

    expect(summary.ok).toBe(true)
    expect(summary.runId).toBe("opsrun_1")

    const paths = calls.map((c) => new URL(c.url).pathname)
    expect(paths[0]).toBe("/api/dse/ops/run/start")
    expect(paths).toContain("/api/dse/ops/run/event")
    expect(paths[paths.length - 1]).toBe("/api/dse/ops/run/finish")

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
})


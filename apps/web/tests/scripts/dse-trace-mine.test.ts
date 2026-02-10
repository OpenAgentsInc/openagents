import { describe, expect, it } from "vitest"

import { parseTraceMineArgs, runTraceMine } from "../../scripts/dse-trace-mine-lib"

const jsonOk = (body: unknown) =>
  new Response(JSON.stringify({ ok: true, ...(body as any) }), {
    status: 200,
    headers: { "content-type": "application/json; charset=utf-8" },
  })

describe("apps/web/scripts/dse-trace-mine", () => {
  it("parses defaults for mining filters and tags", () => {
    const parsed = parseTraceMineArgs(["--base-url", "https://example.com", "--signature-id", "@openagents/test/Sig.v1"])
    expect(parsed.ok).toBe(true)
    if (!parsed.ok) return
    expect(parsed.options.strategyId).toBe("rlm_lite.v1")
    expect(parsed.options.resultTag).toBe("Ok")
    expect(parsed.options.requireRlmTrace).toBe(true)
    expect(parsed.options.tags).toContain("trace_mined")
  })

  it("lists receipts then exports examples (headless, bearer-auth)", async () => {
    const calls: Array<{ url: string; init?: RequestInit; body?: any }> = []

    const fetchFn = async (url: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
      const bodyText = typeof init?.body === "string" ? init.body : ""
      const body = bodyText ? JSON.parse(bodyText) : null
      calls.push({ url: String(url), init, body })

      const pathname = new URL(String(url)).pathname
      if (pathname === "/api/dse/receipts/list") {
        return jsonOk({
          receipts: [
            {
              receiptId: "r1",
              signatureId: "@openagents/test/Sig.v1",
              compiled_id: "c1",
              threadId: "t1",
              runId: "run1",
              createdAtMs: 1,
              strategyId: "rlm_lite.v1",
              resultTag: "Ok",
              rlmTraceBlobId: "sha256:trace",
              rlmTraceEventCount: 2,
            },
          ],
        })
      }

      if (pathname === "/api/dse/trace/export") {
        return jsonOk({ signatureId: "@openagents/test/Sig.v1", exampleId: "trace:r1", existed: false, dryRun: false })
      }

      return new Response("not found", { status: 404 })
    }

    const parsed = parseTraceMineArgs([
      "--base-url",
      "https://example.com",
      "--signature-id",
      "@openagents/test/Sig.v1",
      "--split",
      "train",
      "--tag",
      "seed",
      "--concurrency",
      "1",
    ])
    expect(parsed.ok).toBe(true)
    if (!parsed.ok) return

    const summary = await runTraceMine({
      options: parsed.options,
      env: { OA_DSE_ADMIN_SECRET: "secret" },
      fetchFn,
    })

    expect(summary.ok).toBe(true)
    expect(summary.exported).toBe(1)
    expect(summary.failed).toBe(0)

    const paths = calls.map((c) => new URL(c.url).pathname)
    expect(paths[0]).toBe("/api/dse/receipts/list")
    expect(paths).toContain("/api/dse/trace/export")

    // Ensure bearer auth header is attached on all requests.
    for (const c of calls) {
      const h = (c.init?.headers ?? {}) as any
      expect(String(h.authorization ?? "")).toBe("Bearer secret")
    }

    const exportCall = calls.find((c) => new URL(c.url).pathname === "/api/dse/trace/export")
    expect(exportCall?.body?.receiptId).toBe("r1")
    expect(exportCall?.body?.split).toBe("train")
    expect(Array.isArray(exportCall?.body?.tags)).toBe(true)
    expect(exportCall?.body?.tags).toContain("trace_mined")
    expect(exportCall?.body?.tags).toContain("seed")
  })
})


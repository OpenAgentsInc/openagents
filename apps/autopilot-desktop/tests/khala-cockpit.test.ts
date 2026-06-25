import { describe, expect, test } from "bun:test"

import { buildKhalaTurn } from "../src/bun/khala-turn"
import {
  isEventStreamResponse,
  isKhalaCockpitModelId,
  isLiveReceipt,
  normalizeKhalaCockpitModelId,
  parseKhalaReceipt,
  reconstructKhalaCompletionFromSse,
  summarizeKhalaReceipt,
  KHALA_COCKPIT_MODEL_IDS,
  KHALA_MODEL_ID,
} from "../src/shared/khala-cockpit"

// Build the SSE wire bytes the gateway emits for a streamed completion: a series
// of `chat.completion.chunk` frames, the terminal `openagents` block on the
// FINAL chunk, then `data: [DONE]`.
function buildKhalaSseWire(
  deltas: string[],
  opts: { openagents?: unknown; usage?: unknown; finishReason?: string } = {},
): string {
  const { openagents, usage, finishReason = "stop" } = opts
  const frames = deltas.map((delta, index) => {
    const last = index === deltas.length - 1
    return `data: ${JSON.stringify({
      id: "chatcmpl_stream_x",
      model: "openagents/khala-code",
      object: "chat.completion.chunk",
      choices: [
        {
          index: 0,
          delta: delta === "" ? {} : { content: delta },
          finish_reason: last ? finishReason : null,
        },
      ],
      ...(last && usage !== undefined ? { usage } : {}),
      ...(last && openagents !== undefined ? { openagents } : {}),
    })}\n\n`
  })
  return `${frames.join("")}data: [DONE]\n\n`
}

// A Response-like object backed by a real ReadableStream so the incremental
// reader path in consumeSseBody runs.
function sseResponse(wire: string, headers: HeadersInit = {}): Response {
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      controller.enqueue(new TextEncoder().encode(wire))
      controller.close()
    },
  })
  const responseHeaders = new Headers(headers)
  responseHeaders.set("content-type", "text/event-stream; charset=utf-8")
  return new Response(stream, {
    headers: responseHeaders,
  })
}

// M1 (#6009, EPIC #6017) — Lane A Cockpit. The crossy-road smoke prompt the
// roadmap names as the north-star task.
const CROSSY_ROAD_PROMPT =
  "build a really high quality single html file crossy road game with three.js"

describe("khala-cockpit model ids", () => {
  test("exposes the single public openagents/khala id", () => {
    expect(KHALA_COCKPIT_MODEL_IDS).toEqual([KHALA_MODEL_ID])
    expect(KHALA_MODEL_ID).toBe("openagents/khala")
  })

  test("type guard recognizes the public id and tolerates deprecated split ids", () => {
    expect(isKhalaCockpitModelId("openagents/khala")).toBe(true)
    // Deprecated split ids are still recognized so a stale slug normalizes
    // instead of re-triggering model_unavailable.
    expect(isKhalaCockpitModelId("openagents/khala-mini")).toBe(true)
    expect(isKhalaCockpitModelId("openagents/khala-code")).toBe(true)
    expect(isKhalaCockpitModelId("openagents/khala-typo")).toBe(false)
    expect(isKhalaCockpitModelId("gemini-3.5-flash")).toBe(false)
  })

  test("normalizes any khala-family slug to the single public id", () => {
    expect(normalizeKhalaCockpitModelId(undefined)).toBe("openagents/khala")
    expect(normalizeKhalaCockpitModelId("openagents/khala")).toBe(
      "openagents/khala",
    )
    expect(normalizeKhalaCockpitModelId("openagents/khala-mini")).toBe(
      "openagents/khala",
    )
    expect(normalizeKhalaCockpitModelId("openagents/khala-code")).toBe(
      "openagents/khala",
    )
  })
})

describe("parseKhalaReceipt — consumes the gateway openagents block", () => {
  test("parses a khala-mini receipt (verification none, no metering)", () => {
    const body = {
      choices: [{ message: { content: "hi" } }],
      openagents: {
        requested_model: "openagents/khala-mini",
        served_model: "gemini-3.5-flash",
        worker: "vertex-gemini",
        lane: "cheap",
        verification: "none",
      },
    }
    const r = parseKhalaReceipt(body)
    expect(r).not.toBeNull()
    expect(r?.requestedModel).toBe("openagents/khala-mini")
    expect(r?.servedModel).toBe("gemini-3.5-flash")
    expect(r?.worker).toBe("vertex-gemini")
    expect(r?.lane).toBe("cheap")
    expect(r?.verification).toBe("none")
    expect(r?.receipt).toBeNull()
    // No receipt => not live.
    expect(isLiveReceipt(r)).toBe(false)
  })

  test("parses a khala-code receipt with rubric, receipt, and verified pass", () => {
    const body = {
      choices: [{ message: { content: "<html>...</html>" } }],
      openagents: {
        requested_model: "openagents/khala-code",
        served_model: "fireworks-coder",
        worker: "fireworks",
        lane: "coding",
        route: "coding",
        verification: "test_passed",
        verified: true,
        receipt: "oa_receipt_abc",
        receipt_url: "/api/public/inference/receipts/oa_receipt_abc",
        workers: ["fireworks", "khala-code-verifier"],
        rubric: {
          ref: "rubric.crossy_road.v1",
          passed_checks: ["single_html_file", "loads_and_runs_headless"],
          failed_checks: [],
        },
      },
    }
    const r = parseKhalaReceipt(body)
    expect(r?.lane).toBe("coding")
    expect(r?.verification).toBe("test_passed")
    expect(r?.verified).toBe(true)
    expect(r?.receipt).toBe("oa_receipt_abc")
    expect(r?.receiptUrl).toBe("/api/public/inference/receipts/oa_receipt_abc")
    expect(r?.rubric?.ref).toBe("rubric.crossy_road.v1")
    expect(r?.rubric?.passedChecks).toContain("single_html_file")
    // Real receipt ref => LIVE.
    expect(isLiveReceipt(r)).toBe(true)
  })

  test("returns null on a non-khala / missing openagents block (renders not-live)", () => {
    expect(parseKhalaReceipt({ choices: [] })).toBeNull()
    expect(parseKhalaReceipt({ openagents: null })).toBeNull()
    expect(parseKhalaReceipt({ openagents: {} })).toBeNull()
    expect(parseKhalaReceipt(null)).toBeNull()
    expect(parseKhalaReceipt("nope")).toBeNull()
  })

  test("summary is public-safe and reflects live/not-live", () => {
    const verified = parseKhalaReceipt({
      openagents: {
        requested_model: "openagents/khala-code",
        served_model: "fireworks-coder",
        worker: "fireworks",
        lane: "coding",
        verification: "test_passed",
        verified: true,
        receipt: "oa_receipt_abc",
      },
    })
    expect(summarizeKhalaReceipt(verified)).toContain("verified (tests passed)")
    expect(summarizeKhalaReceipt(verified)).toContain("live")
    expect(summarizeKhalaReceipt(null)).toContain("not verified")
  })
})

describe("buildKhalaTurn — cockpit call path (stub gateway)", () => {
  const baseEnv = { OPENAGENTS_INFERENCE_GATEWAY_BASE_URL: "https://gw.test" }
  const env = { ...baseEnv, OPENAGENTS_DESKTOP_KHALA_ISSUER: "off" }

  test("no token: honest message, no network call, not live", async () => {
    let called = false
    const fetchFn = (() => {
      called = true
      return Promise.resolve(new Response("{}"))
    }) as unknown as typeof fetch
    const r = await buildKhalaTurn({
      prompt: CROSSY_ROAD_PROMPT,
      env,
      agentToken: null,
      fetchFn,
    })
    expect(called).toBe(false)
    expect(r.ok).toBe(false)
    expect(r.live).toBe(false)
    expect(r.receipt).toBeNull()
    expect(r.text).toContain("OPENAGENTS_AGENT_TOKEN")
  })

  test("crossy-road smoke against khala-code: renders completion + LIVE receipt", async () => {
    let sentUrl = ""
    let sentBody: unknown = null
    const fetchFn = ((url: string, init?: RequestInit) => {
      sentUrl = url
      sentBody = init?.body ? JSON.parse(init.body as string) : null
      return Promise.resolve(
        new Response(
          JSON.stringify({
            id: "chatcmpl_x",
            model: "openagents/khala-code",
            choices: [
              {
                index: 0,
                finish_reason: "stop",
                message: {
                  role: "assistant",
                  content: "<!doctype html><html><!-- crossy road --></html>",
                },
              },
            ],
            usage: { prompt_tokens: 10, completion_tokens: 200, total_tokens: 210 },
            openagents: {
              requested_model: "openagents/khala-code",
              served_model: "fireworks-coder",
              worker: "fireworks",
              lane: "coding",
              route: "coding",
              verification: "test_passed",
              verified: true,
              receipt: "oa_receipt_live_1",
              receipt_url: "/api/public/inference/receipts/oa_receipt_live_1",
              rubric: {
                ref: "rubric.crossy_road.v1",
                passed_checks: ["single_html_file"],
                failed_checks: [],
              },
            },
          }),
          { headers: { "content-type": "application/json" } },
        ),
      )
    }) as unknown as typeof fetch

    const r = await buildKhalaTurn({
      prompt: CROSSY_ROAD_PROMPT,
      model: KHALA_MODEL_ID,
      env,
      agentToken: "agent-token",
      fetchFn,
    })

    expect(sentUrl).toBe("https://gw.test/v1/chat/completions")
    // The cockpit always submits the single public id, even when a split slug
    // is requested — the gateway only serves openagents/khala.
    expect((sentBody as { model?: string }).model).toBe("openagents/khala")
    expect(r.ok).toBe(true)
    expect(r.text).toContain("crossy road")
    expect(r.receipt?.servedModel).toBe("fireworks-coder")
    expect(r.receipt?.verification).toBe("test_passed")
    // Real receipt ref => the cockpit may show a LIVE badge.
    expect(r.live).toBe(true)
  })

  test("khala-mini stub with NO receipt: real answer but NOT live", async () => {
    const fetchFn = (() =>
      Promise.resolve(
        new Response(
          JSON.stringify({
            choices: [{ message: { content: "an answer" } }],
            // No openagents block (e.g. a stub/free route).
          }),
          { headers: { "content-type": "application/json" } },
        ),
      )) as unknown as typeof fetch
    const r = await buildKhalaTurn({
      prompt: "hello",
      env,
      agentToken: "agent-token",
      fetchFn,
    })
    expect(r.ok).toBe(true)
    expect(r.text).toBe("an answer")
    expect(r.receipt).toBeNull()
    // The LIVE gate must be false without a receipt — never claim live off a stub.
    expect(r.live).toBe(false)
  })

  test("defaults to the single public openagents/khala when no model given", async () => {
    let sentModel = ""
    const fetchFn = ((_url: string, init?: RequestInit) => {
      sentModel = init?.body
        ? (JSON.parse(init.body as string) as { model: string }).model
        : ""
      return Promise.resolve(
        new Response(JSON.stringify({ choices: [{ message: { content: "x" } }] })),
      )
    }) as unknown as typeof fetch
    await buildKhalaTurn({ prompt: "hi", env, agentToken: "t", fetchFn })
    expect(sentModel).toBe("openagents/khala")
  })

  test("normalizes a stale split slug to openagents/khala (the model_unavailable fix)", async () => {
    let sentModel = ""
    const fetchFn = ((_url: string, init?: RequestInit) => {
      sentModel = init?.body
        ? (JSON.parse(init.body as string) as { model: string }).model
        : ""
      return Promise.resolve(
        new Response(JSON.stringify({ choices: [{ message: { content: "x" } }] })),
      )
    }) as unknown as typeof fetch
    await buildKhalaTurn({
      prompt: "hi",
      model: "openagents/khala-mini",
      env,
      agentToken: "t",
      fetchFn,
    })
    expect(sentModel).toBe("openagents/khala")
  })

  test("402 maps to an actionable credit message, not a faked answer", async () => {
    const fetchFn = (() =>
      Promise.resolve(
        new Response(JSON.stringify({ error: "insufficient credits" }), {
          status: 402,
        }),
      )) as unknown as typeof fetch
    const r = await buildKhalaTurn({
      prompt: "hi",
      env,
      agentToken: "t",
      fetchFn,
    })
    expect(r.ok).toBe(false)
    expect(r.live).toBe(false)
    expect(r.text).toContain("credit")
  })

  test("STREAMS by default: sends stream:true and reconstructs from SSE", async () => {
    let sentBody: { stream?: boolean; model?: string } = {}
    const wire = buildKhalaSseWire(["<!doctype ", "html>", "<!-- crossy -->"], {
      usage: { prompt_tokens: 10, completion_tokens: 200, total_tokens: 210 },
      openagents: {
        requested_model: "openagents/khala-code",
        served_model: "fireworks-coder",
        worker: "fireworks",
        lane: "coding",
        verification: "test_passed",
        verified: true,
        receipt: "oa_receipt_stream_live",
        receipt_url: "/api/public/inference/receipts/oa_receipt_stream_live",
      },
    })
    const fetchFn = ((_url: string, init?: RequestInit) => {
      sentBody = init?.body ? JSON.parse(init.body as string) : {}
      return Promise.resolve(sseResponse(wire))
    }) as unknown as typeof fetch

    const tokens: string[] = []
    const r = await buildKhalaTurn({
      prompt: CROSSY_ROAD_PROMPT,
      model: KHALA_MODEL_ID,
      env,
      agentToken: "agent-token",
      fetchFn,
      onToken: (t) => tokens.push(t),
    })

    // Default is streaming.
    expect(sentBody.stream).toBe(true)
    expect(r.ok).toBe(true)
    expect(r.text).toBe("<!doctype html><!-- crossy -->")
    // Tokens streamed live during consumption.
    expect(tokens.join("")).toBe("<!doctype html><!-- crossy -->")
    // Terminal openagents receipt attached on stream close => LIVE.
    expect(r.receipt?.servedModel).toBe("fireworks-coder")
    expect(r.receipt?.verification).toBe("test_passed")
    expect(r.receipt?.receipt).toBe("oa_receipt_stream_live")
    expect(r.live).toBe(true)
  })

  test("legacy gateway path captures resumable durable headers when present", async () => {
    const wire = buildKhalaSseWire(["durable ", "answer"])
    const fetchFn = (() =>
      Promise.resolve(
        sseResponse(wire, {
          "openagents-coding-assignment-ref": "assignment.khala.legacy",
          "openagents-durable-stream-url":
            "/v1/chat/completions/durable/chatcmpl_legacy_123",
        }),
      )) as unknown as typeof fetch
    const r = await buildKhalaTurn({
      prompt: "hello",
      env,
      agentToken: "agent-token",
      fetchFn,
    })
    expect(r.ok).toBe(true)
    expect(r.issuerPath).toBe("legacy_gateway")
    expect(r.durableRequestId).toBe("chatcmpl_legacy_123")
    expect(r.durableStreamUrl).toBe(
      "/v1/chat/completions/durable/chatcmpl_legacy_123",
    )
    expect(r.assignmentRef).toBe("assignment.khala.legacy")
  })

  test("local issuer mode uses the Pylon MCP khala.request contract", async () => {
    let sentUrl = ""
    let sentBody: Record<string, unknown> = {}
    const wire = buildKhalaSseWire(["mcp ", "answer"])
    const fetchFn = ((url: URL | string, init?: RequestInit) => {
      sentUrl = String(url)
      sentBody = init?.body ? JSON.parse(init.body as string) : {}
      return Promise.resolve(
        sseResponse(wire, {
          "openagents-coding-assignment-ref": "assignment.khala.local",
          "openagents-durable-stream-url":
            "/v1/chat/completions/durable/chatcmpl_local_123",
        }),
      )
    }) as unknown as typeof fetch
    const r = await buildKhalaTurn({
      prompt: "build the thing",
      env: {
        ...baseEnv,
        OPENAGENTS_DESKTOP_KHALA_ISSUER: "local",
        OPENAGENTS_DESKTOP_KHALA_TARGET_PYLON_REF: "pylon.local_123",
      },
      agentToken: "agent-token",
      fetchFn,
    })
    expect(sentUrl).toBe("https://gw.test/v1/chat/completions")
    expect(sentBody.model).toBe("openagents/khala")
    expect(sentBody.stream).toBe(true)
    expect((sentBody.openagents as { workflowClass?: string }).workflowClass).toBe(
      "codex_agent_task",
    )
    expect(
      (
        (sentBody.openagents as { coding?: { targetPylonRef?: string } })
          .coding ?? {}
      ).targetPylonRef,
    ).toBe("pylon.local_123")
    expect(r.ok).toBe(true)
    expect(r.issuerPath).toBe("pylon_mcp_local")
    expect(r.text).toBe("mcp answer")
    expect(r.durableRequestId).toBe("chatcmpl_local_123")
    expect(r.assignmentRef).toBe("assignment.khala.local")
  })

  test("default issuer mode calls remote /api/mcp and surfaces the durable handle", async () => {
    let sentUrl = ""
    let sentBody: Record<string, unknown> = {}
    const fetchFn = ((url: string, init?: RequestInit) => {
      sentUrl = url
      sentBody = init?.body ? JSON.parse(init.body as string) : {}
      return Promise.resolve(
        new Response(
          JSON.stringify({
            id: "desktop.khala.request",
            jsonrpc: "2.0",
            result: {
              content: [{ text: "{}", type: "text" }],
              structuredContent: {
                assignmentRef: "assignment.khala.remote",
                durableRequestId: "chatcmpl_remote_123",
                durableStreamUrl:
                  "/v1/chat/completions/durable/chatcmpl_remote_123",
                ok: true,
                schema: "openagents.khala_mcp.request.v1",
                stream: true,
                workflow: "codex_agent_task",
              },
            },
          }),
          { headers: { "content-type": "application/json" } },
        ),
      )
    }) as unknown as typeof fetch
    const r = await buildKhalaTurn({
      prompt: "build the remote thing",
      env: baseEnv,
      agentToken: "agent-token",
      fetchFn,
    })
    expect(sentUrl).toBe("https://gw.test/api/mcp")
    expect(sentBody.method).toBe("tools/call")
    expect((sentBody.params as { name?: string }).name).toBe("khala.request")
    expect(
      ((sentBody.params as { arguments?: { workflow?: string } }).arguments ?? {})
        .workflow,
    ).toBe("codex_agent_task")
    expect(r.ok).toBe(true)
    expect(r.issuerPath).toBe("remote_mcp")
    expect(r.text).toContain("Resume handle: chatcmpl_remote_123")
    expect(r.durableRequestId).toBe("chatcmpl_remote_123")
    expect(r.assignmentRef).toBe("assignment.khala.remote")
  })

  test("streamed completion with NO receipt: real answer but NOT live", async () => {
    const wire = buildKhalaSseWire(["plain ", "answer"])
    const fetchFn = (() =>
      Promise.resolve(sseResponse(wire))) as unknown as typeof fetch
    const r = await buildKhalaTurn({
      prompt: "hello",
      env,
      agentToken: "agent-token",
      fetchFn,
    })
    expect(r.ok).toBe(true)
    expect(r.text).toBe("plain answer")
    expect(r.receipt).toBeNull()
    expect(r.live).toBe(false)
  })

  test("stream:false opt-out still parses the blocking JSON body", async () => {
    let sentStream: boolean | undefined
    const fetchFn = ((_url: string, init?: RequestInit) => {
      sentStream = init?.body
        ? (JSON.parse(init.body as string) as { stream?: boolean }).stream
        : undefined
      return Promise.resolve(
        new Response(
          JSON.stringify({ choices: [{ message: { content: "blocking" } }] }),
          { headers: { "content-type": "application/json" } },
        ),
      )
    }) as unknown as typeof fetch
    const r = await buildKhalaTurn({
      prompt: "hi",
      env,
      agentToken: "t",
      fetchFn,
      stream: false,
    })
    expect(sentStream).toBe(false)
    expect(r.ok).toBe(true)
    expect(r.text).toBe("blocking")
  })
})

describe("reconstructKhalaCompletionFromSse + isEventStreamResponse", () => {
  test("concatenates deltas, keeps terminal openagents + usage", () => {
    const wire = buildKhalaSseWire(["a", "b", "c"], {
      usage: { total_tokens: 3 },
      openagents: { requested_model: "openagents/khala-code", receipt: "r1" },
    })
    const body = reconstructKhalaCompletionFromSse(wire)
    expect(body.choices[0].message.content).toBe("abc")
    expect(body.choices[0].finish_reason).toBe("stop")
    expect((body.usage as { total_tokens: number }).total_tokens).toBe(3)
    expect((body.openagents as { receipt: string }).receipt).toBe("r1")

    // The reconstructed body parses through the same receipt parser.
    const receipt = parseKhalaReceipt(body)
    expect(receipt?.receipt).toBe("r1")
  })

  test("fires onToken per delta", () => {
    const wire = buildKhalaSseWire(["x", "y"])
    const tokens: string[] = []
    reconstructKhalaCompletionFromSse(wire, (t) => tokens.push(t))
    expect(tokens).toEqual(["x", "y"])
  })

  test("isEventStreamResponse detects the SSE content-type", () => {
    expect(isEventStreamResponse("text/event-stream; charset=utf-8")).toBe(true)
    expect(isEventStreamResponse("application/json")).toBe(false)
    expect(isEventStreamResponse(null)).toBe(false)
  })
})

/**
 * KHS-1 (#8600): Khala gateway inference transport tests.
 *
 * The gateway transport is flag-gated on SARAH_INFERENCE_GATEWAY_URL +
 * SARAH_INFERENCE_GATEWAY_TOKEN; with the flags absent the direct-Google path
 * is byte-for-byte the pre-KHS-1 behavior. All fetches are mocked — no test
 * ever calls a live provider.
 */

import { afterEach, beforeEach, describe, expect, test } from "bun:test"

import {
  generateSarahGemmaReply,
  gatewayTelemetryToTurnUsage,
  gatewayUsageToTurnUsage,
  isSarahInferenceBusyError,
  resetSarahDailyTokenUsageForTests,
  SARAH_DAILY_TOKEN_CAP_ERROR,
  SARAH_GATEWAY_DEMAND_HEADERS,
  SARAH_GATEWAY_MODEL_DEFAULT,
  sarahActiveModelId,
  sarahInferenceArmed,
  sarahInferenceGatewayArmed,
  sarahInferenceTransport,
  streamSarahGemmaReply,
  toGatewayMessages,
} from "./google-inference.ts"
import type {
  GemmaStreamEvent,
  SarahTextInferenceSpendAlert,
} from "./google-inference.ts"

const ENV_KEYS = [
  "GEMINI_API_KEY",
  "SARAH_INFERENCE_GATEWAY_URL",
  "SARAH_INFERENCE_GATEWAY_TOKEN",
  "SARAH_INFERENCE_GATEWAY_MODEL",
  "SARAH_TEXT_DAILY_TOKEN_CAP",
  "SARAH_TEXT_SPEND_ALERT_THRESHOLD",
  "SARAH_TEXT_MODEL",
  "SARAH_TEXT_MODEL_FALLBACKS",
] as const

const savedEnv = new Map<string, string | undefined>()

beforeEach(() => {
  for (const key of ENV_KEYS) {
    savedEnv.set(key, process.env[key])
    delete process.env[key]
  }
  resetSarahDailyTokenUsageForTests()
})

afterEach(() => {
  for (const key of ENV_KEYS) {
    const value = savedEnv.get(key)
    if (value === undefined) delete process.env[key]
    else process.env[key] = value
  }
  resetSarahDailyTokenUsageForTests()
})

function armGateway(url = "https://openagents.com/api/v1"): void {
  process.env.SARAH_INFERENCE_GATEWAY_URL = url
  process.env.SARAH_INFERENCE_GATEWAY_TOKEN = "test-agent-token"
}

type RecordedCall = { url: string; init: RequestInit }

function recordingFetch(
  response: Response | (() => Response),
  calls: RecordedCall[],
): (url: string, init: RequestInit) => Promise<Response> {
  return async (url, init) => {
    calls.push({ url, init })
    return typeof response === "function" ? response() : response
  }
}

function gatewayJsonResponse(overrides?: {
  content?: string
  usage?: Record<string, number>
}): Response {
  return Response.json({
    id: "chatcmpl-test",
    model: SARAH_GATEWAY_MODEL_DEFAULT,
    choices: [
      {
        index: 0,
        message: {
          role: "assistant",
          content: overrides?.content ?? "Hi! I'm Sarah.",
        },
        finish_reason: "stop",
      },
    ],
    usage: overrides?.usage ?? {
      prompt_tokens: 40,
      completion_tokens: 50,
      total_tokens: 100,
    },
  })
}

function sseResponse(frames: string[]): Response {
  const encoder = new TextEncoder()
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      for (const frame of frames) {
        controller.enqueue(encoder.encode(frame))
      }
      controller.close()
    },
  })
  return new Response(stream, {
    headers: { "content-type": "text/event-stream" },
  })
}

async function collect(
  events: AsyncGenerator<GemmaStreamEvent>,
): Promise<GemmaStreamEvent[]> {
  const out: GemmaStreamEvent[] = []
  for await (const event of events) out.push(event)
  return out
}

const ignoreSpendAlert = (): void => {}

describe("arming / transport selection", () => {
  test("flag-off default: nothing armed without any env", () => {
    expect(sarahInferenceGatewayArmed()).toBe(false)
    expect(sarahInferenceArmed()).toBe(false)
    expect(sarahInferenceTransport()).toBe("not_armed")
  })

  test("gateway needs BOTH url and token", () => {
    process.env.SARAH_INFERENCE_GATEWAY_URL = "https://openagents.com/api/v1"
    expect(sarahInferenceGatewayArmed()).toBe(false)
    process.env.SARAH_INFERENCE_GATEWAY_TOKEN = "t"
    expect(sarahInferenceGatewayArmed()).toBe(true)
    expect(sarahInferenceTransport()).toBe("khala_gateway")
  })

  test("gateway wins over the direct key when both are set", () => {
    process.env.GEMINI_API_KEY = "google-key"
    expect(sarahInferenceTransport()).toBe("google_direct")
    armGateway()
    expect(sarahInferenceTransport()).toBe("khala_gateway")
    expect(sarahActiveModelId()).toBe(SARAH_GATEWAY_MODEL_DEFAULT)
  })

  test("generate without any transport returns the not-armed error", async () => {
    const result = await generateSarahGemmaReply({
      system: "s",
      contents: [{ role: "user", parts: [{ text: "hi" }] }],
    })
    expect(result).toEqual({ ok: false, error: "google_inference_not_armed" })
  })
})

describe("flag-off default behavior unchanged (direct Google path)", () => {
  test("generate hits generativelanguage.googleapis.com, never the gateway", async () => {
    process.env.GEMINI_API_KEY = "google-key"
    const calls: RecordedCall[] = []
    const result = await generateSarahGemmaReply({
      system: "sys",
      contents: [{ role: "user", parts: [{ text: "hello" }] }],
      fetchImpl: recordingFetch(
        Response.json({
          candidates: [
            {
              content: {
                parts: [
                  { text: "thinking...", thought: true },
                  { text: "Hello from Gemma." },
                ],
              },
            },
          ],
          usageMetadata: {
            promptTokenCount: 10,
            candidatesTokenCount: 5,
            thoughtsTokenCount: 3,
            totalTokenCount: 18,
          },
        }),
        calls,
      ),
    })
    expect(calls).toHaveLength(1)
    expect(calls[0]!.url).toContain(
      "generativelanguage.googleapis.com/v1beta/models/gemma-4-31b-it:generateContent",
    )
    expect(result).toEqual({
      ok: true,
      reply: "Hello from Gemma.",
      model: "gemma-4-31b-it",
      usage: {
        promptTokens: 10,
        outputTokens: 5,
        thoughtTokens: 3,
        totalTokens: 18,
      },
    })
  })
})

describe("gateway generate", () => {
  test("posts OpenAI-compatible request with bearer + internal demand headers", async () => {
    armGateway()
    const calls: RecordedCall[] = []
    const result = await generateSarahGemmaReply({
      system: "You are Sarah.",
      contents: [
        { role: "user", parts: [{ text: "hi" }] },
        { role: "model", parts: [{ text: "hello" }] },
        { role: "user", parts: [{ text: "tell me more" }] },
      ],
      fetchImpl: recordingFetch(gatewayJsonResponse(), calls),
    })

    expect(calls).toHaveLength(1)
    expect(calls[0]!.url).toBe(
      "https://openagents.com/api/v1/chat/completions",
    )
    const headers = calls[0]!.init.headers as Record<string, string>
    expect(headers.authorization).toBe("Bearer test-agent-token")
    expect(headers["x-openagents-demand-kind"]).toBe("internal")
    expect(headers["x-openagents-demand-source"]).toBe("sarah")
    expect(headers["x-openagents-client"]).toBe("sarah-server")
    // The URL must never carry a credential on the gateway transport.
    expect(calls[0]!.url).not.toContain("key=")

    const body = JSON.parse(String(calls[0]!.init.body)) as {
      model: string
      stream: boolean
      max_tokens: number
      messages: Array<{ role: string; content: string }>
    }
    expect(body.model).toBe("openagents/internal-neutral")
    expect(body.stream).toBe(false)
    expect(body.max_tokens).toBe(2048)
    expect(body.messages).toEqual([
      { role: "system", content: "You are Sarah." },
      { role: "user", content: "hi" },
      { role: "assistant", content: "hello" },
      { role: "user", content: "tell me more" },
    ])

    expect(result).toEqual({
      ok: true,
      reply: "Hi! I'm Sarah.",
      model: "openagents/internal-neutral",
      usage: {
        promptTokens: 40,
        outputTokens: 50,
        // Exact reconciliation gap total - (prompt + completion): the
        // thinking-lane scratchpad tokens, mirroring gateway telemetry's
        // unaccountedTokens. Never invented beyond provider-reported numbers.
        thoughtTokens: 10,
        totalTokens: 100,
      },
    })
  })

  test("model env override is honored", async () => {
    armGateway()
    process.env.SARAH_INFERENCE_GATEWAY_MODEL = "khala"
    const calls: RecordedCall[] = []
    await generateSarahGemmaReply({
      system: "s",
      contents: [{ role: "user", parts: [{ text: "hi" }] }],
      fetchImpl: recordingFetch(gatewayJsonResponse(), calls),
    })
    const body = JSON.parse(String(calls[0]!.init.body)) as { model: string }
    expect(body.model).toBe("khala")
  })

  test("gateway 429 surfaces as a typed busy-classified error", async () => {
    armGateway()
    const result = await generateSarahGemmaReply({
      system: "s",
      contents: [{ role: "user", parts: [{ text: "hi" }] }],
      fetchImpl: recordingFetch(new Response("rate limited", { status: 429 }), []),
    })
    expect(result).toEqual({ ok: false, error: "gateway_inference_http_429" })
    expect(isSarahInferenceBusyError("gateway_inference_http_429")).toBe(true)
    expect(isSarahInferenceBusyError("google_inference_http_429")).toBe(true)
    expect(isSarahInferenceBusyError("gateway_inference_http_500")).toBe(false)
  })

  test("empty gateway reply is a typed error", async () => {
    armGateway()
    const result = await generateSarahGemmaReply({
      system: "s",
      contents: [{ role: "user", parts: [{ text: "hi" }] }],
      fetchImpl: recordingFetch(gatewayJsonResponse({ content: "  " }), []),
    })
    expect(result).toEqual({
      ok: false,
      error: "gateway_inference_empty_reply",
    })
  })

  test("network failure is a typed unreachable error", async () => {
    armGateway()
    const result = await generateSarahGemmaReply({
      system: "s",
      contents: [{ role: "user", parts: [{ text: "hi" }] }],
      fetchImpl: async () => {
        throw new Error("connect ECONNREFUSED")
      },
    })
    expect(result).toEqual({
      ok: false,
      error: "gateway_inference_unreachable",
    })
  })
})

describe("gateway streaming", () => {
  test("external coordinator abort propagates to the live provider request", async () => {
    armGateway()
    const controller = new AbortController()
    const providerSignals: AbortSignal[] = []
    const collected = collect(
      streamSarahGemmaReply({
        system: "sys",
        contents: [{ role: "user", parts: [{ text: "hi" }] }],
        signal: controller.signal,
        fetchImpl: async (_url, init) => {
          const providerSignal = init.signal as AbortSignal
          providerSignals.push(providerSignal)
          return await new Promise<Response>((_resolve, reject) => {
            providerSignal.addEventListener(
              "abort",
              () => reject(new DOMException("aborted", "AbortError")),
              { once: true },
            )
          })
        },
      }),
    )
    await Promise.resolve()
    expect(providerSignals[0]?.aborted).toBe(false)
    controller.abort()
    expect(await collected).toEqual([
      { type: "error", error: "gateway_inference_timeout" },
    ])
    expect(providerSignals[0]?.aborted).toBe(true)
  })

  test("forwards content deltas, never reasoning_content, usage from telemetry", async () => {
    armGateway()
    const calls: RecordedCall[] = []
    const frames = [
      `data: ${JSON.stringify({
        object: "chat.completion.chunk",
        choices: [{ index: 0, delta: { role: "assistant" }, finish_reason: null }],
      })}\n\n`,
      // Thinking-model scratchpad rides reasoning_content — must be dropped.
      `data: ${JSON.stringify({
        object: "chat.completion.chunk",
        choices: [
          {
            index: 0,
            delta: { reasoning_content: "secret scratchpad" },
            finish_reason: null,
          },
        ],
      })}\n\n`,
      `data: ${JSON.stringify({
        object: "chat.completion.chunk",
        choices: [{ index: 0, delta: { content: "Hi! " }, finish_reason: null }],
      })}\n\n`,
      `data: ${JSON.stringify({
        object: "chat.completion.chunk",
        choices: [{ index: 0, delta: { content: "I'm Sarah." }, finish_reason: null }],
      })}\n\n`,
      `data: ${JSON.stringify({
        object: "chat.completion.chunk",
        choices: [{ index: 0, delta: {}, finish_reason: "stop" }],
        openagents: {
          telemetry: {
            promptTokens: 30,
            completionTokens: 12,
            totalTokens: 60,
          },
        },
      })}\n\n`,
      "data: [DONE]\n\n",
    ]
    const events = await collect(
      streamSarahGemmaReply({
        system: "sys",
        contents: [{ role: "user", parts: [{ text: "hi" }] }],
        fetchImpl: recordingFetch(() => sseResponse(frames), calls),
      }),
    )

    const body = JSON.parse(String(calls[0]!.init.body)) as { stream: boolean }
    expect(body.stream).toBe(true)

    expect(events).toEqual([
      { type: "delta", text: "Hi! " },
      { type: "delta", text: "I'm Sarah." },
      {
        type: "done",
        fullText: "Hi! I'm Sarah.",
        usage: {
          promptTokens: 30,
          outputTokens: 12,
          thoughtTokens: 18,
          totalTokens: 60,
        },
      },
    ])
    const serialized = JSON.stringify(events)
    expect(serialized).not.toContain("scratchpad")
  })

  test("not_measured telemetry sentinels degrade to zero, never invented", () => {
    expect(
      gatewayTelemetryToTurnUsage({
        promptTokens: "not_measured",
        completionTokens: "not_measured",
        totalTokens: "not_measured",
      }),
    ).toEqual({
      promptTokens: 0,
      outputTokens: 0,
      thoughtTokens: 0,
      totalTokens: 0,
    })
  })

  test("stream with no visible content is a typed empty-reply error", async () => {
    armGateway()
    const events = await collect(
      streamSarahGemmaReply({
        system: "sys",
        contents: [{ role: "user", parts: [{ text: "hi" }] }],
        fetchImpl: recordingFetch(
          () =>
            sseResponse([
              `data: ${JSON.stringify({
                choices: [
                  {
                    index: 0,
                    delta: { reasoning_content: "only thoughts" },
                    finish_reason: "stop",
                  },
                ],
              })}\n\n`,
              "data: [DONE]\n\n",
            ]),
          [],
        ),
      }),
    )
    expect(events).toEqual([
      { type: "error", error: "gateway_inference_empty_reply" },
    ])
  })

  test("stream gateway 429 is a typed error", async () => {
    armGateway()
    const events = await collect(
      streamSarahGemmaReply({
        system: "sys",
        contents: [{ role: "user", parts: [{ text: "hi" }] }],
        fetchImpl: recordingFetch(new Response("busy", { status: 429 }), []),
      }),
    )
    expect(events).toEqual([
      { type: "error", error: "gateway_inference_http_429" },
    ])
  })
})

describe("daily token cap and spend alert (#8600)", () => {
  test("emits one typed public-safe threshold alert before hard refusal", async () => {
    armGateway()
    process.env.SARAH_TEXT_DAILY_TOKEN_CAP = "250"
    process.env.SARAH_TEXT_SPEND_ALERT_THRESHOLD = "0.5"
    const calls: RecordedCall[] = []
    const alerts: SarahTextInferenceSpendAlert[] = []
    const fetchImpl = recordingFetch(() => gatewayJsonResponse(), calls)
    const spendAlertSink = (alert: SarahTextInferenceSpendAlert): void => {
      alerts.push(alert)
    }

    const turn = () =>
      generateSarahGemmaReply({
        system: "s",
        contents: [{ role: "user" as const, parts: [{ text: "hi" }] }],
        fetchImpl,
        spendAlertSink,
      })

    expect((await turn()).ok).toBe(true)
    expect(alerts).toHaveLength(0)

    // The second exact 100-token receipt crosses the 125-token threshold.
    expect((await turn()).ok).toBe(true)
    expect(alerts).toHaveLength(1)
    expect(alerts[0]).toMatchObject({
      type: "sarah.text_inference_spend_alert.v1",
      usageTruth: "provider_reported",
      providerReportedTokens: 200,
      dailyTokenCap: 250,
      thresholdTokens: 125,
    })
    expect(Object.keys(alerts[0]!).sort()).toEqual(
      [
        "dailyTokenCap",
        "day",
        "emittedAt",
        "providerReportedTokens",
        "thresholdTokens",
        "type",
        "usageTruth",
      ].sort(),
    )
    expect(JSON.stringify(alerts[0])).not.toContain("test-agent-token")
    expect(JSON.stringify(alerts[0])).not.toContain("openagents/internal-neutral")

    // Crossing the cap does not duplicate the threshold alert. The following
    // call refuses before provider fetch, after the alert is already visible.
    expect((await turn()).ok).toBe(true)
    expect(alerts).toHaveLength(1)
    const refusal = await turn()
    expect(refusal).toEqual({
      ok: false,
      error: SARAH_DAILY_TOKEN_CAP_ERROR,
    })
    expect(alerts).toHaveLength(1)
    expect(calls).toHaveLength(3)
  })

  test("cap refusal after exact provider-reported usage crosses the cap", async () => {
    armGateway()
    process.env.SARAH_TEXT_DAILY_TOKEN_CAP = "150"
    const fetchImpl = recordingFetch(() => gatewayJsonResponse(), [])

    const first = await generateSarahGemmaReply({
      system: "s",
      contents: [{ role: "user", parts: [{ text: "hi" }] }],
      fetchImpl,
      spendAlertSink: ignoreSpendAlert,
    })
    expect(first.ok).toBe(true)

    // 100 < 150 — still under the cap.
    const second = await generateSarahGemmaReply({
      system: "s",
      contents: [{ role: "user", parts: [{ text: "hi" }] }],
      fetchImpl,
      spendAlertSink: ignoreSpendAlert,
    })
    expect(second.ok).toBe(true)

    // 200 >= 150 — typed refusal BEFORE any provider call.
    const calls: RecordedCall[] = []
    const third = await generateSarahGemmaReply({
      system: "s",
      contents: [{ role: "user", parts: [{ text: "hi" }] }],
      fetchImpl: recordingFetch(gatewayJsonResponse(), calls),
      spendAlertSink: ignoreSpendAlert,
    })
    expect(third).toEqual({ ok: false, error: SARAH_DAILY_TOKEN_CAP_ERROR })
    expect(calls).toHaveLength(0)
    expect(isSarahInferenceBusyError(SARAH_DAILY_TOKEN_CAP_ERROR)).toBe(true)
  })

  test("cap applies to the streaming path too", async () => {
    armGateway()
    process.env.SARAH_TEXT_DAILY_TOKEN_CAP = "1"
    // Consume exact usage via the non-streaming path first.
    await generateSarahGemmaReply({
      system: "s",
      contents: [{ role: "user", parts: [{ text: "hi" }] }],
      fetchImpl: recordingFetch(gatewayJsonResponse(), []),
      spendAlertSink: ignoreSpendAlert,
    })
    const calls: RecordedCall[] = []
    const events = await collect(
      streamSarahGemmaReply({
        system: "s",
        contents: [{ role: "user", parts: [{ text: "hi" }] }],
        fetchImpl: recordingFetch(gatewayJsonResponse(), calls),
        spendAlertSink: ignoreSpendAlert,
      }),
    )
    expect(events).toEqual([
      { type: "error", error: SARAH_DAILY_TOKEN_CAP_ERROR },
    ])
    expect(calls).toHaveLength(0)
  })

  test("absent provider usage never invents threshold progress", async () => {
    armGateway()
    process.env.SARAH_TEXT_DAILY_TOKEN_CAP = "1"
    const alerts: SarahTextInferenceSpendAlert[] = []
    const result = await generateSarahGemmaReply({
      system: "s",
      contents: [{ role: "user", parts: [{ text: "hi" }] }],
      fetchImpl: recordingFetch(gatewayJsonResponse({ usage: {} }), []),
      spendAlertSink: (alert) => {
        alerts.push(alert)
      },
    })

    expect(result).toMatchObject({
      ok: true,
      usage: { totalTokens: 0 },
    })
    expect(alerts).toHaveLength(0)
  })

  test("unset cap is a pure no-op", async () => {
    armGateway()
    for (let i = 0; i < 3; i += 1) {
      const result = await generateSarahGemmaReply({
        system: "s",
        contents: [{ role: "user", parts: [{ text: "hi" }] }],
        fetchImpl: recordingFetch(gatewayJsonResponse(), []),
      })
      expect(result.ok).toBe(true)
    }
  })
})

describe("usage mapping helpers", () => {
  test("gatewayUsageToTurnUsage derives the exact thought gap", () => {
    expect(
      gatewayUsageToTurnUsage({
        prompt_tokens: 347,
        completion_tokens: 20,
        total_tokens: 679,
      }),
    ).toEqual({
      promptTokens: 347,
      outputTokens: 20,
      thoughtTokens: 312,
      totalTokens: 679,
    })
    // No negative thought counts when total is exactly prompt + completion.
    expect(
      gatewayUsageToTurnUsage({
        prompt_tokens: 10,
        completion_tokens: 10,
        total_tokens: 20,
      }).thoughtTokens,
    ).toBe(0)
    expect(gatewayUsageToTurnUsage(undefined)).toEqual({
      promptTokens: 0,
      outputTokens: 0,
      thoughtTokens: 0,
      totalTokens: 0,
    })
  })

  test("toGatewayMessages maps system + gemma roles to OpenAI roles", () => {
    expect(
      toGatewayMessages("sys", [
        { role: "user", parts: [{ text: "a" }, { text: "b" }] },
        { role: "model", parts: [{ text: "c" }] },
      ]),
    ).toEqual([
      { role: "system", content: "sys" },
      { role: "user", content: "ab" },
      { role: "assistant", content: "c" },
    ])
  })

  test("demand headers are the bounded internal attribution tokens", () => {
    expect(SARAH_GATEWAY_DEMAND_HEADERS).toEqual({
      "x-openagents-demand-kind": "internal",
      "x-openagents-demand-source": "sarah",
      "x-openagents-client": "sarah-server",
    })
  })
})

// ---------------------------------------------------------------------------
// FC-BRAIN #8600: persona-neutral internal lane + typed lane-fallback events
// ---------------------------------------------------------------------------

describe("persona-neutral internal lane (#8600)", () => {
  test("the default gateway model is the persona-neutral internal id, env-overridable", () => {
    expect(SARAH_GATEWAY_MODEL_DEFAULT).toBe("openagents/internal-neutral")
    armGateway()
    expect(sarahActiveModelId()).toBe("openagents/internal-neutral")
    process.env.SARAH_INFERENCE_GATEWAY_MODEL = "openagents/custom"
    expect(sarahActiveModelId()).toBe("openagents/custom")
    delete process.env.SARAH_INFERENCE_GATEWAY_MODEL
  })

  test("PERSONA PROBES: short identity turns carry ONLY Sarah's system prompt and the reply is verbatim", async () => {
    armGateway()
    const probes = ["who are you", "what are you", "hi", "you?"]
    for (const probe of probes) {
      const calls: RecordedCall[] = []
      const result = await generateSarahGemmaReply({
        system: "You are Sarah, the OpenAgents relationship agent.",
        contents: [{ role: "user", parts: [{ text: probe }] }],
        fetchImpl: recordingFetch(
          gatewayJsonResponse({ content: "I'm Sarah. How can I help?" }),
          calls,
        ),
      })
      const body = JSON.parse(String(calls[0]!.init.body)) as {
        model: string
        messages: Array<{ role: string; content: string }>
      }
      // The neutral lane is requested and the ONLY conditioning sent is
      // Sarah's own system prompt — nothing for a collective identity to win
      // over, and the reply passes through verbatim.
      expect(body.model).toBe("openagents/internal-neutral")
      expect(body.messages).toEqual([
        {
          role: "system",
          content: "You are Sarah, the OpenAgents relationship agent.",
        },
        { role: "user", content: probe },
      ])
      expect(result).toMatchObject({ ok: true, reply: "I'm Sarah. How can I help?" })
    }
  })

  test("the gateway per-account daily-cap 402 maps to the busy reply class", () => {
    expect(isSarahInferenceBusyError("gateway_inference_http_402")).toBe(true)
    expect(isSarahInferenceBusyError("google_inference_http_402")).toBe(false)
  })
})

describe("typed gateway lane-fallback events (#8600)", () => {
  // The wire receipt shape verified live on staging 2026-07-09: adapter id
  // rides `worker`, backing model rides `served_model`, fallback reason rides
  // `routing.fallback_reason`; `telemetry` is the token/latency summary only.
  function gatewayJsonResponseWithReceipt(receipt: {
    worker?: string
    served_model?: string
    fallback_reason?: string | null
  }): Response {
    return Response.json({
      id: "chatcmpl-test",
      model: "openagents/internal-neutral",
      choices: [
        {
          index: 0,
          message: { role: "assistant", content: "Answer." },
          finish_reason: "stop",
        },
      ],
      usage: { prompt_tokens: 10, completion_tokens: 5, total_tokens: 20 },
      openagents: {
        worker: receipt.worker,
        served_model: receipt.served_model,
        routing: { fallback_reason: receipt.fallback_reason ?? null },
        telemetry: {
          promptTokens: 10,
          completionTokens: 5,
          totalTokens: 20,
        },
      },
    })
  }

  test("non-streaming: a turn served OFF the primary Gemma lane emits one typed event", async () => {
    armGateway()
    const events: unknown[] = []
    const result = await generateSarahGemmaReply({
      system: "s",
      contents: [{ role: "user", parts: [{ text: "hi" }] }],
      fetchImpl: recordingFetch(
        gatewayJsonResponseWithReceipt({
          worker: "vertex-gemini",
          served_model: "gemini-3.5-flash",
          fallback_reason: "primary_lane_429",
        }),
        [],
      ),
      laneFallbackSink: (event) => {
        events.push(event)
      },
    })
    expect(result.ok).toBe(true)
    expect(events).toHaveLength(1)
    expect(events[0]).toMatchObject({
      type: "sarah.gateway_lane_fallback.v1",
      requestedModel: "openagents/internal-neutral",
      provider: "vertex-gemini",
      servedModel: "gemini-3.5-flash",
      fallbackReason: "primary_lane_429",
    })
    // Public-safe: bounded refs only — never the bearer token.
    expect(JSON.stringify(events[0])).not.toContain("test-agent-token")
  })

  test("non-streaming: a turn served on the primary Gemma lane emits NO event", async () => {
    armGateway()
    const events: unknown[] = []
    const result = await generateSarahGemmaReply({
      system: "s",
      contents: [{ role: "user", parts: [{ text: "hi" }] }],
      fetchImpl: recordingFetch(
        gatewayJsonResponseWithReceipt({
          worker: "google-gemma4",
          served_model: "gemma-4-31b-it",
          fallback_reason: null,
        }),
        [],
      ),
      laneFallbackSink: (event) => {
        events.push(event)
      },
    })
    expect(result.ok).toBe(true)
    expect(events).toHaveLength(0)
  })

  test("streaming: the terminal telemetry frame drives the same typed event", async () => {
    armGateway()
    const events: unknown[] = []
    const frames = [
      'data: {"choices":[{"delta":{"content":"Hel"}}]}\n',
      'data: {"choices":[{"delta":{"content":"lo"}}]}\n',
      'data: {"choices":[{"delta":{"content":""}}],"openagents":{"worker":"fireworks","served_model":"deepseek-v4-flash","routing":{"fallback_reason":"primary_lane_timeout"},"telemetry":{"promptTokens":10,"completionTokens":2,"totalTokens":12}}}\n',
      "data: [DONE]\n",
    ]
    const collected = await collect(
      streamSarahGemmaReply({
        system: "s",
        contents: [{ role: "user", parts: [{ text: "hi" }] }],
        fetchImpl: recordingFetch(sseResponse(frames), []),
        laneFallbackSink: (event) => {
          events.push(event)
        },
      }),
    )
    expect(collected.at(-1)).toMatchObject({ type: "done", fullText: "Hello" })
    expect(events).toHaveLength(1)
    expect(events[0]).toMatchObject({
      type: "sarah.gateway_lane_fallback.v1",
      provider: "fireworks",
      servedModel: "deepseek-v4-flash",
      fallbackReason: "primary_lane_timeout",
    })
  })

  test("a lane-fallback sink failure never affects the turn", async () => {
    armGateway()
    const result = await generateSarahGemmaReply({
      system: "s",
      contents: [{ role: "user", parts: [{ text: "hi" }] }],
      fetchImpl: recordingFetch(
        gatewayJsonResponseWithReceipt({
          worker: "vertex-gemini",
          fallback_reason: "primary_lane_429",
        }),
        [],
      ),
      laneFallbackSink: () => {
        throw new Error("sink down")
      },
    })
    expect(result.ok).toBe(true)
  })
})

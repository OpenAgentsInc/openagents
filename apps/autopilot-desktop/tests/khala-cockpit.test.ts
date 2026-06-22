import { describe, expect, test } from "bun:test"

import { buildKhalaTurn } from "../src/bun/khala-turn"
import {
  isKhalaCockpitModelId,
  isLiveReceipt,
  parseKhalaReceipt,
  summarizeKhalaReceipt,
  KHALA_CODE_MODEL_ID,
  KHALA_COCKPIT_MODEL_IDS,
  KHALA_MINI_MODEL_ID,
} from "../src/shared/khala-cockpit"

// M1 (#6009, EPIC #6017) — Lane A Cockpit. The crossy-road smoke prompt the
// roadmap names as the north-star task.
const CROSSY_ROAD_PROMPT =
  "build a really high quality single html file crossy road game with three.js"

describe("khala-cockpit model ids", () => {
  test("exposes khala-mini and khala-code", () => {
    expect(KHALA_COCKPIT_MODEL_IDS).toEqual([
      KHALA_MINI_MODEL_ID,
      KHALA_CODE_MODEL_ID,
    ])
    expect(KHALA_MINI_MODEL_ID).toBe("openagents/khala-mini")
    expect(KHALA_CODE_MODEL_ID).toBe("openagents/khala-code")
  })

  test("type guard accepts only known khala ids", () => {
    expect(isKhalaCockpitModelId("openagents/khala-mini")).toBe(true)
    expect(isKhalaCockpitModelId("openagents/khala-code")).toBe(true)
    expect(isKhalaCockpitModelId("openagents/khala-typo")).toBe(false)
    expect(isKhalaCockpitModelId("gemini-3.5-flash")).toBe(false)
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
  const env = { OPENAGENTS_INFERENCE_GATEWAY_BASE_URL: "https://gw.test" }

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
      model: KHALA_CODE_MODEL_ID,
      env,
      agentToken: "agent-token",
      fetchFn,
    })

    expect(sentUrl).toBe("https://gw.test/v1/chat/completions")
    expect((sentBody as { model?: string }).model).toBe("openagents/khala-code")
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

  test("defaults to khala-mini when no model given", async () => {
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
    expect(sentModel).toBe("openagents/khala-mini")
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
})

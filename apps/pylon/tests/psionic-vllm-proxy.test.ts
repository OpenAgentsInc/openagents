import { describe, expect, test } from "bun:test"

import {
  handlePsionicVllmProxyRequest,
  type PsionicVllmProxyConfig,
} from "../src/psionic-vllm-proxy"

const config = (
  fetchImpl: typeof fetch = async () =>
    new Response(
      JSON.stringify({
        choices: [{ finish_reason: "stop", message: { content: "OK" } }],
        model: "Qwen/Qwen2.5-0.5B-Instruct",
        usage: {
          completion_tokens: 1,
          prompt_tokens: 7,
          total_tokens: 8,
        },
      }),
      { headers: { "content-type": "application/json" }, status: 200 },
    ),
): PsionicVllmProxyConfig => ({
  bearerToken: "proxy-token",
  canaryRef: "canary.pylon.serving.known_answer.ok.v1",
  fetchImpl,
  nodeRef: "gcloud.gswarm508-clean2-20260325044551-contrib",
  replayChallengeRef: "challenge.pylon.serving.GuUBPkgNgLRtTCgkkO-s",
  servedModel: "model.psionic.qwen35.0_8b.q8_0",
  upstreamModel: "Qwen/Qwen2.5-0.5B-Instruct",
  upstreamUrl: "http://127.0.0.1:8000/v1/chat/completions",
})

const psionicRequest = (): Request =>
  new Request("http://127.0.0.1:8011/serve", {
    body: JSON.stringify({
      messages: [{ content: "Respond with exactly OK and nothing else.", role: "user" }],
      model: "model.psionic.qwen35.0_8b.q8_0",
      passthroughParams: { max_tokens: 1, temperature: 0 },
      requireExactGreedyParity: true,
    }),
    headers: { authorization: "Bearer proxy-token" },
    method: "POST",
  })

describe("Psionic vLLM proxy", () => {
  test("rejects missing bearer auth before touching upstream", async () => {
    let called = false
    const response = await handlePsionicVllmProxyRequest(
      new Request("http://127.0.0.1:8011/serve", { method: "POST" }),
      config(async () => {
        called = true
        return new Response("{}")
      }),
    )
    expect(response.status).toBe(401)
    expect(called).toBe(false)
  })

  test("returns a Psionic serve response with paid verification for the known-answer canary", async () => {
    const calls: Array<Record<string, unknown>> = []
    const response = await handlePsionicVllmProxyRequest(
      psionicRequest(),
      config(async (_input, init) => {
        calls.push(JSON.parse(String(init?.body)) as Record<string, unknown>)
        return new Response(
          JSON.stringify({
            choices: [{ finish_reason: "stop", message: { content: "OK" } }],
            usage: {
              completion_tokens: 1,
              prompt_tokens: 7,
              total_tokens: 8,
            },
          }),
          { headers: { "content-type": "application/json" }, status: 200 },
        )
      }),
    )
    const body = (await response.json()) as {
      content?: unknown
      parityVerified?: unknown
      paidTrafficVerification?: unknown
    }

    expect(response.status).toBe(200)
    expect(calls[0]?.model).toBe("Qwen/Qwen2.5-0.5B-Instruct")
    expect(body.content).toBe("OK")
    expect(body.parityVerified).toBe(true)
    expect(body.paidTrafficVerification).toEqual({
      blockerRefs: [],
      canaryPassed: true,
      parityPassed: true,
      payoutEligible: true,
      replayPassed: true,
    })
  })

  test("keeps non-canary output unpayable", async () => {
    const response = await handlePsionicVllmProxyRequest(
      psionicRequest(),
      config(async () =>
        new Response(
          JSON.stringify({
            choices: [{ finish_reason: "stop", message: { content: "Nope" } }],
            usage: {
              completion_tokens: 2,
              prompt_tokens: 7,
              total_tokens: 9,
            },
          }),
          { headers: { "content-type": "application/json" }, status: 200 },
        ),
      ),
    )
    const body = (await response.json()) as {
      parityVerified?: unknown
      paidTrafficVerification?: { blockerRefs?: unknown } | undefined
    }
    expect(response.status).toBe(200)
    expect(body.parityVerified).toBe(false)
    expect(body.paidTrafficVerification.blockerRefs).toEqual([
      "blocker.pylon_gateway_proxy.known_answer_canary_failed",
    ])
  })
})

// Khala cockpit smoke (M1, #6009, EPIC #6017) — Lane A.
//
// Stands up a tiny LOCAL STUB gateway that mimics the OpenAI-compatible
// `/v1/chat/completions` shape + the NON-BREAKING `openagents` receipt block,
// then runs the crossy-road north-star prompt through the real cockpit call
// path (`buildKhalaTurn`) against `openagents/khala-mini` and
// `openagents/khala-code`. Prints the rendered completion and the receipt
// projection, and asserts the LIVE gate behaves: a receipt-bearing response is
// live, a receipt-less response is not.
//
// SCAFFOLD, not product proof: this exercises the cockpit consumption path
// against a STUB, so the prod gateway being inert/owner-gated does not block it.
// Replace the stub with a real local/staging gateway to get a real receipt.

import { buildKhalaTurn } from "../src/bun/khala-turn.js"
import {
  KHALA_CODE_MODEL_ID,
  KHALA_MINI_MODEL_ID,
  summarizeKhalaReceipt,
} from "../src/shared/khala-cockpit.js"

const CROSSY_ROAD_PROMPT =
  "build a really high quality single html file crossy road game with three.js"

const STUB_HTML =
  "<!doctype html><html><head><title>Crossy Road</title></head>" +
  "<body><script type=\"module\">/* three.js crossy road */</script></body></html>"

// A stub gateway: khala-code returns a verified receipt; khala-mini returns a
// no-receipt (free/stub) route. Mimics the gateway `openagents` block shape.
const startStubGateway = (): { url: string; stop: () => void } => {
  const server = Bun.serve({
    port: 0,
    fetch: async (req) => {
      const body = (await req.json()) as { model: string }
      const model = body.model
      if (model === KHALA_CODE_MODEL_ID) {
        return Response.json({
          id: "chatcmpl_stub_code",
          model,
          choices: [
            { index: 0, finish_reason: "stop", message: { role: "assistant", content: STUB_HTML } },
          ],
          usage: { prompt_tokens: 18, completion_tokens: 540, total_tokens: 558 },
          openagents: {
            requested_model: KHALA_CODE_MODEL_ID,
            served_model: "stub-coder",
            worker: "stub-fabric",
            lane: "coding",
            route: "coding",
            verification: "test_passed",
            verified: true,
            receipt: "oa_receipt_stub_code_1",
            receipt_url: "/api/public/inference/receipts/oa_receipt_stub_code_1",
            rubric: {
              ref: "rubric.crossy_road.v1",
              passed_checks: ["single_html_file", "loads_and_runs_headless"],
              failed_checks: [],
            },
          },
        })
      }
      // khala-mini: a real answer but NO receipt (e.g. free/stub route).
      return Response.json({
        id: "chatcmpl_stub_mini",
        model,
        choices: [
          {
            index: 0,
            finish_reason: "stop",
            message: { role: "assistant", content: "Here is a plan for a crossy road game..." },
          },
        ],
        usage: { prompt_tokens: 18, completion_tokens: 60, total_tokens: 78 },
      })
    },
  })
  return { url: `http://127.0.0.1:${server.port}`, stop: () => server.stop(true) }
}

const main = async () => {
  const gateway = startStubGateway()
  const env = { OPENAGENTS_INFERENCE_GATEWAY_BASE_URL: gateway.url }
  const agentToken = "stub-agent-token"
  let failures = 0

  const run = async (
    label: string,
    model: typeof KHALA_MINI_MODEL_ID | typeof KHALA_CODE_MODEL_ID,
    expectLive: boolean,
  ) => {
    const r = await buildKhalaTurn({ prompt: CROSSY_ROAD_PROMPT, model, env, agentToken })
    console.log(`\n=== ${label} (${model}) ===`)
    console.log(`ok=${r.ok} live=${r.live}`)
    console.log(`completion: ${r.text.slice(0, 80)}${r.text.length > 80 ? "..." : ""}`)
    console.log(`receipt: ${summarizeKhalaReceipt(r.receipt)}`)
    if (r.receipt !== null) {
      console.log(
        `  requested=${r.receipt.requestedModel} served=${r.receipt.servedModel} ` +
          `worker=${r.receipt.worker} lane=${r.receipt.lane} ` +
          `verification=${r.receipt.verification} receipt=${r.receipt.receipt ?? "—"}`,
      )
    }
    if (!r.ok) {
      console.error(`  FAIL: turn not ok`)
      failures += 1
    }
    if (r.live !== expectLive) {
      console.error(`  FAIL: expected live=${expectLive}, got live=${r.live}`)
      failures += 1
    }
  }

  await run("khala-mini (no receipt → not live)", KHALA_MINI_MODEL_ID, false)
  await run("khala-code (verified receipt → live)", KHALA_CODE_MODEL_ID, true)

  gateway.stop()
  if (failures > 0) {
    console.error(`\nSMOKE FAILED (${failures} assertion(s)) — SCAFFOLD evidence.`)
    process.exit(1)
  }
  console.log("\nSMOKE PASSED — SCAFFOLD evidence (stub gateway, not product proof).")
}

await main()

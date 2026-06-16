import { describe, expect, test } from "bun:test"

import { claimTipReadiness } from "../src/tips"
import type { WalletCommandResult } from "../src/wallet"

// Returns a valid BOLT 12 offer for the `receive-bolt12` probe so the claim
// proceeds to the readiness POST we want to inspect.
const bolt12Runner = async (args: string[]): Promise<WalletCommandResult> =>
  args[0] === "receive-bolt12"
    ? { exitCode: 0, stdout: JSON.stringify({ offer: "lno1testoffer" }), stderr: "" }
    : { exitCode: 1, stdout: "", stderr: "unexpected" }

const captureFetch = (sink: { body: Record<string, unknown> | null }) =>
  (async (_url: unknown, init: { body: string }) => {
    sink.body = JSON.parse(init.body) as Record<string, unknown>
    return new Response(JSON.stringify({ tipRecipientReadiness: { state: "ready" } }), {
      status: 200,
      headers: { "content-type": "application/json" },
    })
  }) as unknown as typeof fetch

describe("claimTipReadiness static Lightning Address (#5078)", () => {
  test("publishes lightningAddress alongside bolt12Offer when provided", async () => {
    const sink: { body: Record<string, unknown> | null } = { body: null }
    await claimTipReadiness(
      { baseUrl: "https://x.test", agentToken: "t", fetch: captureFetch(sink) },
      { pylonRef: "pylon.test.one", lightningAddress: "oa-abc123@spark.example" },
      bolt12Runner,
    )
    expect(sink.body?.bolt12Offer).toBe("lno1testoffer")
    expect(sink.body?.lightningAddress).toBe("oa-abc123@spark.example")
  })

  test("omits lightningAddress when the node has none (Spark backup off)", async () => {
    const sink: { body: Record<string, unknown> | null } = { body: null }
    await claimTipReadiness(
      { baseUrl: "https://x.test", agentToken: "t", fetch: captureFetch(sink) },
      { pylonRef: "pylon.test.one" },
      bolt12Runner,
    )
    expect(sink.body?.bolt12Offer).toBe("lno1testoffer")
    expect(sink.body && "lightningAddress" in sink.body).toBe(false)
  })
})

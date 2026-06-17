import { describe, expect, test } from "bun:test"

import { claimTipReadiness } from "../src/tips"

const captureFetch = (sink: { body: Record<string, unknown> | null }) =>
  (async (_url: unknown, init: { body: string }) => {
    sink.body = JSON.parse(init.body) as Record<string, unknown>
    return new Response(JSON.stringify({ tipRecipientReadiness: { state: "ready" } }), {
      status: 200,
      headers: { "content-type": "application/json" },
    })
  }) as unknown as typeof fetch

describe("claimTipReadiness Spark Lightning Address (#5181)", () => {
  test("publishes Spark Lightning Address without minting an MDK BOLT 12 offer", async () => {
    const sink: { body: Record<string, unknown> | null } = { body: null }
    await claimTipReadiness(
      { baseUrl: "https://x.test", agentToken: "t", fetch: captureFetch(sink) },
      { pylonRef: "pylon.test.one", lightningAddress: "oa-abc123@spark.example" },
    )
    expect(sink.body && "bolt12Offer" in sink.body).toBe(false)
    expect(sink.body?.lightningAddress).toBe("oa-abc123@spark.example")
    expect(sink.body?.providerClass).toBe("external_lightning")
    expect(sink.body?.readinessRefs).toEqual([
      "readiness.public.spark_lightning_address.receive_ready",
      "readiness.public.spark_primary.agent_balance",
    ])
  })

  test("requires a Spark Lightning Address", async () => {
    const sink: { body: Record<string, unknown> | null } = { body: null }
    await expect(
      claimTipReadiness(
        { baseUrl: "https://x.test", agentToken: "t", fetch: captureFetch(sink) },
        { pylonRef: "pylon.test.one" },
      ),
    ).rejects.toThrow("Spark Lightning Address is required")
    expect(sink.body).toBeNull()
  })
})

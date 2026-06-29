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
    expect(sink.body && "sparkAddress" in sink.body).toBe(false)
    expect(sink.body?.lightningAddress).toBe("oa-abc123@spark.example")
    expect(sink.body?.providerClass).toBe("external_lightning")
    expect(sink.body?.readinessRefs).toEqual([
      "readiness.public.spark_lightning_address.receive_ready",
      "readiness.public.spark_primary.agent_balance",
    ])
  })

  test("requires a Spark address or a Spark Lightning Address", async () => {
    const sink: { body: Record<string, unknown> | null } = { body: null }
    await expect(
      claimTipReadiness(
        { baseUrl: "https://x.test", agentToken: "t", fetch: captureFetch(sink) },
        { pylonRef: "pylon.test.one" },
      ),
    ).rejects.toThrow("native Spark address or a Spark Lightning Address is required")
    expect(sink.body).toBeNull()
  })
})

describe("claimTipReadiness native Spark address (#5345)", () => {
  test("registers the native Spark address with no Lightning Address (no LSP)", async () => {
    const sink: { body: Record<string, unknown> | null } = { body: null }
    await claimTipReadiness(
      { baseUrl: "https://x.test", agentToken: "t", fetch: captureFetch(sink) },
      { pylonRef: "pylon.test.one", sparkAddress: "spark1pgssyuuuhnrrdjswal5c3s3rafw9w3y5dd4cjy3duxlf7hjzkp0rqx6dj6mrhu" },
    )
    expect(sink.body?.sparkAddress).toBe("spark1pgssyuuuhnrrdjswal5c3s3rafw9w3y5dd4cjy3duxlf7hjzkp0rqx6dj6mrhu")
    expect(sink.body && "lightningAddress" in sink.body).toBe(false)
    expect(sink.body && "bolt12Offer" in sink.body).toBe(false)
    // Native Spark is the node's own self-custodial wallet.
    expect(sink.body?.providerClass).toBe("mdk_agent_wallet")
    expect(sink.body?.readinessRefs).toEqual([
      "readiness.public.spark_address.offline_receive_ready",
      "readiness.public.spark_primary.agent_balance",
    ])
  })

  test("keeps the Lightning Address as an optional add alongside the Spark address", async () => {
    const sink: { body: Record<string, unknown> | null } = { body: null }
    await claimTipReadiness(
      { baseUrl: "https://x.test", agentToken: "t", fetch: captureFetch(sink) },
      {
        pylonRef: "pylon.test.one",
        sparkAddress: "spark1pgssyuuuhnrrdjswal5c3s3rafw9w3y5dd4cjy3duxlf7hjzkp0rqx6dj6mrhu",
        lightningAddress: "oa-abc123@spark.example",
      },
    )
    expect(sink.body?.sparkAddress).toBe("spark1pgssyuuuhnrrdjswal5c3s3rafw9w3y5dd4cjy3duxlf7hjzkp0rqx6dj6mrhu")
    expect(sink.body?.lightningAddress).toBe("oa-abc123@spark.example")
    // Spark-rail self-custody takes precedence for the provider class.
    expect(sink.body?.providerClass).toBe("mdk_agent_wallet")
  })
})

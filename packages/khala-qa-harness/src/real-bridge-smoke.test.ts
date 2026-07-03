import { describe, expect, test } from "bun:test"

import { runKhalaCodeRealBridgeSeedSmoke } from "./real-bridge-smoke.js"

describe("Khala Code real bridge seed smoke", () => {
  test("boots the real HTTP bridge with bearer auth, SSE, and fixture app-server command", async () => {
    const report = await runKhalaCodeRealBridgeSeedSmoke()

    expect(report.status).toBe("pass")
    expect(report.bearerAuth).toMatchObject({
      acceptedStatus: 200,
      rejectedStatus: 401,
    })
    expect(report.sse).toMatchObject({
      connected: true,
      observedChatTurnEvent: true,
    })
    expect(report.sse.contentType).toContain("text/event-stream")
    expect(report.bridge.command).toContain("fixture-codex-app-server-command")
    expect(report.bridge.scenarioSource).toBe("seed_corpus_transport_valid")
    expect(report.scenarioCount).toBeGreaterThan(50)
    expect(report.scenarios.every(scenario => scenario.status === "pass")).toBe(true)
  }, 20_000)
})

import { Effect } from "effect"
import { describe, expect, test } from "vite-plus/test"

import {
  DESKTOP_CODEX_USAGE_INGEST_PATH,
  DESKTOP_CODEX_USAGE_SCHEMA_VERSION,
  makeDesktopCodexUsageReporter,
  type DesktopCodexUsageReport,
} from "./desktop-codex-usage-reporter.ts"

const report: DesktopCodexUsageReport = {
  turnRef: "turn.desktop.1",
  model: "gpt-5.6-sol",
  observedAt: "2026-07-16T14:30:00.000Z",
  usage: {
    inputTokens: 120,
    cachedInputTokens: 40,
    outputTokens: 30,
    reasoningTokens: 10,
    totalTokens: 160,
  },
}

const credential = {
  ownerUserId: "owner-1",
  accessToken: "access-secret",
  refreshToken: "refresh-secret",
}

describe("makeDesktopCodexUsageReporter", () => {
  test("main keeps reporting hard-disabled at the shared ordinary and Full Auto completion seam", () => {
    const source = readFileSync(path.join(import.meta.dirname, "main.ts"), "utf8")
    const codexLaneStart = source.indexOf("const codexLocalLane")
    const completionStart = source.indexOf('if (turnEvent.kind === "turn_completed"', codexLaneStart)
    const persistenceStart = source.indexOf("// Persist reasoning/notice lines", completionStart)
    const completionSource = source.slice(completionStart, persistenceStart)

    expect(source).toContain("consentEnabled: () => false")
    expect(completionSource).toContain("desktopCodexUsageReporter.report")
    expect(completionSource).not.toContain("request.fullAuto")
    expect(source.match(/desktopCodexUsageReporter\.report/g)).toHaveLength(1)
  })

  test("default-off consent performs zero credential reads and zero requests", async () => {
    let credentialReads = 0
    let requests = 0
    const reporter = makeDesktopCodexUsageReporter({
      consentEnabled: () => false,
      sessionReady: () => true,
      credential: () => {
        credentialReads += 1
        return credential
      },
      baseUrl: "https://openagents.test",
      fetch: async () => {
        requests += 1
        return new Response(null, { status: 204 })
      },
    })

    await Effect.runPromise(reporter.report(report))

    expect(credentialReads).toBe(0)
    expect(requests).toBe(0)
  })

  test("a session that is not server-verified performs zero requests", async () => {
    let requests = 0
    const reporter = makeDesktopCodexUsageReporter({
      consentEnabled: () => true,
      sessionReady: () => false,
      credential: () => credential,
      baseUrl: "https://openagents.test",
      fetch: async () => {
        requests += 1
        return new Response(null, { status: 204 })
      },
    })

    await Effect.runPromise(reporter.report(report))

    expect(requests).toBe(0)
  })

  test("opted-in verified reporting sends only the exact bounded usage contract", async () => {
    const requests: Array<Readonly<{ url: string; init: RequestInit }>> = []
    const reporter = makeDesktopCodexUsageReporter({
      consentEnabled: () => true,
      sessionReady: () => true,
      credential: () => credential,
      baseUrl: "https://openagents.test/base",
      fetch: async (url, init) => {
        requests.push({ url: String(url), init: init ?? {} })
        return new Response(null, { status: 204 })
      },
    })

    await Effect.runPromise(reporter.report(report))

    expect(requests).toHaveLength(1)
    expect(requests[0]!.url).toBe(`https://openagents.test${DESKTOP_CODEX_USAGE_INGEST_PATH}`)
    expect(requests[0]!.init.headers).toEqual({
      authorization: "Bearer access-secret",
      "content-type": "application/json",
      "idempotency-key": "desktop.codex.turn.turn.desktop.1",
    })
    expect(JSON.parse(String(requests[0]!.init.body))).toEqual({
      schemaVersion: DESKTOP_CODEX_USAGE_SCHEMA_VERSION,
      ...report,
    })
    expect(String(requests[0]!.init.body)).not.toContain("owner-1")
  })

  test("invalid usage and transport or server failures remain fail-soft", async () => {
    let requests = 0
    const rejecting = makeDesktopCodexUsageReporter({
      consentEnabled: () => true,
      sessionReady: () => true,
      credential: () => credential,
      baseUrl: "https://openagents.test",
      fetch: async () => {
        requests += 1
        if (requests === 1) {
          throw new Error("offline")
        }
        return new Response(null, { status: 503 })
      },
    })

    await expect(Effect.runPromise(rejecting.report(report))).resolves.toBeUndefined()
    await expect(Effect.runPromise(rejecting.report(report))).resolves.toBeUndefined()
    await expect(Effect.runPromise(rejecting.report({
      ...report,
      usage: { ...report.usage, totalTokens: 0 },
    }))).resolves.toBeUndefined()
    await expect(Effect.runPromise(rejecting.report({
      ...report,
      usage: { ...report.usage, totalTokens: report.usage.totalTokens - 1 },
    }))).resolves.toBeUndefined()
    expect(requests).toBe(2)
  })
})
import { readFileSync } from "node:fs"
import path from "node:path"

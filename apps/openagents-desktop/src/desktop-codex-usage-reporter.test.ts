import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { Effect } from "effect"
import { afterEach, describe, expect, test } from "vite-plus/test"

import {
  DESKTOP_CODEX_USAGE_ADMISSION_PATH,
  DESKTOP_CODEX_USAGE_ADMISSION_SCHEMA_VERSION,
  DESKTOP_CODEX_USAGE_INGEST_PATH,
  DESKTOP_CODEX_USAGE_SCHEMA_VERSION,
  makeDesktopCodexUsageReporter,
  type DesktopCodexUsageReport,
} from "./desktop-codex-usage-reporter.ts"
import { openDesktopCodexUsageOutbox } from "./desktop-codex-usage-outbox.ts"

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
const directories: string[] = []
const outbox = (now: () => Date = () => new Date("2026-07-16T14:00:00.000Z")) => {
  const directory = mkdtempSync(path.join(tmpdir(), "oa-usage-outbox-"))
  directories.push(directory)
  const file = path.join(directory, "outbox.json")
  return { file, value: openDesktopCodexUsageOutbox(file, now) }
}
afterEach(() => {
  for (const directory of directories.splice(0)) rmSync(directory, { recursive: true, force: true })
})

describe("makeDesktopCodexUsageReporter", () => {
  test("main keeps both rollout and consent default-off at the ordinary and Full Auto seam", () => {
    const source = readFileSync(path.join(import.meta.dirname, "main.ts"), "utf8")
    const codexLaneStart = source.indexOf("const codexLocalLane")
    const completionStart = source.indexOf('if (turnEvent.kind === "turn_completed"', codexLaneStart)
    const completionSource = source.slice(completionStart, source.indexOf("// Persist reasoning/notice lines", completionStart))

    expect(source).toContain('OPENAGENTS_DESKTOP_USAGE_CONSENT_CONTROL === "1"')
    expect(source).toContain("preferencesStore.snapshot().privacy.shareLocalCodexUsage")
    expect(source).toContain("desktopCodexUsageReporter.admit")
    expect(completionSource).toContain("desktopCodexUsageReporter.report")
    expect(completionSource).not.toContain("request.fullAuto")
    expect(source.match(/desktopCodexUsageReporter\.report/g)).toHaveLength(1)
  })

  test("default-off consent performs zero credential reads, writes, and requests", async () => {
    let credentialReads = 0
    let requests = 0
    const queue = outbox()
    const reporter = makeDesktopCodexUsageReporter({
      consentEnabled: () => false,
      sessionReady: () => true,
      credential: () => { credentialReads += 1; return credential },
      outbox: queue.value,
      baseUrl: "https://openagents.test",
      fetch: async () => { requests += 1; return new Response(null, { status: 204 }) },
    })

    await reporter.admit({ turnRef: report.turnRef, model: report.model })
    await Effect.runPromise(reporter.report(report))
    await Effect.runPromise(reporter.flush())

    expect(credentialReads).toBe(0)
    expect(requests).toBe(0)
    expect(queue.value.snapshot()).toEqual([])
  })

  test("an opted-in verified turn is pre-admitted and sends only the bounded contract", async () => {
    const queue = outbox()
    const requests: Array<Readonly<{ url: string; init: RequestInit }>> = []
    const reporter = makeDesktopCodexUsageReporter({
      consentEnabled: () => true,
      sessionReady: () => true,
      credential: () => credential,
      outbox: queue.value,
      baseUrl: "https://openagents.test/base",
      fetch: async (url, init) => {
        requests.push({ url: String(url), init: init ?? {} })
        return String(url).endsWith(DESKTOP_CODEX_USAGE_ADMISSION_PATH)
          ? Response.json({
              schemaVersion: DESKTOP_CODEX_USAGE_ADMISSION_SCHEMA_VERSION,
              admissionRef: "admission.desktop.codex.1",
              admittedAt: "2026-07-16T14:00:00.000Z",
              expiresAt: "2026-07-17T14:00:00.000Z",
            }, { status: 201 })
          : Response.json({ insertedTokenUsage: true }, { status: 200 })
      },
    })

    await reporter.admit({ turnRef: report.turnRef, model: report.model })
    await Effect.runPromise(reporter.report(report))

    expect(requests.map(value => value.url)).toEqual([
      `https://openagents.test${DESKTOP_CODEX_USAGE_ADMISSION_PATH}`,
      `https://openagents.test${DESKTOP_CODEX_USAGE_INGEST_PATH}`,
    ])
    const ingest = requests[1]!
    expect(ingest.init.headers).toEqual({
      authorization: "Bearer access-secret",
      "content-type": "application/json",
      "idempotency-key": "desktop.codex.turn.turn.desktop.1",
    })
    expect(JSON.parse(String(ingest.init.body))).toEqual({
      schemaVersion: DESKTOP_CODEX_USAGE_SCHEMA_VERSION,
      admissionRef: "admission.desktop.codex.1",
      ...report,
    })
    expect(String(ingest.init.body)).not.toMatch(/owner-1|access-secret|refresh-secret|prompt|workspace|path/i)
    expect(queue.value.snapshot()).toEqual([])
  })

  test("a transient failure persists a credential-free retry across restart", async () => {
    let now = new Date("2026-07-16T14:00:00.000Z")
    const queue = outbox(() => now)
    const admission = {
      schemaVersion: DESKTOP_CODEX_USAGE_ADMISSION_SCHEMA_VERSION,
      admissionRef: "admission.desktop.codex.retry",
      admittedAt: now.toISOString(),
      expiresAt: "2026-07-17T14:00:00.000Z",
    }
    let requests = 0
    const first = makeDesktopCodexUsageReporter({
      consentEnabled: () => true,
      sessionReady: () => true,
      credential: () => credential,
      outbox: queue.value,
      baseUrl: "https://openagents.test",
      fetch: async (url) => {
        requests += 1
        if (String(url).endsWith(DESKTOP_CODEX_USAGE_ADMISSION_PATH)) return Response.json(admission, { status: 201 })
        throw new Error("offline")
      },
    })
    await first.admit({ turnRef: report.turnRef, model: report.model })
    await Effect.runPromise(first.report(report))
    expect(queue.value.snapshot()[0]).toMatchObject({ attempts: 1, report: { usage: report.usage } })
    expect(JSON.stringify(queue.value.snapshot())).not.toMatch(/access-secret|refresh-secret|owner-1/)

    now = new Date("2026-07-16T14:00:31.000Z")
    const reopened = openDesktopCodexUsageOutbox(queue.file, () => now)
    const second = makeDesktopCodexUsageReporter({
      consentEnabled: () => true,
      sessionReady: () => true,
      credential: () => credential,
      outbox: reopened,
      baseUrl: "https://openagents.test",
      fetch: async () => Response.json({ insertedTokenUsage: true }),
    })
    await Effect.runPromise(second.flush())
    expect(reopened.snapshot()).toEqual([])
    expect(requests).toBe(2)
  })
})

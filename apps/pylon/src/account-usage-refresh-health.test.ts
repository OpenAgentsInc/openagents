import { mkdtemp } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, expect, test } from "vite-plus/test"

import { loadCodexAccountHealthRecord } from "./codex-account-health-ledger.js"
import {
  recordCodexUsageRefreshFailure,
  recordCodexUsageRefreshSuccess,
} from "./account-usage-refresh-health.js"

describe("Codex account usage refresh health", () => {
  test("persists a revoked refresh failure so readiness cannot stay ready", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-usage-refresh-health-"))
    const summary = { paths: { home } } as never
    const blockerRefs = await recordCodexUsageRefreshFailure(summary, {
      accountRefHash: "account.pylon.codex.revoked",
      error: new Error("refresh token was revoked; bearer secret-not-retained"),
      now: new Date("2026-07-13T08:00:00.000Z"),
    })

    expect(blockerRefs).toEqual([
      "blocker.pylon.accounts_usage.codex_refresh_credentials_revoked",
    ])
    expect(await loadCodexAccountHealthRecord(summary, "account.pylon.codex.revoked"))
      .toMatchObject({
        accountRefHash: "account.pylon.codex.revoked",
        observedAt: "2026-07-13T08:00:00.000Z",
        reason: "credentials_revoked",
      })
  })

  test("a successful real refresh clears stale blocking health", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-usage-refresh-health-"))
    const summary = { paths: { home } } as never
    await recordCodexUsageRefreshFailure(summary, {
      accountRefHash: "account.pylon.codex.recovered",
      error: new Error("usage limit exhausted"),
    })

    await recordCodexUsageRefreshSuccess(summary, "account.pylon.codex.recovered")

    expect(await loadCodexAccountHealthRecord(summary, "account.pylon.codex.recovered"))
      .toBeNull()
  })

  test("transient network failure is recorded but does not advertise a capacity blocker", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-usage-refresh-health-"))
    const summary = { paths: { home } } as never
    const blockerRefs = await recordCodexUsageRefreshFailure(summary, {
      accountRefHash: "account.pylon.codex.network",
      error: new Error("network socket unavailable"),
    })

    expect(blockerRefs).toEqual([])
    expect(await loadCodexAccountHealthRecord(summary, "account.pylon.codex.network"))
      .toMatchObject({ reason: "network" })
  })
})

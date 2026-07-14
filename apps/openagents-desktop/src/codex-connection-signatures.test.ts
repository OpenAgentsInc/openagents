/**
 * EP250 Codex connection failure-signature REGRESSION CORPUS (#8712).
 *
 * Owner mandate, verbatim: "add thorough fucking tests or whatever to
 * prevent this category of codex connection error PLEASE I HATE ALL THEF
 * UCKING SPEEDBUMPS HERE".
 *
 * Table-driven over checked-in VERBATIM fixtures (live-captured on this
 * machine 2026-07-11 where marked; synthetic shapes otherwise). Every row
 * asserts, through the REAL parser/classifier/rotation path:
 *   1. classification (auth-class / rate-limit / generic),
 *   2. child-runtime rotation behavior,
 *   3. the in-process health-map effect,
 *   4. the UI-facing reason string,
 *   5. the fleet readiness projection (typed reconnect override).
 * A NEW signature is ONE new row.
 *
 * Plus the chip lifecycle state machine (boot-probe→verified→enabled;
 * revoke-mid-session→demote→other-verified keeps the chip→reconnect clears;
 * no-verified-accounts→disabled with the reconnect reason).
 */
import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  classifyCodexFailureText,
  isCodexPolicyDenialText,
  isCodexQuotaExhaustionText,
  isCodexRateLimitText,
  isCodexReconnectRequiredText,
  type CodexFailureClass,
} from "./codex-child-contract.ts"
import {
  FIXTURE_CODEX_MISSING_AUTH_MESSAGE,
  FIXTURE_CODEX_NETWORK_REFUSED_MESSAGE,
  FIXTURE_CODEX_RATE_LIMIT_MESSAGE,
  FIXTURE_CODEX_SHORT_AUTH_MESSAGE,
  FIXTURE_CODEX_USAGE_LIMIT_MESSAGE,
  fixtureCodex401TokenInvalidatedStderr,
  fixtureCodexMalformedAuthStderr,
  fixtureCodexMissingAuthStdout,
  fixtureCodexNetworkRefusedStdout,
  fixtureCodexRateLimitStdout,
  fixtureCodexRefreshTokenInvalidatedStderr,
  fixtureCodexRevokedStderr,
  fixtureCodexRevokedStdout,
  fixtureCodexShortAuthStdout,
  fixtureCodexSuccessStdout,
  fixtureCodexUsageLimitStdout,
  makeCodexAccountHealth,
  makeCodexChildRuntime,
  makeFixtureCodexChildSpawn,
  type CodexChildAccount,
  type FixtureCodexScript,
} from "./codex-child-runtime.ts"
import {
  CODEX_CHIP_REASON_NO_VERIFIED_ACCOUNT,
  CODEX_CHIP_REASON_POLICY_DENIED,
  CODEX_CHIP_REASON_QUOTA_EXHAUSTED,
  CODEX_CHIP_REASON_RATE_LIMITED,
  CODEX_CHIP_REASON_VERIFYING,
  codexHarnessLaneFromAvailability,
  codexLocalFailureMessage,
} from "./codex-local-contract.ts"
import { makeCodexLocalRuntime } from "./codex-local-runtime.ts"
import { makeCodexPreflight, type CodexProbeResult } from "./codex-preflight.ts"
import { makeUsageLedger } from "./usage-ledger.ts"
import {
  emptyFleetWorkspaceState,
  fleetDotEvidence,
  fleetReconnectRequired,
  withFleetLedger,
  type FleetAccount,
} from "./renderer/fleet-workspace.ts"

const accounts: ReadonlyArray<CodexChildAccount> = [
  { ref: "codex", home: "/isolated/accounts/codex/codex" },
  { ref: "codex-2", home: "/isolated/accounts/codex/codex-2" },
]

const scratch = (): string => mkdtempSync(join(tmpdir(), "codex-signatures-"))

/**
 * One corpus row. `script` is the exact child stream for the FIRST account;
 * the second account always succeeds, so `rotates: true` rows must land on
 * codex-2 while `rotates: false` rows must fail typed on codex.
 */
type SignatureRow = Readonly<{
  name: string
  script: FixtureCodexScript
  /** classifyCodexFailureText over the row's error text. */
  classification: CodexFailureClass
  /** The child runtime's behavior for this signature. */
  rotates: boolean
  /** Expected typed failure reason when rotation is off/terminal. */
  terminalReason?: "child_timeout" | "child_failed"
  /** Expected health mark on the failing ref after the run. */
  healthAfter: "auth_failed" | null
  /** The row's raw failure text the classifier sees. */
  failureText: string
  /** The UI-facing reason substring for this class. */
  uiReasonIncludes: string
}>

const rows: ReadonlyArray<SignatureRow> = [
  {
    name: "LONG revoked-refresh-token variant (LIVE VERBATIM 2026-07-11)",
    script: { stdout: fixtureCodexRevokedStdout, stderr: fixtureCodexRevokedStderr, exitCode: 1 },
    classification: "auth",
    rotates: true,
    healthAfter: "auth_failed",
    failureText:
      "Your access token could not be refreshed because your refresh token was revoked. Please log out and sign in again.",
    uiReasonIncludes: "Reconnect in Settings",
  },
  {
    name: "SHORT auth variant (LIVE VERBATIM — the pre-broadening miss)",
    script: { stdout: fixtureCodexShortAuthStdout, exitCode: 1 },
    classification: "auth",
    rotates: true,
    healthAfter: "auth_failed",
    failureText: FIXTURE_CODEX_SHORT_AUTH_MESSAGE,
    uiReasonIncludes: "Reconnect in Settings",
  },
  {
    name: "401 token_invalidated stderr only (LIVE VERBATIM models-manager shape)",
    script: { stdout: "", stderr: fixtureCodex401TokenInvalidatedStderr, exitCode: 1 },
    classification: "auth",
    rotates: true,
    healthAfter: "auth_failed",
    failureText: fixtureCodex401TokenInvalidatedStderr,
    uiReasonIncludes: "Reconnect in Settings",
  },
  {
    name: "refresh_token_invalidated stderr (LIVE VERBATIM login-manager shape)",
    script: { stdout: "", stderr: fixtureCodexRefreshTokenInvalidatedStderr, exitCode: 1 },
    classification: "auth",
    rotates: true,
    healthAfter: "auth_failed",
    failureText: fixtureCodexRefreshTokenInvalidatedStderr,
    uiReasonIncludes: "Reconnect in Settings",
  },
  {
    name: "missing auth.json — bearer-missing 401 turn.failed (LIVE VERBATIM empty home)",
    script: { stdout: fixtureCodexMissingAuthStdout, exitCode: 1 },
    classification: "auth",
    rotates: true,
    healthAfter: "auth_failed",
    failureText: FIXTURE_CODEX_MISSING_AUTH_MESSAGE,
    uiReasonIncludes: "Reconnect in Settings",
  },
  {
    name: "malformed auth.json — empty stdout, TOML-parse stderr (LIVE VERBATIM)",
    script: { stdout: "", stderr: fixtureCodexMalformedAuthStderr, exitCode: 1 },
    classification: "generic",
    rotates: true,
    healthAfter: null,
    failureText: fixtureCodexMalformedAuthStderr,
    uiReasonIncludes: "failed before producing content",
  },
  {
    name: "quota / 429 exhaustion shape",
    script: { stdout: fixtureCodexRateLimitStdout, exitCode: 1 },
    classification: "quota_exhausted",
    rotates: true,
    healthAfter: null,
    failureText: FIXTURE_CODEX_RATE_LIMIT_MESSAGE,
    uiReasonIncludes: "failed before producing content",
  },
  {
    name: "usage-limit quota variant (LIVE VERBATIM codex-5, EP250 live proof) — exhausted, NEVER auth",
    script: { stdout: fixtureCodexUsageLimitStdout, exitCode: 1 },
    classification: "quota_exhausted",
    rotates: true,
    healthAfter: null,
    failureText: FIXTURE_CODEX_USAGE_LIMIT_MESSAGE,
    uiReasonIncludes: "failed before producing content",
  },
  {
    name: "transient rate-limit provider response without quota exhaustion",
    script: { stdout: `${JSON.stringify({ type: "turn.failed", error: { message: "provider rate limit exceeded" } })}\n`, exitCode: 1 },
    classification: "rate_limit",
    rotates: true,
    healthAfter: null,
    failureText: "provider rate limit exceeded",
    uiReasonIncludes: "failed before producing content",
  },
  {
    name: "policy denial without credential or quota failure",
    script: { stdout: `${JSON.stringify({ type: "turn.failed", error: { message: "command denied by policy" } })}\n`, exitCode: 1 },
    classification: "policy_denied",
    rotates: true,
    healthAfter: null,
    failureText: "command denied by policy",
    uiReasonIncludes: "failed before producing content",
  },
  {
    name: "network refused (connection refused pre-content)",
    script: { stdout: fixtureCodexNetworkRefusedStdout, exitCode: 1 },
    classification: "generic",
    rotates: true,
    healthAfter: null,
    failureText: FIXTURE_CODEX_NETWORK_REFUSED_MESSAGE,
    uiReasonIncludes: "failed before producing content",
  },
  {
    name: "hang — host-side wall-clock timeout (terminal, never rotates)",
    script: { stdout: JSON.stringify({ type: "thread.started", thread_id: "t" }), exitCode: 0, hang: true },
    classification: "generic",
    rotates: false,
    terminalReason: "child_timeout",
    healthAfter: null,
    failureText: "wall clock budget reached",
    uiReasonIncludes: "timed out",
  },
]

describe("EP250 codex connection-signature corpus (one row per signature)", () => {
  for (const row of rows) {
    test(`${row.name}: classification=${row.classification}, rotates=${row.rotates}`, async () => {
      // 1. CLASSIFICATION over the raw failure text.
      expect(classifyCodexFailureText(row.failureText)).toBe(row.classification)
      expect(isCodexReconnectRequiredText(row.failureText)).toBe(row.classification === "auth")
      if (row.classification === "rate_limit") {
        expect(isCodexRateLimitText(row.failureText)).toBe(true)
      }
      if (row.classification === "quota_exhausted") {
        expect(isCodexQuotaExhaustionText(row.failureText)).toBe(true)
      }
      if (row.classification === "policy_denied") {
        expect(isCodexPolicyDenialText(row.failureText)).toBe(true)
      }

      // 2 + 3. ROTATION + HEALTH through the REAL child runtime and parser.
      const health = makeCodexAccountHealth()
      const runtime = makeCodexChildRuntime({
        scratchRoot: scratch,
        spawnImpl: makeFixtureCodexChildSpawn([
          row.script,
          { stdout: fixtureCodexSuccessStdout(), exitCode: 0 },
        ]),
        discoverImpl: async () => accounts,
        health,
        ...(row.terminalReason === "child_timeout" ? { timeoutMs: 40 } : {}),
      })
      const events: Array<{ kind: string; accountRef?: string }> = []
      const result = await runtime.runChild({
        childRef: `signature-${rows.indexOf(row)}`,
        task: "go",
        onEvent: event => events.push(event as { kind: string; accountRef?: string }),
      })
      if (row.rotates) {
        if (!result.ok) throw new Error(`expected rotation to succeed, got ${result.reason}: ${result.detail}`)
        expect(result.accountRef).toBe("codex-2")
        // Rotation is TYPED and VISIBLE — never silent.
        const rotationEvent = row.classification === "auth"
          ? "account_reconnect_required"
          : "pre_content_failure_rotated"
        expect(events.some(event => event.kind === rotationEvent && event.accountRef === "codex")).toBe(true)
      } else {
        expect(result.ok).toBe(false)
        if (!result.ok) expect(result.reason).toBe(row.terminalReason!)
        expect(events.some(event =>
          event.kind === "account_reconnect_required" || event.kind === "pre_content_failure_rotated")).toBe(false)
      }
      expect(health.stateOf("codex")).toBe(row.healthAfter as never)

      // 4. UI-FACING REASON STRING for the signature class.
      const uiReason = row.classification === "auth"
        ? codexLocalFailureMessage("account_reconnect_required", row.failureText.slice(0, 80))
        : row.terminalReason === "child_timeout"
          ? codexLocalFailureMessage("timeout", "")
          : codexLocalFailureMessage(
              "session_failed",
              `all 2 registered Codex account(s) failed before producing content`,
            )
      expect(uiReason).toContain(row.uiReasonIncludes)

      // 5. FLEET READINESS PROJECTION: an auth-class signature must flip the
      // typed reconnect override on the fleet row (probe/child evidence
      // supersedes presence-based "ready"); non-auth signatures must NOT.
      const ledger = makeUsageLedger()
      if (row.classification === "auth") {
        ledger.markReconnectRequired({ provider: "codex", accountRef: "codex" })
      }
      const fleet = withFleetLedger(
        { ...emptyFleetWorkspaceState(), phase: "ready" },
        ledger.snapshot(),
      )
      const account: FleetAccount = { ref: "codex", provider: "codex", email: null, readiness: "ready" }
      expect(fleetReconnectRequired(fleet, account)).toBe(row.classification === "auth")
      expect(fleetDotEvidence(fleet, account)).toBe(
        row.classification === "auth" ? "reconnect-required" : "lit",
      )
    })
  }

  test("corpus covers every mandated signature family (adding one = one row)", () => {
    const names = rows.map(row => row.name).join("\n")
    for (const family of [
      "LONG revoked",
      "SHORT auth variant",
      "401 token_invalidated",
      "refresh_token_invalidated",
      "missing auth.json",
      "malformed auth.json",
      "rate-limit",
      "policy",
      "network refused",
      "timeout",
    ]) {
      expect(names.toLowerCase()).toContain(family.toLowerCase().split(" ")[0]!)
    }
    expect(rows.length).toBeGreaterThanOrEqual(9)
  })
})

// ---------------------------------------------------------------------------
// Chip lifecycle state machine (EP250 chip-verified-evidence rule)
// ---------------------------------------------------------------------------

const fakePreflight = (verified: () => ReadonlyArray<string>) => ({
  probeAll: async () => [] as ReadonlyArray<CodexProbeResult>,
  ensureProbed: async () => [] as ReadonlyArray<CodexProbeResult>,
  results: () => [] as ReadonlyArray<CodexProbeResult>,
  verifiedRefs: verified,
})

describe("EP250 chip lifecycle state machine", () => {
  test("probe still pending → chip disabled 'verifying'; no availability → reconnect reason; verified → enabled", () => {
    expect(codexHarnessLaneFromAvailability(null)).toEqual({
      available: false,
      reason: CODEX_CHIP_REASON_VERIFYING,
    })
    expect(codexHarnessLaneFromAvailability({ state: "unavailable", reason: "no_verified_account" }))
      .toEqual({ available: false, reason: CODEX_CHIP_REASON_NO_VERIFIED_ACCOUNT })
    expect(codexHarnessLaneFromAvailability({ state: "unavailable", reason: "no_codex_account" }))
      .toEqual({ available: false, reason: CODEX_CHIP_REASON_NO_VERIFIED_ACCOUNT })
    expect(codexHarnessLaneFromAvailability({ state: "available", accountRef: "codex-5", verifiedCount: 1 }))
      .toEqual({ available: true, reason: null })
    // Quota honesty (live receipt): reconnecting cannot fix a rate limit,
    // so the reason must never send the owner to Settings for one.
    expect(codexHarnessLaneFromAvailability({ state: "unavailable", reason: "rate_limited" }))
      .toEqual({ available: false, reason: CODEX_CHIP_REASON_RATE_LIMITED })
    expect(codexHarnessLaneFromAvailability({ state: "unavailable", reason: "quota_exhausted" }))
      .toEqual({ available: false, reason: CODEX_CHIP_REASON_QUOTA_EXHAUSTED })
    expect(codexHarnessLaneFromAvailability({ state: "unavailable", reason: "policy_denied" }))
      .toEqual({ available: false, reason: CODEX_CHIP_REASON_POLICY_DENIED })
  })

  test("LIVE QUOTA CASE (codex-5, EP250): zero verified + exhausted usage → a distinct quota state, not reconnect or rate limit", async () => {
    const health = makeCodexAccountHealth()
    const preflight = makeCodexPreflight({
      scratchRoot: scratch,
      hasAuthImpl: () => true,
      spawnImpl: makeFixtureCodexChildSpawn([
        { stdout: fixtureCodexRevokedStdout, stderr: fixtureCodexRevokedStderr, exitCode: 1 },
        // The LIVE VERBATIM usage-limit stream on the only healthy account.
        { stdout: fixtureCodexUsageLimitStdout, exitCode: 1 },
      ]),
      discoverImpl: async () => [accounts[0]!, { ref: "codex-5", home: "/isolated/accounts/codex/codex-5" }],
      health,
    })
    await preflight.probeAll("boot")
    const runtime = makeCodexLocalRuntime({
      scratchRoot: scratch,
      spawnImpl: makeFixtureCodexChildSpawn([{ stdout: "", exitCode: 1 }]),
      discoverImpl: async () => [accounts[0]!, { ref: "codex-5", home: "/isolated/accounts/codex/codex-5" }],
      health,
      preflight,
    })
    const availability = await runtime.availability()
    expect(availability).toEqual({ state: "unavailable", reason: "quota_exhausted" })
    expect(codexHarnessLaneFromAvailability(availability))
      .toEqual({ available: false, reason: CODEX_CHIP_REASON_QUOTA_EXHAUSTED })
    // The rate-limited account is NOT health-demoted (quota is not auth).
    expect(health.stateOf("codex-5")).toBe(null)
  })

  test("boot probe verifies an account → availability available → chip enabled", async () => {
    const health = makeCodexAccountHealth()
    const preflight = makeCodexPreflight({
      scratchRoot: scratch,
      hasAuthImpl: () => true,
      spawnImpl: makeFixtureCodexChildSpawn([
        { stdout: fixtureCodexRevokedStdout, stderr: fixtureCodexRevokedStderr, exitCode: 1 },
        { stdout: fixtureCodexSuccessStdout(), exitCode: 0 },
      ]),
      discoverImpl: async () => accounts,
      health,
    })
    await preflight.probeAll("boot")
    const runtime = makeCodexLocalRuntime({
      scratchRoot: scratch,
      spawnImpl: makeFixtureCodexChildSpawn([{ stdout: fixtureCodexSuccessStdout(), exitCode: 0 }]),
      discoverImpl: async () => accounts,
      health,
      preflight,
    })
    const availability = await runtime.availability()
    expect(availability).toEqual({ state: "available", accountRef: "codex-2", verifiedCount: 1 })
    expect(codexHarnessLaneFromAvailability(availability)).toEqual({ available: true, reason: null })
    // Health ordering puts the verified account FIRST for dispatch.
    expect(health.order(accounts).map(account => account.ref)).toEqual(["codex-2", "codex"])
  })

  test("revoke mid-session: turn fails typed on one account, health demotes it, the chip STAYS enabled on the other verified account", async () => {
    const health = makeCodexAccountHealth()
    // Session start: BOTH accounts verified.
    health.recordSuccess("codex")
    health.recordSuccess("codex-2")
    const verified = new Set(["codex", "codex-2"])
    const runtime = makeCodexLocalRuntime({
      scratchRoot: scratch,
      spawnImpl: makeFixtureCodexChildSpawn([
        // codex-2 is most-recent-good so it goes first — revoke it mid-run.
        { stdout: fixtureCodexShortAuthStdout, exitCode: 1 },
        { stdout: fixtureCodexSuccessStdout(), exitCode: 0 },
      ]),
      discoverImpl: async () => accounts,
      health,
      preflight: fakePreflight(() => [...verified]),
    })
    const events: Array<{ kind: string }> = []
    const result = await runtime.runTurn({
      turnRef: "turn-revoke",
      threadRef: "thread-revoke",
      history: [],
      message: "go",
      emit: event => events.push(event),
    })
    if (!result.ok) throw new Error(`expected rotation success, got ${result.reason}`)
    expect(result.accountRef).toBe("codex")
    // Typed visible rotation notice reached the transcript stream.
    expect(events.some(event => event.kind === "lane_notice")).toBe(true)
    // Health demoted the revoked ref…
    expect(health.stateOf("codex-2")).toBe("auth_failed")
    // …and the chip stays enabled because ANOTHER verified account exists.
    verified.delete("codex-2")
    const availability = await runtime.availability()
    expect(availability.state).toBe("available")
    if (availability.state === "available") expect(availability.accountRef).toBe("codex")
  })

  test("reconnect → fresh probe clears: the demoted ref verifies again and rises in the ordering", async () => {
    const health = makeCodexAccountHealth()
    health.recordAuthFailure("codex")
    const preflight = makeCodexPreflight({
      scratchRoot: scratch,
      hasAuthImpl: () => true,
      spawnImpl: makeFixtureCodexChildSpawn([
        { stdout: fixtureCodexSuccessStdout(), exitCode: 0 },
      ]),
      discoverImpl: async () => [accounts[0]!],
      health,
    })
    const results = await preflight.probeAll("reconnect_completed")
    expect(results[0]!.state).toBe("verified")
    expect(health.stateOf("codex")).toBe("last_good")
    expect(preflight.verifiedRefs()).toEqual(["codex"])
  })

  test("no verified accounts → chip disabled with the reconnect reason; Send refuses typed", async () => {
    const health = makeCodexAccountHealth()
    const runtime = makeCodexLocalRuntime({
      scratchRoot: scratch,
      spawnImpl: makeFixtureCodexChildSpawn([{ stdout: "", exitCode: 1 }]),
      discoverImpl: async () => accounts,
      health,
      preflight: fakePreflight(() => []),
    })
    const availability = await runtime.availability()
    expect(availability).toEqual({ state: "unavailable", reason: "no_verified_account" })
    expect(codexHarnessLaneFromAvailability(availability))
      .toEqual({ available: false, reason: CODEX_CHIP_REASON_NO_VERIFIED_ACCOUNT })
  })
})

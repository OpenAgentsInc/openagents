import { inspectCodexFleet, type KhalaCodexFleetToolOptions } from "./khala-fleet-tools.js"

/**
 * Recurring Khala Sync `fleet_account` visibility reporter (openagents #8406).
 *
 * Background: `fleet.reportAccountState` (MC-2,
 * `packages/khala-sync-server/src/fleet-mutators.ts`) and the matching desktop
 * client mutator/RPC (`khalaSyncFleetReportAccountState`,
 * `clients/khala-code-desktop/src/bun/khala-sync-service.ts`) landed with only
 * a MANUAL one-off push proven end-to-end
 * (docs/khala-code/2026-07-04-mobile-tailnet-handshake.md, "fleet.reportAccountState
 * closes the account-visibility gap") — verified by reading every call site in
 * this repo before writing this file: nothing invoked that RPC on any
 * recurring basis for EITHER provider. This module is that missing recurring
 * caller, built to cover both `codex` and `claude_agent` local Pylon accounts
 * from day one rather than a Codex-only pass that would need a later "extend
 * to Claude" follow-up.
 *
 * Enumeration reuses `inspectCodexFleet` with `workerKind: "auto"` (already
 * merges `pylon accounts list --json` + `pylon accounts status --provider
 * <provider> --json` for BOTH providers — the same tested code path the
 * desktop's own Fleet panel and inbox use for local account inventory), so
 * this reporter carries no separate/parallel account-discovery logic.
 *
 * Honesty rules carried from the wider Khala Sync fleet work:
 * - Capacity numbers (`capacityAvailable`/`Busy`/`Queued`) are reported only
 *   when `inspectCodexFleet` actually resolved them (from a live
 *   `ownCapacityDispatch` provider projection); they are never fabricated or
 *   defaulted to a placeholder like `1`.
 * - Readiness is mapped from the real Pylon per-account readiness state
 *   (`CodexAgentReadinessState` / `ClaudeAgentReadinessState`,
 *   `apps/pylon/src/{codex,claude}-agent.ts`) into the Khala Sync
 *   `FleetAccountEntity`'s deliberately coarse 4-value public enum — never a
 *   1:1 passthrough of the richer local state.
 * - There is currently no stable per-owner "list my fleet runs" Khala Sync
 *   scope (a known, already-documented gap — see the mobile-tailnet-handshake
 *   doc's "Fleet runs are scoped per session, not per owner" section). This
 *   reporter does not invent one: it reports into whichever `runId`s an
 *   operator explicitly configures via `KHALA_SYNC_FLEET_ACCOUNT_REPORT_RUN_ID`
 *   (comma-separated). With none configured, `reportNow()` is an honest no-op
 *   (`skipped: "no_run_id_configured"`), never a guessed scope.
 */

export type FleetAccountStateReadiness = "ready" | "cooldown" | "unavailable" | "unknown"

export type FleetAccountStateReport = {
  readonly accountRefHash: string
  readonly provider: "codex" | "claude"
  readonly readiness: FleetAccountStateReadiness
  readonly capacityAvailable?: number
  readonly capacityBusy?: number
  readonly capacityQueued?: number
}

/**
 * Maps a raw Pylon per-account readiness state
 * (`CodexAgentReadinessState | ClaudeAgentReadinessState`) into the Khala Sync
 * `FleetAccountEntity`'s public-safe 4-value readiness enum:
 * - `ready` stays `ready`.
 * - `usage_limited` / `rate_limited` become `cooldown` — the account is
 *   expected to recover on its own without an owner action.
 * - `credentials_missing` / `credentials_revoked` / `sdk_missing` /
 *   `auth_error` / `platform_unsupported` / `disabled_by_config` become
 *   `unavailable` — the account is structurally blocked until an owner acts.
 * - `network` / `timeout` (transient local probe failures) and anything
 *   unrecognized become `unknown` — never guessed as `ready` or `cooldown`.
 */
export const fleetAccountReadinessFromPylonState = (
  state: string,
): FleetAccountStateReadiness => {
  switch (state) {
    case "ready":
      return "ready"
    case "usage_limited":
    case "rate_limited":
      return "cooldown"
    case "credentials_missing":
    case "credentials_revoked":
    case "sdk_missing":
    case "auth_error":
    case "platform_unsupported":
    case "disabled_by_config":
      return "unavailable"
    case "network":
    case "timeout":
    default:
      return "unknown"
  }
}

/** Maps the local Pylon account-registry provider tag to the public Khala Sync wire tag. */
export const fleetAccountWireProvider = (
  provider: "codex" | "claude_agent",
): "codex" | "claude" => (provider === "claude_agent" ? "claude" : "codex")

export type CollectLocalFleetAccountStateReportsOptions = KhalaCodexFleetToolOptions

/**
 * Enumerates every locally-registered Pylon account (both `codex` and
 * `claude_agent`) and projects each into the wire shape
 * `fleet.reportAccountState` expects. Accounts with no resolvable
 * `accountRefHash` (should not happen for a real registry/default/sibling
 * account, but defensively skipped rather than pushing a malformed ref) are
 * dropped.
 */
export async function collectLocalFleetAccountStateReports(
  options: CollectLocalFleetAccountStateReportsOptions = {},
): Promise<FleetAccountStateReport[]> {
  const fleet = await inspectCodexFleet(
    { includeProcesses: false, includeRateLimits: false, startPylon: false, workerKind: "auto" },
    options,
  )
  const reports: FleetAccountStateReport[] = []
  for (const account of fleet.accounts) {
    if (account.accountRefHash === null) continue
    const capacity = account.capacity
    reports.push({
      accountRefHash: account.accountRefHash,
      provider: fleetAccountWireProvider(account.provider),
      readiness: fleetAccountReadinessFromPylonState(account.readiness),
      ...(capacity?.available == null ? {} : { capacityAvailable: capacity.available }),
      ...(capacity?.busy == null ? {} : { capacityBusy: capacity.busy }),
      ...(capacity?.queued == null ? {} : { capacityQueued: capacity.queued }),
    })
  }
  return reports
}

// ---------------------------------------------------------------------------
// Config resolution
// ---------------------------------------------------------------------------

export type FleetAccountStateReporterEnv = Readonly<Record<string, string | undefined>>

export const KHALA_SYNC_FLEET_ACCOUNT_REPORT_RUN_ID_ENV = "KHALA_SYNC_FLEET_ACCOUNT_REPORT_RUN_ID"
export const KHALA_SYNC_FLEET_ACCOUNT_REPORT_INTERVAL_MS_ENV =
  "KHALA_SYNC_FLEET_ACCOUNT_REPORT_INTERVAL_MS"
export const KHALA_SYNC_FLEET_ACCOUNT_REPORT_DISABLED_ENV = "KHALA_SYNC_FLEET_ACCOUNT_REPORT_DISABLED"

const DEFAULT_FLEET_ACCOUNT_REPORT_INTERVAL_MS = 30_000

const boolEnv = (value: string | undefined): boolean => {
  const normalized = value?.trim().toLowerCase()
  return normalized === "1" || normalized === "true"
}

/**
 * Comma-separated Khala Sync `fleet_run` scope ids to report local account
 * state into. Never inferred/guessed — an operator (or the mobile app's own
 * `EXPO_PUBLIC_KHALA_SYNC_DEMO_FLEET_RUN_ID` counterpart) must name the run(s)
 * they want visible, since there is no stable per-owner fleet-roster scope
 * yet (a documented, separate follow-up).
 */
export const fleetAccountReportRunIds = (
  env: FleetAccountStateReporterEnv,
): readonly string[] => {
  const raw = env[KHALA_SYNC_FLEET_ACCOUNT_REPORT_RUN_ID_ENV]?.trim()
  if (raw === undefined || raw.length === 0) return []
  return raw.split(",").map(id => id.trim()).filter(id => id.length > 0)
}

/** Mirrors `startKhalaCodeDesktopTokenUsageBackgroundSync`'s disable/interval env convention. */
export const fleetAccountReportIntervalMs = (
  env: FleetAccountStateReporterEnv,
): number => {
  if (boolEnv(env[KHALA_SYNC_FLEET_ACCOUNT_REPORT_DISABLED_ENV])) return 0
  const explicit = env[KHALA_SYNC_FLEET_ACCOUNT_REPORT_INTERVAL_MS_ENV]?.trim()
  if (explicit === undefined || explicit.length === 0) {
    return DEFAULT_FLEET_ACCOUNT_REPORT_INTERVAL_MS
  }
  const parsed = Number.parseInt(explicit, 10)
  return Number.isSafeInteger(parsed) && parsed >= 0
    ? parsed
    : DEFAULT_FLEET_ACCOUNT_REPORT_INTERVAL_MS
}

// ---------------------------------------------------------------------------
// Scheduler
// ---------------------------------------------------------------------------

export type FleetAccountStateReporterKhalaSync = {
  readonly fleetReportAccountState: (
    request: {
      readonly runId: string
      readonly accountRefHash: string
      readonly readiness: FleetAccountStateReadiness
      readonly provider?: string
      readonly capacityAvailable?: number
      readonly capacityBusy?: number
      readonly capacityQueued?: number
    },
  ) => Promise<{ readonly ok: boolean; readonly error?: string | undefined }>
}

export type FleetAccountStateReportOutcome = {
  readonly runId: string
  readonly accountRefHash: string
  readonly provider: "codex" | "claude"
  readonly ok: boolean
  readonly error?: string
}

export type FleetAccountStateReportSyncResult =
  | { readonly skipped: "no_run_id_configured" }
  | {
      readonly reportCount: number
      readonly runIds: readonly string[]
      readonly outcomes: readonly FleetAccountStateReportOutcome[]
      readonly failedCount: number
    }

export type StartFleetAccountStateReporterOptions = {
  readonly env?: FleetAccountStateReporterEnv
  readonly runIds?: readonly string[]
  readonly intervalMs?: number
  readonly khalaSync: FleetAccountStateReporterKhalaSync
  readonly toolOptions?: CollectLocalFleetAccountStateReportsOptions
  readonly setInterval?: (callback: () => void, milliseconds: number) => unknown
  readonly clearInterval?: (timer: unknown) => void
  readonly onResult?: (result: FleetAccountStateReportSyncResult) => void
  readonly onError?: (error: unknown) => void
}

export type FleetAccountStateReporterHandle = {
  readonly dispose: () => void
  readonly reportNow: () => Promise<FleetAccountStateReportSyncResult | null>
}

/**
 * Starts the recurring reporter. Mirrors
 * `startKhalaCodeDesktopTokenUsageBackgroundSync`'s shape exactly
 * (`codex-token-usage-telemetry.ts`): injectable `setInterval`/`clearInterval`
 * for deterministic tests, an in-flight guard so overlapping ticks collapse
 * into one, an immediate first `reportNow()` at start, and a `dispose()` that
 * stops the timer and silences any in-flight tick's callbacks.
 */
export function startKhalaCodeDesktopFleetAccountStateReporter(
  options: StartFleetAccountStateReporterOptions,
): FleetAccountStateReporterHandle {
  const env = options.env ?? {}
  const intervalMs = options.intervalMs ?? fleetAccountReportIntervalMs(env)
  const setIntervalImpl = options.setInterval ??
    ((callback: () => void, milliseconds: number) => globalThis.setInterval(callback, milliseconds))
  const clearIntervalImpl = options.clearInterval ??
    ((timer: unknown) => globalThis.clearInterval(timer as ReturnType<typeof globalThis.setInterval>))
  let disposed = false
  let inFlight: Promise<FleetAccountStateReportSyncResult | null> | null = null

  const reportNow = (): Promise<FleetAccountStateReportSyncResult | null> => {
    if (disposed) return Promise.resolve(null)
    if (inFlight !== null) return inFlight
    inFlight = (async (): Promise<FleetAccountStateReportSyncResult> => {
      const runIds = options.runIds ?? fleetAccountReportRunIds(env)
      if (runIds.length === 0) return { skipped: "no_run_id_configured" }
      const reports = await collectLocalFleetAccountStateReports(options.toolOptions)
      const outcomes: FleetAccountStateReportOutcome[] = []
      for (const runId of runIds) {
        for (const report of reports) {
          const result = await options.khalaSync.fleetReportAccountState({
            accountRefHash: report.accountRefHash,
            provider: report.provider,
            readiness: report.readiness,
            runId,
            ...(report.capacityAvailable === undefined
              ? {}
              : { capacityAvailable: report.capacityAvailable }),
            ...(report.capacityBusy === undefined ? {} : { capacityBusy: report.capacityBusy }),
            ...(report.capacityQueued === undefined ? {} : { capacityQueued: report.capacityQueued }),
          })
          outcomes.push({
            accountRefHash: report.accountRefHash,
            provider: report.provider,
            runId,
            ok: result.ok,
            ...(result.error === undefined ? {} : { error: result.error }),
          })
        }
      }
      return {
        failedCount: outcomes.filter(outcome => !outcome.ok).length,
        outcomes,
        reportCount: outcomes.length,
        runIds,
      }
    })()
      .then(result => {
        options.onResult?.(result)
        return result
      })
      .catch((error: unknown) => {
        options.onError?.(error)
        return null
      })
      .finally(() => {
        inFlight = null
      })
    return inFlight
  }

  const timer = intervalMs > 0
    ? setIntervalImpl(() => {
      void reportNow()
    }, intervalMs)
    : null
  void reportNow()

  return {
    dispose: () => {
      disposed = true
      if (timer !== null) clearIntervalImpl(timer)
    },
    reportNow,
  }
}

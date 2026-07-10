import type { FleetAutoTargetSkipReason } from "@openagentsinc/khala-fleet-intents"
import {
  pylonAccountMarginalCostClasses,
  type PylonAccountMarginalCostClass,
} from "@openagentsinc/pylon-core/custody/account-registry"

import type { FleetRunSupervisorAccount } from "./fleet-run-supervisor.js"

export type PylonFleetMarginalCostClass = PylonAccountMarginalCostClass

export type PublicSafePylonFleetCapacityAccount = {
  readonly accountRef: string
  readonly capacity: {
    readonly available: number | null
    readonly ready: number | null
  } | null
  readonly isDefaultAccount?: boolean | undefined
  readonly marginalCostClass?: unknown
  readonly paused: boolean
  readonly provider: string
  readonly readiness: string
}

export type MapPylonFleetSupervisorCapacityOptions = {
  readonly allowDefaultAccount?: boolean | undefined
  readonly grokExecutionAvailable?: boolean | undefined
  // The Pylon-owned standing supervisor needs bounded zero-capacity rows so
  // the shared auto policy can record every skipped named candidate. Legacy
  // callers keep the historical ready-only projection by default.
  readonly includeUnavailableCandidates?: boolean | undefined
}

const supportedProviders = new Set(["codex", "claude_agent", "grok"])
const readyStates = new Set(["ready", "available"])

export function parsePylonFleetMarginalCostClass(value: unknown): PylonAccountMarginalCostClass | null {
  return typeof value === "string" &&
      (pylonAccountMarginalCostClasses as ReadonlyArray<string>).includes(value)
    ? value as PylonAccountMarginalCostClass
    : null
}

export function normalizePylonFleetMarginalCostClass(value: unknown): PylonAccountMarginalCostClass {
  return parsePylonFleetMarginalCostClass(value) ?? "not_measured"
}

const availableSlots = (account: PublicSafePylonFleetCapacityAccount): number => {
  const candidate = account.capacity?.available ?? account.capacity?.ready
  if (candidate === null || candidate === undefined) return 0
  return Number.isFinite(candidate) ? Math.max(0, Math.trunc(candidate)) : 0
}

const autoSkipReasonForReadiness = (readiness: string): FleetAutoTargetSkipReason => {
  const normalized = readiness.trim().toLowerCase().replaceAll("-", "_")
  if (
    normalized === "usage_limited" ||
    normalized === "account_usage_limited" ||
    normalized === "account_exhausted" ||
    normalized === "account_quota_exhausted" ||
    normalized === "weekly_exhausted"
  ) return "account_exhausted"
  if (
    normalized === "rate_limited" ||
    normalized === "account_rate_limited" ||
    normalized === "cooldown"
  ) return "account_rate_limited"
  if (
    normalized === "credentials_revoked" ||
    normalized === "account_credentials_revoked" ||
    normalized === "credentials_missing" ||
    normalized === "auth_required" ||
    normalized === "auth_error"
  ) return "account_requires_reauth"
  return "account_unavailable"
}

/**
 * Converts public-safe Pylon account status/capacity rows into the scheduler's
 * concrete mixed-account vocabulary.
 *
 * Provider is used only for the explicit harness mapping. Cost is always read
 * from the row and invalid/absent values become the honest `not_measured`;
 * provider names and account refs never influence economics. Unsupported
 * Unknown providers are skipped rather than silently substituted onto Codex.
 * Named Grok custody maps to an explicit Grok row, but its capacity remains
 * zero until the claimed-work executor is composed through a separate gate.
 */
export function mapPylonFleetSupervisorCapacity(
  accounts: readonly PublicSafePylonFleetCapacityAccount[],
  options: MapPylonFleetSupervisorCapacityOptions = {},
): readonly FleetRunSupervisorAccount[] {
  const allowDefaultAccount = options.allowDefaultAccount ?? true
  const grokExecutionAvailable = options.grokExecutionAvailable ?? false
  const includeUnavailableCandidates = options.includeUnavailableCandidates ?? false
  return accounts.flatMap((account): FleetRunSupervisorAccount[] => {
    if (!supportedProviders.has(account.provider)) return []
    const readinessIsReady = readyStates.has(account.readiness)
    if ((!readinessIsReady || account.paused) && !includeUnavailableCandidates) return []
    if (!allowDefaultAccount && account.isDefaultAccount === true) return []
    const accountRef = account.accountRef.trim()
    if (accountRef.length === 0) return []
    const workerKind =
      account.provider === "claude_agent"
        ? "claude" as const
        : account.provider === "grok"
          ? "grok" as const
          : "codex" as const
    const slots = availableSlots(account)
    const unavailabilityReason: FleetAutoTargetSkipReason | undefined =
      account.paused
        ? "account_unavailable"
        : !readinessIsReady
          ? autoSkipReasonForReadiness(account.readiness)
          : workerKind === "grok" && !grokExecutionAvailable
            ? "account_unavailable"
            : slots <= 0
              ? "account_unavailable"
              : undefined
    return [{
      accountRef,
      advertisedCapacity: unavailabilityReason === undefined ? slots : 0,
      marginalCostClass: normalizePylonFleetMarginalCostClass(account.marginalCostClass),
      workerKind,
      ...(includeUnavailableCandidates && unavailabilityReason !== undefined
        ? { unavailabilityReason }
        : {}),
    }]
  })
}

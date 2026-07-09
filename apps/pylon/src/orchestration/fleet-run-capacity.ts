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
}

const supportedProviders = new Set(["codex", "claude_agent"])
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

/**
 * Converts public-safe Pylon account status/capacity rows into the scheduler's
 * concrete mixed-account vocabulary.
 *
 * Provider is used only for the explicit harness mapping. Cost is always read
 * from the row and invalid/absent values become the honest `not_measured`;
 * provider names and account refs never influence economics. Unsupported
 * providers (including Grok until it has a Pylon account row) are skipped
 * rather than silently substituted onto Codex.
 */
export function mapPylonFleetSupervisorCapacity(
  accounts: readonly PublicSafePylonFleetCapacityAccount[],
  options: MapPylonFleetSupervisorCapacityOptions = {},
): readonly FleetRunSupervisorAccount[] {
  const allowDefaultAccount = options.allowDefaultAccount ?? true
  return accounts.flatMap((account): FleetRunSupervisorAccount[] => {
    if (!supportedProviders.has(account.provider)) return []
    if (!readyStates.has(account.readiness) || account.paused) return []
    if (!allowDefaultAccount && account.isDefaultAccount === true) return []
    const accountRef = account.accountRef.trim()
    if (accountRef.length === 0) return []
    return [{
      accountRef,
      advertisedCapacity: availableSlots(account),
      marginalCostClass: normalizePylonFleetMarginalCostClass(account.marginalCostClass),
      workerKind: account.provider === "claude_agent" ? "claude" : "codex",
    }]
  })
}

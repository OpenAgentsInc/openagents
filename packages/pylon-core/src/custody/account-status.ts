import {
  hashPylonAccountRef,
  loadPylonAccountRegistry,
  type PylonAccountMarginalCostClass,
  type PylonAccountProvider,
} from "./account-registry.js"
import {
  isAccountAvailable,
  loadManualQuotaResetRecord,
  loadQuotaRecord,
  type QuotaRecord,
} from "./account-quota-ledger.js"
import {
  countAccountRollingWindowLocalSessionTokens,
  loadAccountUsageStore,
  quotaStateFrom,
  type PylonAccountUsageStoreEntry,
  type PylonProviderRateLimitSnapshot,
} from "./account-usage.js"
import type { BootstrapSummary } from "../shared/bootstrap.js"
import { assertPublicProjectionSafe } from "../shared/state.js"

export const PYLON_OPERATOR_ACCOUNT_STATUS_SCHEMA =
  "openagents.pylon.operator_account_status.v0.1"

export type PylonOperatorAccountStatusEntry = {
  accountRefHash: string
  provider: PylonAccountProvider
  isRateLimited: boolean
  quotaState: "available" | "cooldown" | "weekly_exhausted" | "limited"
  cooldownExpiresAt: string | null
  hourlyCap: number | null
  hourlyUsage: number | null
  weeklyCap: number | null
  weeklyUsage: number | null
  manualResetsRemaining: number | null
  resetAllowed: boolean
  // MH-8 (#8587) economics wiring: the account's DATA-DRIVEN marginal cost
  // class, read straight from the account registry entry. Makes the `auto`
  // preference order inspectable/auditable via `pylon accounts status --json`
  // instead of a black box — never inferred from `provider` by name.
  marginalCostClass: PylonAccountMarginalCostClass
}

export type PylonOperatorAccountStatusProjection = {
  schema: typeof PYLON_OPERATOR_ACCOUNT_STATUS_SCHEMA
  observedAt: string
  accounts: PylonOperatorAccountStatusEntry[]
}

const fallbackQuotaCooldownMs = 3600_000

function fallbackCooldownExpiresAt(record: QuotaRecord): string | null {
  if (record.retryAtIso !== null) return record.retryAtIso
  const observedAt = Date.parse(record.observedAt)
  if (!Number.isFinite(observedAt)) return null
  return new Date(observedAt + fallbackQuotaCooldownMs).toISOString()
}

function cooldownExpiresAt(record: QuotaRecord | null, now: Date): string | null {
  if (record === null || isAccountAvailable(record, now)) return null
  return fallbackCooldownExpiresAt(record)
}

function usageFromPercent(cap: number | null, usedPercent: number): number {
  return cap === null ? Math.round(usedPercent) : Math.round((cap * usedPercent) / 100)
}

function snapshotWindowUsage(
  snapshots: PylonProviderRateLimitSnapshot[],
  kind: "hourly" | "weekly",
  cap: number | null,
): number | null {
  const targetMinutes = kind === "hourly" ? 60 : 7 * 24 * 60
  for (const snapshot of snapshots) {
    for (const window of [snapshot.primary, snapshot.secondary]) {
      if (window === null) continue
      const minutes = window.windowMinutes
      const labelMatches = kind === "weekly"
        ? window.label === "weekly"
        : window.label === "hourly"
      const minutesMatch = minutes !== null && Math.abs(minutes - targetMinutes) <= targetMinutes * 0.1
      if (minutesMatch || labelMatches) return usageFromPercent(cap, window.usedPercent)
    }
  }
  return null
}

function usageFor(
  entry: PylonAccountUsageStoreEntry | undefined,
  kind: "hourly" | "weekly",
  cap: number | null,
  now: Date,
): number | null {
  const snapshots = entry?.providerTruth?.snapshots ?? []
  const fromProvider = snapshotWindowUsage(snapshots, kind, cap)
  if (fromProvider !== null) return fromProvider
  if (kind === "hourly") return countAccountRollingWindowLocalSessionTokens(entry, { now, windowMinutes: 60 })
  return null
}

export async function collectPylonOperatorAccountStatus(
  summary: Pick<BootstrapSummary, "paths">,
  options: { now?: Date } = {},
): Promise<PylonOperatorAccountStatusProjection> {
  const now = options.now ?? new Date()
  const registry = await loadPylonAccountRegistry(summary)
  const usageStore = await loadAccountUsageStore(summary)
  const accounts: PylonOperatorAccountStatusEntry[] = []

  for (const account of registry) {
    const accountRefHash = hashPylonAccountRef(account.provider, account.ref)
    const quotaRecord = await loadQuotaRecord(summary as BootstrapSummary, accountRefHash)
    const resetRecord = await loadManualQuotaResetRecord(summary as BootstrapSummary, {
      accountRefHash,
      provider: account.provider,
      defaultManualResetsRemaining: account.manualResetsRemaining,
    })
    const usageEntry = usageStore.accounts[accountRefHash]
    const quotaState = quotaStateFrom(quotaRecord, now)
    accounts.push({
      accountRefHash,
      provider: account.provider,
      isRateLimited: quotaState.limited,
      quotaState: quotaState.state,
      cooldownExpiresAt: cooldownExpiresAt(quotaRecord, now),
      hourlyCap: account.hourlyCap,
      hourlyUsage: usageFor(usageEntry, "hourly", account.hourlyCap, now),
      weeklyCap: account.weeklyCap,
      weeklyUsage: usageFor(usageEntry, "weekly", account.weeklyCap, now),
      manualResetsRemaining: resetRecord.manualResetsRemaining,
      resetAllowed: quotaState.state === "weekly_exhausted",
      marginalCostClass: account.marginalCostClass,
    })
  }

  const projection = {
    schema: PYLON_OPERATOR_ACCOUNT_STATUS_SCHEMA,
    observedAt: now.toISOString(),
    accounts,
  } satisfies PylonOperatorAccountStatusProjection
  assertPublicProjectionSafe(projection)
  return projection
}

import type { FleetAccountEntity, FleetRunEntity, FleetWorkerEntity } from "@openagentsinc/khala-sync"

export const fleetAccountIdOf = (account: FleetAccountEntity): string => account.accountRefHash
export const fleetWorkerIdOf = (worker: FleetWorkerEntity): string => worker.workerId
export const fleetRunIdOf = (run: FleetRunEntity): string => run.runId

export const sortWorkersByIdAsc = (
  workers: ReadonlyArray<FleetWorkerEntity>
): ReadonlyArray<FleetWorkerEntity> => [...workers].sort((a, b) => a.workerId.localeCompare(b.workerId))

export const sortAccountsByReadinessThenRef = (
  accounts: ReadonlyArray<FleetAccountEntity>
): ReadonlyArray<FleetAccountEntity> => {
  const readinessRank: Record<FleetAccountEntity["readiness"], number> = {
    cooldown: 1,
    ready: 0,
    unavailable: 2,
    unknown: 3
  }
  return [...accounts].sort((a, b) => {
    const rank = readinessRank[a.readiness] - readinessRank[b.readiness]
    return rank !== 0 ? rank : a.accountRefHash.localeCompare(b.accountRefHash)
  })
}

/** Shortens a public hashed account ref (`account.pylon.codex.<24-hex>`) to
 * `pylon.codex.<8-hex>…` for display — still opaque, just less unwieldy. */
export const formatAccountRefHash = (accountRefHash: string): string => {
  const parts = accountRefHash.split(".")
  if (parts.length < 3) return accountRefHash
  const [, ...rest] = parts
  const digest = rest[rest.length - 1] ?? ""
  const lane = rest.slice(0, -1).join(".")
  return digest.length > 8 ? `${lane}.${digest.slice(0, 8)}…` : accountRefHash
}

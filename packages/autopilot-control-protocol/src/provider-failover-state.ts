export type ProviderFailoverAccount = {
  provider: string
  ready: boolean
  limited?: boolean
}

export type ProviderFailoverState = {
  active: string | null
  standby: string[]
  failedOver: boolean
  reason: string
}

export function projectFailover(
  accounts: ProviderFailoverAccount[],
): ProviderFailoverState {
  const rows = Array.isArray(accounts) ? accounts : []
  const available = rows.flatMap((account) => {
    const provider = readAvailableProvider(account)
    return provider === null ? [] : [provider]
  })

  const active = available[0] ?? null
  const primary = readProvider(rows[0])
  const failedOver = rows.length > 0 && active !== primary

  return {
    active,
    standby: available.slice(1),
    failedOver,
    reason: projectReason(rows[0], active, failedOver),
  }
}

function projectReason(
  primaryAccount: ProviderFailoverAccount | undefined,
  active: string | null,
  failedOver: boolean,
): string {
  if (primaryAccount === undefined) return "no_accounts"
  if (active === null) return "no_ready_unlimited_provider"
  if (!failedOver) return "primary_active"
  if (!isRecord(primaryAccount)) return "primary_invalid"
  if (primaryAccount.ready !== true) return "primary_not_ready"
  if (primaryAccount.limited === true) return "primary_limited"
  return "primary_unavailable"
}

function readAvailableProvider(account: unknown): string | null {
  if (!isRecord(account)) return null
  if (account.ready !== true) return null
  if (account.limited === true) return null
  return readProvider(account)
}

function readProvider(account: unknown): string | null {
  if (!isRecord(account)) return null
  const value = account.provider
  if (typeof value !== "string") return null

  const provider = value.trim()
  return provider === "" ? null : provider
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

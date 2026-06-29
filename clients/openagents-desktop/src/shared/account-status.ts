export const OPENAGENTS_DESKTOP_ACCOUNT_POLL_INTERVAL_MS = 5_000

export type FleetAccountProvider = "claude" | "codex"

export type FleetAccount = {
  readonly accountRef: string | null
  readonly label: string
  readonly provider: FleetAccountProvider
  readonly state: string
  readonly enabled: boolean
  readonly blockerRefs: readonly string[]
  readonly ready: boolean
  readonly credentialsMissing: boolean
}

export type AccountStatusResult =
  | {
      readonly ok: true
      readonly observedAt: string
      readonly accounts: readonly FleetAccount[]
      readonly readyCount: number
      readonly needsReconnectCount: number
    }
  | {
      readonly ok: false
      readonly observedAt: string
      readonly accounts: readonly FleetAccount[]
      readonly readyCount: 0
      readonly needsReconnectCount: 0
      readonly error: string
    }

const asRecord = (value: unknown): Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {}

const asArray = (value: unknown): readonly unknown[] =>
  Array.isArray(value) ? value : []

const stringValue = (value: unknown): string | null =>
  typeof value === "string" && value.trim() !== "" ? value.trim() : null

const stringArray = (value: unknown): readonly string[] =>
  asArray(value)
    .map(item => stringValue(item))
    .filter((item): item is string => item !== null)

const providerFrom = (
  rawProvider: string | null,
  schema: string | null,
  capabilityRefs: readonly string[],
): FleetAccountProvider => {
  const haystack = [
    rawProvider ?? "",
    schema ?? "",
    capabilityRefs.join(" "),
  ]
    .join(" ")
    .toLowerCase()
  return haystack.includes("claude") ? "claude" : "codex"
}

const labelFrom = (
  accountRef: string | null,
  credentialSourceRef: string | null,
): string => accountRef ?? credentialSourceRef ?? "unnamed"

export const fleetAccountFromEntry = (value: unknown): FleetAccount => {
  const entry = asRecord(value)
  const readiness = asRecord(entry.readiness)
  const accountRef = stringValue(entry.accountRef)
  const schema = stringValue(readiness.schema)
  const capabilityRefs = stringArray(readiness.capabilityRefs)
  const credentialSourceRef = stringValue(readiness.credentialSourceRef)
  const state = stringValue(readiness.state) ?? "unknown"
  const blockerRefs =
    stringArray(readiness.blockerRefs).length > 0
      ? stringArray(readiness.blockerRefs)
      : stringArray(entry.blockerRefs)
  const enabled = readiness.enabled === true

  return {
    accountRef,
    label: labelFrom(accountRef, credentialSourceRef),
    provider: providerFrom(stringValue(entry.provider), schema, capabilityRefs),
    state,
    enabled,
    blockerRefs,
    ready: state === "ready",
    credentialsMissing: state === "credentials_missing",
  }
}

export const parseFleetAccounts = (value: unknown): readonly FleetAccount[] =>
  asArray(asRecord(value).accounts).map(fleetAccountFromEntry)

export const accountStatusFromPayload = (
  value: unknown,
  observedAt: string,
): AccountStatusResult => {
  const accounts = parseFleetAccounts(value)
  return {
    ok: true,
    observedAt,
    accounts,
    readyCount: accounts.filter(account => account.ready).length,
    needsReconnectCount: accounts.filter(account => !account.ready).length,
  }
}

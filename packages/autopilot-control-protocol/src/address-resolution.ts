export type AddressPreference = "tailnet" | "lan" | "loopback"

export interface PairingAddresses {
  loopback?: string
  lan?: string
  tailnet?: string
}

export const defaultAddressPreferenceOrder: readonly AddressPreference[] = [
  "tailnet",
  "lan",
  "loopback",
]

export function resolveBaseUrls(
  addresses: PairingAddresses,
  prefs: readonly AddressPreference[] = defaultAddressPreferenceOrder,
): string[] {
  const seen = new Set<string>()
  const resolved: string[] = []

  for (const preference of prefs) {
    const baseUrl = addresses[preference]
    if (baseUrl === undefined || seen.has(baseUrl)) {
      continue
    }

    seen.add(baseUrl)
    resolved.push(baseUrl)
  }

  return resolved
}

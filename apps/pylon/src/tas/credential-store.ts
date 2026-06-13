export type CredentialRef = `ref://${string}` | `sha256:${string}`

export type ProviderCredentialLeaseState = "active" | "expired" | "revoked"

export type ProviderCredentialLease = {
  readonly providerAccountRef: string
  readonly credentialRef: CredentialRef
  readonly issuedAt: number
  readonly expiresAt: number
  readonly state: ProviderCredentialLeaseState
}

export function leaseState(
  lease: ProviderCredentialLease,
  nowMs: number,
): ProviderCredentialLeaseState {
  if (lease.state === "revoked") {
    return "revoked"
  }

  return nowMs >= lease.expiresAt ? "expired" : "active"
}

export function revoke(
  lease: ProviderCredentialLease,
): ProviderCredentialLease {
  return {
    ...lease,
    state: "revoked",
  }
}

export function invalidateStale(
  leases: readonly ProviderCredentialLease[],
  nowMs: number,
): readonly ProviderCredentialLease[] {
  return leases.map((lease) => {
    if (lease.state === "revoked" || nowMs < lease.expiresAt) {
      return lease
    }

    return {
      ...lease,
      state: "expired",
    }
  })
}

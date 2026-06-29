import type {
  Capability,
  PairingCredentialClaims,
} from "./bridge.js"
import type { ProjectionLevel } from "./control.js"

export function isCredentialUsable(claims: PairingCredentialClaims, nowMs: number): boolean {
  const expiresAtMs = Date.parse(claims.expiresAt)
  return Number.isFinite(expiresAtMs) && nowMs < expiresAtMs
}

export function hasCapability(claims: PairingCredentialClaims, cap: Capability): boolean {
  return claims.capabilities.includes(cap)
}

export function projectionLevelOf(claims: PairingCredentialClaims): ProjectionLevel {
  return claims.projectionLevel
}

export interface CredentialStore {
  set(claims: PairingCredentialClaims): void
  get(): PairingCredentialClaims | undefined
  clear(): void
}

export function createCredentialStore(): CredentialStore {
  let current: PairingCredentialClaims | undefined

  return {
    set(claims) {
      current = claims
    },
    get() {
      return current
    },
    clear() {
      current = undefined
    },
  }
}

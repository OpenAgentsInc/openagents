import { createHash } from "node:crypto"
import {
  type Capability,
  type PairingCredentialClaims,
} from "@openagentsinc/autopilot-control-protocol"

export type BootstrapSecret = {
  bootstrapId: string
  secret: string
}

export type StoredBootstrapSecret = {
  bootstrapId: string
  secretHash: string
  used: boolean
}

export type PairingRecord = {
  pairingRef: string
  jti: string
  revoked: boolean
}

export type ExchangeBootstrapInput = {
  bootstrapId: string
  secret: string
  now: Date
  ttlSeconds: number
  clientId: string
  deviceClass: string
  capabilities: Capability[]
  projectionLevel: "public_safe" | "team" | "private"
  issuer: string
  audience: string
  jti: string
  stored: StoredBootstrapSecret
}

export type ExchangeBootstrapResult =
  | { ok: true; claims: PairingCredentialClaims }
  | { ok: false; reason: "unknown_bootstrap" | "bad_secret" | "already_used" }

export function mintBootstrapSecret(rand: () => string): BootstrapSecret {
  return {
    bootstrapId: rand(),
    secret: rand(),
  }
}

export function hashSecret(secret: string): string {
  return createHash("sha256").update(secret).digest("hex")
}

export function exchangeBootstrap(input: ExchangeBootstrapInput): ExchangeBootstrapResult {
  if (input.stored.bootstrapId !== input.bootstrapId) {
    return { ok: false, reason: "unknown_bootstrap" }
  }

  if (input.stored.used) {
    return { ok: false, reason: "already_used" }
  }

  if (input.stored.secretHash !== hashSecret(input.secret)) {
    return { ok: false, reason: "bad_secret" }
  }

  const expiresAt = new Date(input.now.getTime() + input.ttlSeconds * 1000).toISOString()

  return {
    ok: true,
    claims: {
      pairingRef: input.bootstrapId,
      clientId: input.clientId,
      deviceClass: input.deviceClass,
      issuer: input.issuer,
      audience: input.audience,
      expiresAt,
      jti: input.jti,
      projectionLevel: input.projectionLevel,
      capabilities: input.capabilities,
    },
  }
}

export function isCredentialValid(claims: PairingCredentialClaims, now: Date): boolean {
  return Date.parse(claims.expiresAt) > now.getTime()
}

export function isPairingActive(
  record: PairingRecord,
  claims: PairingCredentialClaims,
  now: Date,
): boolean {
  return (
    !record.revoked &&
    record.pairingRef === claims.pairingRef &&
    record.jti === claims.jti &&
    isCredentialValid(claims, now)
  )
}

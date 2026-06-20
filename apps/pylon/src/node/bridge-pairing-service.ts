// CL-14 bridge transport — node pairing service. Composes the pure pairing
// cores (bridge-pairing.ts) into an in-memory service: issue a single-use
// bootstrap secret (embedded in a QR/bootstrap payload), exchange it for a
// scoped, expiring pairing credential, and validate/revoke credentials. This is
// the secure successor to the dev bearer token — capability-scoped, single-use
// bootstrap, revocable. Pure + framework-free; not yet wired into the HTTP
// surface (additive HTTP endpoints come next, leaving /command untouched).

import { type Capability, type PairingCredentialClaims } from "@openagentsinc/autopilot-control-protocol"
import {
  exchangeBootstrap,
  hashSecret,
  isPairingActive,
  mintBootstrapSecret,
  type PairingRecord,
  type StoredBootstrapSecret,
} from "./bridge-pairing.js"

export type IssueBootstrapResult = { bootstrapId: string; secret: string }

// Operator-facing summary of a paired client (refs-only, no secrets). Powers
// `bridge.clients.list` so the operator can see and revoke paired devices.
export type BridgeClientSummary = {
  pairingRef: string
  clientId: string
  deviceClass: string
  projectionLevel: "public_safe" | "team" | "private"
  capabilities: ReadonlyArray<Capability>
  expiresAt: string
  revoked: boolean
}

export type BridgeExchangeInput = {
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
}

export type BridgeExchangeResult =
  | { ok: true; claims: PairingCredentialClaims }
  | { ok: false; reason: "unknown_bootstrap" | "bad_secret" | "already_used" }

export type BridgePairingService = {
  issueBootstrap: () => IssueBootstrapResult
  exchange: (input: BridgeExchangeInput) => BridgeExchangeResult
  validate: (claims: PairingCredentialClaims, now: Date) => boolean
  // Authoritative authorization for a bridge request: looks up the STORED claims
  // by pairingRef (never trusts client-sent capabilities — a paired client must
  // not be able to escalate its own scope), verifies the presented jti matches
  // and the pairing is active+unexpired, and returns the stored claims or null.
  authorize: (pairingRef: string, jti: string, now: Date) => PairingCredentialClaims | null
  revoke: (pairingRef: string) => boolean
  // Operator-facing roster of paired clients (refs-only). `bridge.clients.list`.
  listClients: () => BridgeClientSummary[]
}

export function createBridgePairingService(options: { rand?: () => string } = {}): BridgePairingService {
  const rand = options.rand ?? (() => crypto.randomUUID())
  const bootstraps = new Map<string, StoredBootstrapSecret>()
  const pairings = new Map<string, PairingRecord>()
  // Authoritative issued claims, keyed by pairingRef.
  const issuedClaims = new Map<string, PairingCredentialClaims>()

  return {
    issueBootstrap() {
      const minted = mintBootstrapSecret(rand)
      bootstraps.set(minted.bootstrapId, {
        bootstrapId: minted.bootstrapId,
        secretHash: hashSecret(minted.secret),
        used: false,
      })
      return { bootstrapId: minted.bootstrapId, secret: minted.secret }
    },

    exchange(input) {
      const stored = bootstraps.get(input.bootstrapId)
      if (stored === undefined) return { ok: false, reason: "unknown_bootstrap" }
      const result = exchangeBootstrap({ ...input, stored })
      if (!result.ok) return result
      // Single-use: burn the bootstrap and record the active pairing.
      bootstraps.set(input.bootstrapId, { ...stored, used: true })
      pairings.set(result.claims.pairingRef, {
        pairingRef: result.claims.pairingRef,
        jti: result.claims.jti,
        revoked: false,
      })
      issuedClaims.set(result.claims.pairingRef, result.claims)
      return result
    },

    validate(claims, now) {
      const record = pairings.get(claims.pairingRef)
      return record !== undefined && isPairingActive(record, claims, now)
    },

    authorize(pairingRef, jti, now) {
      const record = pairings.get(pairingRef)
      const claims = issuedClaims.get(pairingRef)
      if (record === undefined || claims === undefined) return null
      // jti must match the issued credential, and the pairing must be active.
      if (record.jti !== jti) return null
      return isPairingActive(record, claims, now) ? claims : null
    },

    revoke(pairingRef) {
      const record = pairings.get(pairingRef)
      if (record === undefined) return false
      pairings.set(pairingRef, { ...record, revoked: true })
      return true
    },

    listClients() {
      const summaries: BridgeClientSummary[] = []
      for (const [pairingRef, claims] of issuedClaims) {
        const record = pairings.get(pairingRef)
        summaries.push({
          pairingRef,
          clientId: claims.clientId,
          deviceClass: claims.deviceClass,
          projectionLevel: claims.projectionLevel,
          capabilities: claims.capabilities,
          expiresAt: claims.expiresAt,
          revoked: record?.revoked ?? false,
        })
      }
      return summaries
    },
  }
}

/**
 * Pilot-seam adapter (ENV-2, openagents #8780): plugs the RFC 9449 DPoP
 * verifier into the portable-session capability broker's opt-in
 * `proofVerifier` seam.
 *
 * A lease issued with a `clientKeyThumbprint` can only be redeemed by the
 * client that holds the matching private key; the broker fails closed on a
 * missing/invalid proof, and this adapter's replay cache makes every proof
 * single-use. Leases issued without a thumbprint keep the existing path
 * unchanged — the handshake is strictly opt-in.
 */
import type { CapabilityProofVerifier } from "@openagentsinc/portable-session-contract"

import {
  makeInMemoryDpopReplayCache,
  verifyAndConsumeDpopProof,
  type DpopReplayCache,
} from "./dpop.js"

export type DpopCapabilityProofVerifierOptions = {
  readonly replayCache?: DpopReplayCache
  readonly now?: () => Date
  readonly maxAgeSeconds?: number
  readonly maxClockSkewSeconds?: number
}

export function makeDpopCapabilityProofVerifier(
  options: DpopCapabilityProofVerifierOptions = {},
): CapabilityProofVerifier {
  const replayCache = options.replayCache ?? makeInMemoryDpopReplayCache()
  return {
    verify: async (input) => {
      const now = options.now?.() ?? new Date()
      const result = await verifyAndConsumeDpopProof({
        proof: input.proof,
        htm: input.htm,
        htu: input.htu,
        nowEpochSeconds: Math.floor(now.getTime() / 1000),
        expectedThumbprint: input.expectedThumbprint,
        replayCache,
        ...(options.maxAgeSeconds === undefined
          ? {}
          : { maxAgeSeconds: options.maxAgeSeconds }),
        ...(options.maxClockSkewSeconds === undefined
          ? {}
          : { maxClockSkewSeconds: options.maxClockSkewSeconds }),
      })
      return result.ok
        ? { ok: true, thumbprint: result.thumbprint }
        : { ok: false, reason: result.reason }
    },
  }
}

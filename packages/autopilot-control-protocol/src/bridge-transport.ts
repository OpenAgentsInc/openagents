// CL-14 client bridge transport. The secure successor to the dev-token control
// client: a client exchanges a single-use bootstrap for a scoped pairing
// credential (POST /bridge/pair), then makes capability-scoped read requests
// (POST /bridge with `Authorization: Bridge <pairingRef>:<jti>`). Pure +
// transport-agnostic (inject fetch) so web / desktop / mobile share it.

import { buildListRequest } from "./bridge-client"
import type { Capability, PairingCredentialClaims, ProjectionLevel } from "./bridge"
import { decodeSessionSummary, type SessionSummary } from "./control"

export type BridgePairInput = {
  baseUrl: string
  bootstrapId: string
  secret: string
  clientId: string
  deviceClass: string
  capabilities: Capability[]
  projectionLevel: ProjectionLevel
  ttlSeconds?: number
  fetchImpl?: typeof fetch
}

export type BridgePairResult =
  | { ok: true; claims: PairingCredentialClaims }
  | { ok: false; reason: string }

// Exchange a bootstrap secret for a scoped pairing credential.
export async function pairBridge(input: BridgePairInput): Promise<BridgePairResult> {
  const doFetch = input.fetchImpl ?? fetch
  const base = input.baseUrl.replace(/\/+$/, "")
  const res = await doFetch(`${base}/bridge/pair`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      bootstrapId: input.bootstrapId,
      secret: input.secret,
      clientId: input.clientId,
      deviceClass: input.deviceClass,
      capabilities: input.capabilities,
      projectionLevel: input.projectionLevel,
      ...(input.ttlSeconds === undefined ? {} : { ttlSeconds: input.ttlSeconds }),
    }),
  })
  const json = (await res.json()) as { ok?: unknown; claims?: unknown; reason?: unknown }
  if (res.ok && json.ok === true && json.claims) {
    return { ok: true, claims: json.claims as PairingCredentialClaims }
  }
  return { ok: false, reason: typeof json.reason === "string" ? json.reason : `pair failed (${res.status})` }
}

export type BridgeCredential = { pairingRef: string; jti: string; capabilityRef?: string }

export type BridgeTransport = {
  list: () => Promise<SessionSummary[]>
}

// A transport bound to a pairing credential. Sends capability-scoped read
// requests to /bridge; the node authorizes via its stored claims.
export function createBridgeTransport(input: {
  baseUrl: string
  credential: BridgeCredential
  fetchImpl?: typeof fetch
  idgen?: () => string
}): BridgeTransport {
  const doFetch = input.fetchImpl ?? fetch
  const base = input.baseUrl.replace(/\/+$/, "")
  let counter = 0
  const nextId = input.idgen ?? (() => `req.${++counter}`)
  const authHeader = `Bridge ${input.credential.pairingRef}:${input.credential.jti}`

  const send = async (envelope: unknown): Promise<unknown> => {
    const res = await doFetch(`${base}/bridge`, {
      method: "POST",
      headers: { authorization: authHeader, "content-type": "application/json" },
      body: JSON.stringify(envelope),
    })
    const json = (await res.json()) as { ok?: unknown; result?: unknown; error?: unknown }
    if (!res.ok || json.ok !== true) {
      throw new Error(typeof json.error === "string" ? json.error : `bridge request failed (${res.status})`)
    }
    return json.result
  }

  return {
    async list() {
      const requestId = nextId()
      const envelope = buildListRequest({
        pairingRef: input.credential.pairingRef,
        capabilityRef: input.credential.capabilityRef ?? "observe_public",
        clientRequestId: requestId,
        idempotencyKey: requestId,
      })
      return (await send(envelope) as unknown[]).map((row) => decodeSessionSummary(row))
    },
  }
}

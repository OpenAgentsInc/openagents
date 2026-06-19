// CL-14 client bridge transport. The secure successor to the dev-token control
// client: a client exchanges a single-use bootstrap for a scoped pairing
// credential (POST /bridge/pair), then makes capability-scoped read requests
// (POST /bridge with `Authorization: Bridge <pairingRef>:<jti>`). Pure +
// transport-agnostic (inject fetch) so web / desktop / mobile share it.

import { buildArtifactReadRequest, buildCancelRequest, buildHistoryRequest, buildListRequest } from "./bridge-client"
import { buildDecisionResolveEnvelope } from "./bridge-decision-client"
import { parseArtifactReadResponse, type ArtifactReadResponse } from "./artifact-content-view"
import type { Capability, PairingCredentialClaims, ProjectionLevel } from "./bridge"
import type { DecisionVerb } from "./decision"
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
  // Cursor-resumable catch-up for one session over the bridge (#5000
  // session.history). Returns the node's session-events projection (e.g.
  // { recentEvents }); the caller dedups/resumes via the shared cursor model.
  history: (sessionRef: string) => Promise<unknown>
  // #5002 write actions (capability-gated by the node's stored claims):
  // decision.resolve (answer_decision) and session.cancel (cancel). Throw on a
  // non-ok response; callers classify the error via classifyActionOutcome.
  resolveDecision: (input: { requestId: string; verb: DecisionVerb; answer?: string }) => Promise<unknown>
  cancel: (sessionRef: string) => Promise<unknown>
  // G3 (#5495) read action: the retained proof/failure artifact a completed
  // session produced (read_artifact capability). Returns the typed
  // { sessionRef, kind, artifact } envelope; run projectArtifactContentView over
  // it to render the diff/transcript/text view. Throws on a non-ok response.
  readArtifact: (sessionRef: string) => Promise<ArtifactReadResponse>
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
    async history(sessionRef) {
      const requestId = nextId()
      const envelope = buildHistoryRequest({
        sessionRef,
        pairingRef: input.credential.pairingRef,
        capabilityRef: input.credential.capabilityRef ?? "observe_public",
        clientRequestId: requestId,
        idempotencyKey: requestId,
      })
      return send(envelope)
    },
    async resolveDecision({ requestId, verb, answer }) {
      const clientRequestId = nextId()
      const envelope = buildDecisionResolveEnvelope({
        requestId,
        verb,
        pairingRef: input.credential.pairingRef,
        capabilityRef: "answer_decision",
        clientRequestId,
        ...(answer === undefined ? {} : { answer }),
      })
      return send(envelope)
    },
    async cancel(sessionRef) {
      const clientRequestId = nextId()
      const envelope = buildCancelRequest({
        sessionRef,
        pairingRef: input.credential.pairingRef,
        capabilityRef: "cancel",
        clientRequestId,
        idempotencyKey: clientRequestId,
      })
      return send(envelope)
    },
    async readArtifact(sessionRef) {
      const requestId = nextId()
      const envelope = buildArtifactReadRequest({
        sessionRef,
        pairingRef: input.credential.pairingRef,
        capabilityRef: "read_artifact",
        clientRequestId: requestId,
        idempotencyKey: requestId,
      })
      return parseArtifactReadResponse(await send(envelope))
    },
  }
}

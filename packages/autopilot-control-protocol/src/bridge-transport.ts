// CL-14 client bridge transport. The secure successor to the dev-token control
// client: a client exchanges a single-use bootstrap for a scoped pairing
// credential (POST /bridge/pair), then makes capability-scoped read requests
// (POST /bridge with `Authorization: Bridge <pairingRef>:<jti>`). Pure +
// transport-agnostic (inject fetch) so web / desktop / mobile share it.

import { buildArtifactReadRequest, buildCancelRequest, buildHistoryRequest, buildListRequest } from "./bridge-client.js"
import { buildDecisionResolveEnvelope } from "./bridge-decision-client.js"
import { buildSubscribeEnvelope, parseBridgeEventBatch, type BridgeEventBatch } from "./bridge-subscribe-client.js"
import { parseArtifactReadResponse, type ArtifactReadResponse } from "./artifact-content-view.js"
import {
  buildCoordinatorPauseEnvelope,
  buildCoordinatorResumeEnvelope,
  buildDeployCloudEnvelope,
  buildIntentSubmitEnvelope,
  buildSpawnEnvelope,
  buildTurnSteerEnvelope,
  type SpawnLane,
} from "./bridge-steer-client.js"
import type { Capability, PairingCredentialClaims, ProjectionLevel } from "./bridge.js"
import type { DecisionVerb } from "./decision.js"
import { decodeSessionSummary, type SessionSummary } from "./control.js"

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
  // #5000 cursor-resumable LIVE event poll over the bridge (session.subscribe,
  // observe-class capability). The remote-bridge transport leg that lets an
  // Expo/web/desktop client render a live session timeline and feed
  // RemoteDecisionQueue.ingestMany WITHOUT consuming the SSE stream (which RN
  // fetch can't do cleanly): returns the node's bounded event tail parsed and
  // filtered to rows newer than `cursor`, with the resume cursor for the next
  // poll. Throws on a non-ok response (e.g. the node's 403 when the credential
  // lacks an observe capability).
  subscribe: (input: { sessionRef: string; cursor?: number }) => Promise<BridgeEventBatch>
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
  // #5494 (epic #5492 G1): the remaining four steer-actions over the bridge,
  // each gated on a distinct capability class held in the node's stored claims:
  //   spawn          → spawn_session
  //   submitIntent   → send_instruction
  //   pause/resume   → pause_resume
  //   deploy         → deploy_cloud
  // Each sends its own capabilityRef so a credential scoped to several steer
  // classes selects the right one per verb. Throw on a non-ok response.
  spawn: (input: { adapter: "codex" | "claude_agent"; objective: string; verify?: string[]; lane?: SpawnLane }) => Promise<unknown>
  submitIntent: (input: { title: string; body: string; scopeHint?: string; submittedByClientRef?: string }) => Promise<unknown>
  steerTurn: (input: { sessionRef: string; instruction: string; timeoutSeconds?: number }) => Promise<unknown>
  pauseCoordinator: () => Promise<unknown>
  resumeCoordinator: () => Promise<unknown>
  deployCloud: (input: { target: string; ref: string; env?: string }) => Promise<unknown>
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
    async subscribe({ sessionRef, cursor }) {
      const requestId = nextId()
      const envelope = buildSubscribeEnvelope({
        sessionRef,
        pairingRef: input.credential.pairingRef,
        capabilityRef: input.credential.capabilityRef ?? "observe_public",
        clientRequestId: requestId,
        ...(cursor === undefined ? {} : { cursor }),
      })
      return parseBridgeEventBatch(await send(envelope), cursor ?? -1)
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
    async spawn(spawnInput) {
      const clientRequestId = nextId()
      return send(
        buildSpawnEnvelope({
          ...spawnInput,
          pairingRef: input.credential.pairingRef,
          capabilityRef: "spawn_session",
          clientRequestId,
        }),
      )
    },
    async submitIntent(intentInput) {
      const clientRequestId = nextId()
      return send(
        buildIntentSubmitEnvelope({
          ...intentInput,
          pairingRef: input.credential.pairingRef,
          capabilityRef: "send_instruction",
          clientRequestId,
        }),
      )
    },
    async steerTurn(turnInput) {
      const clientRequestId = nextId()
      return send(
        buildTurnSteerEnvelope({
          ...turnInput,
          pairingRef: input.credential.pairingRef,
          capabilityRef: "send_instruction",
          clientRequestId,
        }),
      )
    },
    async pauseCoordinator() {
      const clientRequestId = nextId()
      return send(
        buildCoordinatorPauseEnvelope({
          pairingRef: input.credential.pairingRef,
          capabilityRef: "pause_resume",
          clientRequestId,
        }),
      )
    },
    async resumeCoordinator() {
      const clientRequestId = nextId()
      return send(
        buildCoordinatorResumeEnvelope({
          pairingRef: input.credential.pairingRef,
          capabilityRef: "pause_resume",
          clientRequestId,
        }),
      )
    },
    async deployCloud(deployInput) {
      const clientRequestId = nextId()
      return send(
        buildDeployCloudEnvelope({
          ...deployInput,
          pairingRef: input.credential.pairingRef,
          capabilityRef: "deploy_cloud",
          clientRequestId,
        }),
      )
    },
  }
}

// Control server for the Pylon node (issue #4740): serializes the Phase 0
// PylonEvent seam over HTTP + SSE so a TUI can attach to a running node, and
// exposes a small typed command API. Binds loopback by default; every
// request requires the node's bearer token (a per-node secret file in the
// Pylon home directory). Money commands execute node-side - the attached
// view only ever sends the command after its confirm dialog.

import { chmod, mkdir, writeFile } from "node:fs/promises"
import { randomBytes } from "node:crypto"
import { join } from "node:path"
import { Effect, PubSub, SubscriptionRef, type Scope } from "effect"
import type { PylonEvent, PylonLogEntry, TelemetryPaneState, WalletPaneState } from "./state.js"
import type { PylonNodeRuntime } from "./runtime.js"
import { createBridgePairingService } from "./bridge-pairing-service.js"
import { controlCommandValidationReason } from "./control-command-error.js"
import {
  CONTROL_HEALTH_CAPABILITIES,
  CONTROL_SCHEMA_TAG,
  verbAllowedByCapabilities,
  type BridgeRequestVerb,
  type Capability,
} from "@openagentsinc/autopilot-control-protocol"
import { PYLON_VERSION } from "../version.js"
import type {
  ControlSessionActions,
  AppleFmSessionStartCommand,
  ControlSessionArtifactCommand,
  ControlSessionCancelCommand,
  ControlSessionEventsCommand,
  ControlSessionListCommand,
  ControlSessionReplyCommand,
  ControlSessionSpawnCommand,
} from "./control-sessions.js"

export const defaultControlPort = 4716
export const controlTokenFileName = "control-token"
export const snapshotLogTail = 300

export function isLoopbackHostname(hostname: string): boolean {
  return hostname === "127.0.0.1" || hostname === "localhost" || hostname === "::1"
}

export function assertControlBindSafe(options: { hostname?: string; token?: string }): void {
  const hostname = options.hostname ?? "127.0.0.1"
  if (!isLoopbackHostname(hostname) && !options.token?.trim()) {
    throw new Error(`refusing to expose Pylon control server on ${hostname} without a bearer token`)
  }
}

// Serializable snapshot sent to every new attach connection before the live
// event tail begins.
export type PylonSnapshot = {
  type: "snapshot"
  wallet: WalletPaneState
  telemetry: TelemetryPaneState
  operatorText: string
  logFeed: PylonLogEntry[]
}

export type ControlCommand =
  | { type: "wallet.send"; destinationRef: string; amountSats?: number }
  | { type: "wallet.receive"; amountSats: number }
  | { type: "wallet.admit-payout-target"; kind: string; ref: string }
  // CL-23 read-only balance/earnings: clients fetch the live MDK wallet status
  // (balance + readiness). No spend authority — strictly a projection.
  | { type: "wallet.status" }
  // #5207 warm-session routing: the LOCAL one-shot CLI (`pylon wallet send
  // --rail spark`) routes its Spark send through the running daemon's WARM
  // session so it skips the ~4s cold build + sync. Loopback + token gated, same
  // trust boundary as the other node-side money commands. `confirmSend` carries
  // the explicit `--confirm-send` consent; the raw destination rides the
  // loopback API (local only), exactly as the existing money commands do.
  | { type: "wallet.spark_send"; destination: string; amountSats?: number; confirmSend?: boolean }
  // #5207: route `pylon wallet backup-status` through the warm session too.
  | { type: "wallet.spark_backup_status"; showLocalTarget?: boolean }
  | { type: "apple_fm.status" }
  | AppleFmSessionStartCommand
  | { type: "assignments.poll" }
  | { type: "assignments.accept"; leaseRef: string }
  | ControlSessionSpawnCommand
  | ControlSessionListCommand
  | ControlSessionEventsCommand
  | ControlSessionCancelCommand
  | ControlSessionArtifactCommand
  | ControlSessionReplyCommand
  | { type: "intent.submit"; title: string; body: string; scopeHint?: string; submittedByClientRef?: string }
  | { type: "intent.list"; sinceCursor?: string }
  | { type: "accounts.list" }
  | { type: "accounts.status" }
  // CL-16 approvals: read-only pending list + exactly-once resolve.
  | { type: "approvals.list" }
  | { type: "approvals.resolve"; approvalRef: string; decision: "approve" | "deny" | "answer"; answer?: string }
  // CL-17 (rescoped): pause/resume the autonomous coordinator work loop.
  | { type: "coordinator.pause" }
  | { type: "coordinator.resume" }
  | { type: "coordinator.status" }
  // CL-26 "Deploy to Cloud": a node-triggered deploy through OUR cloud pipeline.
  // Execution is gated behind OA_DEPLOY_ENABLE=1 (fail-safe). deploy.status is
  // a read-only projection of the node's last deploy.
  | { type: "deploy.cloud"; target: string; ref: string; env?: string }
  | { type: "deploy.status" }
  // CL-14 bridge transport: operator (dev-token authed) mints a single-use
  // bootstrap that a client exchanges at /bridge/pair for a scoped credential.
  | { type: "bridge.issueBootstrap" }
  // #5000 bridge admin (dev-token authed): operator lists paired clients and
  // revokes a paired credential by its pairingRef.
  | { type: "bridge.clients.list" }
  | { type: "bridge.revoke"; pairingRef: string }

export interface ControlCommandActions {
  walletSend: (destinationRef: string, amountSats?: number) => Promise<unknown>
  walletReceive: (amountSats: number) => Promise<unknown>
  walletAdmitPayoutTarget: (kind: string, ref: string) => Promise<unknown>
  // CL-23: read-only live wallet status (balance + readiness). Optional so
  // nodes without a wallet runner simply report it as unavailable.
  walletStatus?: () => Promise<unknown>
  // #5207: execute a Spark send on the node's WARM session and append the
  // ledger event node-side (so the CLI does not double-log). Returns the same
  // projection the local cold path returns, so the CLI prints it byte-identical.
  walletSparkSend?: (input: { destination: string; amountSats?: number; confirmSend?: boolean }) => Promise<unknown>
  // #5207: read backup-status (balance + sweep recommendation) off the warm
  // session, returning the same body shape the local cold path prints.
  walletSparkBackupStatus?: (input: { showLocalTarget?: boolean }) => Promise<unknown>
  appleFmStatus?: () => Promise<unknown>
  assignmentsPoll?: () => Promise<unknown>
  assignmentsAccept?: (leaseRef: string) => Promise<unknown>
  sessions?: ControlSessionActions
  // CL-34: the phone composes an "ask" and submits it to the node, which
  // enqueues it as a work intent for the coordinator to plan + fan out.
  intents?: {
    submit: (input: { title: string; body: string; scopeHint?: string; submittedByClientRef?: string }) => Promise<unknown>
    list: (sinceCursor?: string) => Promise<unknown>
  }
  // CL-18: read-only accounts + readiness panel (public-projection-safe).
  accountsList?: () => Promise<unknown>
  accountsStatus?: () => Promise<unknown>
  // CL-16: read-only pending approvals + exactly-once resolve (approve/deny/answer).
  approvals?: {
    list: () => Promise<unknown>
    resolve: (input: { approvalRef: string; decision: "approve" | "deny" | "answer"; answer?: string }) => Promise<unknown>
  }
  // CL-26: node-triggered "Deploy to Cloud". `deployCloud` validates + (only
  // when OA_DEPLOY_ENABLE=1) fire-and-forgets the deploy; `deployStatus`
  // projects the node's last deploy. Optional so nodes without it report it
  // as unavailable.
  deploy?: {
    deployCloud: (input: { target: unknown; ref: unknown; env?: unknown }) => Promise<unknown>
    deployStatus: () => Promise<unknown>
  }
  // CL-17 (rescoped): pause/resume autonomous coordinator work.
  coordinator?: {
    pause: () => { paused: boolean }
    resume: () => { paused: boolean }
    status: () => { paused: boolean }
  }
}

export async function ensureControlToken(homeDir: string): Promise<string> {
  const path = join(homeDir, controlTokenFileName)
  const file = Bun.file(path)
  if (await file.exists()) {
    const existing = (await file.text()).trim()
    if (existing.length >= 16) return existing
  }
  const token = randomBytes(24).toString("hex")
  await mkdir(homeDir, { recursive: true })
  await writeFile(path, `${token}\n`, { mode: 0o600 })
  await chmod(path, 0o600)
  return token
}

export function controlTokenPath(homeDir: string): string {
  return join(homeDir, controlTokenFileName)
}

export const captureNodeSnapshot = (runtime: PylonNodeRuntime): Effect.Effect<PylonSnapshot> =>
  Effect.gen(function* () {
    const wallet = yield* SubscriptionRef.get(runtime.wallet)
    const telemetry = yield* SubscriptionRef.get(runtime.telemetry)
    const operator = yield* SubscriptionRef.get(runtime.operator)
    const feed = yield* SubscriptionRef.get(runtime.logFeed)
    return {
      type: "snapshot",
      wallet,
      telemetry,
      operatorText: operator.text,
      logFeed: feed.slice(-snapshotLogTail),
    }
  })

export interface ControlServerOptions {
  token: string
  actions: ControlCommandActions
  hostname?: string
  port?: number
}

export interface ControlServerHandle {
  port: number
  hostname: string
  url: string
  clientCount: () => number
}

function sseFrame(payload: unknown): string {
  return `data: ${JSON.stringify(payload)}\n\n`
}

function unauthorized(): Response {
  return Response.json({ error: "unauthorized" }, { status: 401 })
}

// Starts the control server inside the caller's Scope: the SSE fan-out fiber
// and the Bun server are torn down when the Scope closes.
export const startControlServer = (
  runtime: PylonNodeRuntime,
  options: ControlServerOptions,
): Effect.Effect<ControlServerHandle, Error, Scope.Scope> =>
  Effect.gen(function* () {
    const clients = new Set<ReadableStreamDefaultController<Uint8Array>>()
    const encoder = new TextEncoder()
    // CL-14 bridge transport pairing service (in-memory; additive to the
    // dev-token transport, which is unchanged).
    const bridgePairing = createBridgePairingService()

    const broadcast = (event: PylonEvent) => {
      const frame = encoder.encode(sseFrame(event))
      for (const controller of clients) {
        try {
          controller.enqueue(frame)
        } catch {
          clients.delete(controller)
        }
      }
    }

    // One pump fiber for all clients.
    const subscription = yield* PubSub.subscribe(runtime.events)
    yield* Effect.forkScoped(
      Effect.gen(function* () {
        while (true) {
          const event = yield* PubSub.take(subscription)
          broadcast(event)
        }
      }),
    )

    const authorized = (request: Request): boolean => {
      const header = request.headers.get("authorization") ?? ""
      return header === `Bearer ${options.token}`
    }

    const runCommand = async (command: ControlCommand): Promise<unknown> => {
      switch (command.type) {
        case "wallet.send":
          return options.actions.walletSend(command.destinationRef, command.amountSats)
        case "wallet.receive":
          return options.actions.walletReceive(command.amountSats)
        case "wallet.admit-payout-target":
          return options.actions.walletAdmitPayoutTarget(command.kind, command.ref)
        case "wallet.status":
          if (!options.actions.walletStatus) throw new Error("wallet status unavailable on this node")
          return options.actions.walletStatus()
        case "wallet.spark_send":
          if (!options.actions.walletSparkSend) throw new Error("spark send unavailable on this node")
          return options.actions.walletSparkSend({
            destination: command.destination,
            ...(command.amountSats === undefined ? {} : { amountSats: command.amountSats }),
            ...(command.confirmSend === undefined ? {} : { confirmSend: command.confirmSend }),
          })
        case "wallet.spark_backup_status":
          if (!options.actions.walletSparkBackupStatus) throw new Error("spark backup-status unavailable on this node")
          return options.actions.walletSparkBackupStatus({
            ...(command.showLocalTarget === undefined ? {} : { showLocalTarget: command.showLocalTarget }),
          })
        case "apple_fm.status":
          if (!options.actions.appleFmStatus) throw new Error("Apple FM status unavailable on this node")
          return options.actions.appleFmStatus()
        case "apple_fm.session.start":
          if (!options.actions.sessions) throw new Error("sessions unavailable on this node")
          return options.actions.sessions.startAppleFm(command)
        case "approvals.list":
          if (!options.actions.approvals) throw new Error("approvals unavailable on this node")
          return options.actions.approvals.list()
        case "approvals.resolve":
          if (!options.actions.approvals) throw new Error("approvals unavailable on this node")
          return options.actions.approvals.resolve({
            approvalRef: command.approvalRef,
            decision: command.decision,
            ...(command.answer === undefined ? {} : { answer: command.answer }),
          })
        case "deploy.cloud":
          if (!options.actions.deploy) throw new Error("deploy unavailable on this node")
          return options.actions.deploy.deployCloud({
            target: command.target,
            ref: command.ref,
            ...(command.env === undefined ? {} : { env: command.env }),
          })
        case "deploy.status":
          if (!options.actions.deploy) throw new Error("deploy unavailable on this node")
          return options.actions.deploy.deployStatus()
        case "coordinator.pause":
          if (!options.actions.coordinator) throw new Error("coordinator unavailable on this node")
          return options.actions.coordinator.pause()
        case "coordinator.resume":
          if (!options.actions.coordinator) throw new Error("coordinator unavailable on this node")
          return options.actions.coordinator.resume()
        case "coordinator.status":
          if (!options.actions.coordinator) throw new Error("coordinator unavailable on this node")
          return options.actions.coordinator.status()
        case "assignments.poll":
          if (!options.actions.assignmentsPoll) throw new Error("assignments unavailable on this node")
          return options.actions.assignmentsPoll()
        case "assignments.accept":
          if (!options.actions.assignmentsAccept) throw new Error("assignments unavailable on this node")
          return options.actions.assignmentsAccept(command.leaseRef)
        case "session.spawn":
          if (!options.actions.sessions) throw new Error("sessions unavailable on this node")
          return options.actions.sessions.spawn(command)
        case "session.reply":
          if (!options.actions.sessions) throw new Error("sessions unavailable on this node")
          return options.actions.sessions.reply(command)
        case "session.list":
          if (!options.actions.sessions) throw new Error("sessions unavailable on this node")
          return options.actions.sessions.list()
        case "session.events":
          if (!options.actions.sessions) throw new Error("sessions unavailable on this node")
          return options.actions.sessions.events(command.sessionRef)
        case "session.cancel":
          if (!options.actions.sessions) throw new Error("sessions unavailable on this node")
          return options.actions.sessions.cancel(command.sessionRef)
        case "session.artifact":
          if (!options.actions.sessions) throw new Error("sessions unavailable on this node")
          return options.actions.sessions.artifact(command.sessionRef)
        case "intent.submit":
          if (!options.actions.intents) throw new Error("intents unavailable on this node")
          return options.actions.intents.submit({
            title: command.title,
            body: command.body,
            ...(command.scopeHint === undefined ? {} : { scopeHint: command.scopeHint }),
            ...(command.submittedByClientRef === undefined ? {} : { submittedByClientRef: command.submittedByClientRef }),
          })
        case "intent.list":
          if (!options.actions.intents) throw new Error("intents unavailable on this node")
          return options.actions.intents.list(command.sinceCursor)
        case "accounts.list":
          if (!options.actions.accountsList) throw new Error("accounts unavailable on this node")
          return options.actions.accountsList()
        case "accounts.status":
          if (!options.actions.accountsStatus) throw new Error("account status unavailable on this node")
          return options.actions.accountsStatus()
        case "bridge.issueBootstrap":
          // Returns { bootstrapId, secret } for the operator to hand to a client
          // (QR/connect payload). Single-use; the secret is exchanged at
          // /bridge/pair. Authed by the dev token (this runs via /command).
          return bridgePairing.issueBootstrap()
        case "bridge.clients.list":
          // Operator roster of paired clients (refs-only, no secrets).
          return { clients: bridgePairing.listClients() }
        case "bridge.revoke": {
          // Operator revokes a paired credential by pairingRef. Idempotent;
          // `revoked: false` means no such pairing was found.
          if (typeof command.pairingRef !== "string" || command.pairingRef.length === 0) {
            throw new Error("pairingRef is required")
          }
          return { revoked: bridgePairing.revoke(command.pairingRef) }
        }
        default:
          throw new Error(`unknown command: ${(command as { type?: string }).type}`)
      }
    }

    const server = yield* Effect.try({
      try: () => {
        assertControlBindSafe(options)
        return Bun.serve({
          hostname: options.hostname ?? "127.0.0.1",
          port: options.port ?? defaultControlPort,
          idleTimeout: 0,
          fetch: async (request) => {
            const url = new URL(request.url)
            if (url.pathname === "/health") {
              return Response.json({
                ok: true,
                schema: CONTROL_SCHEMA_TAG,
                version: PYLON_VERSION,
                capabilities: [...CONTROL_HEALTH_CAPABILITIES],
              })
            }

            // CL-14 bridge pairing exchange: pre-bearer (the single-use
            // bootstrap secret IS the credential). Returns scoped pairing claims.
            if (url.pathname === "/bridge/pair" && request.method === "POST") {
              try {
                const body = (await request.json()) as {
                  bootstrapId?: unknown
                  secret?: unknown
                  clientId?: unknown
                  deviceClass?: unknown
                  capabilities?: unknown
                  projectionLevel?: unknown
                  ttlSeconds?: unknown
                }
                if (typeof body.bootstrapId !== "string" || typeof body.secret !== "string") {
                  return Response.json({ error: "bootstrapId and secret are required" }, { status: 400 })
                }
                const result = bridgePairing.exchange({
                  bootstrapId: body.bootstrapId,
                  secret: body.secret,
                  now: new Date(),
                  ttlSeconds: typeof body.ttlSeconds === "number" ? body.ttlSeconds : 86_400,
                  clientId: typeof body.clientId === "string" ? body.clientId : "client",
                  deviceClass: typeof body.deviceClass === "string" ? body.deviceClass : "unknown",
                  capabilities: Array.isArray(body.capabilities)
                    ? (body.capabilities.filter((c) => typeof c === "string") as Capability[])
                    : (["observe_public"] as Capability[]),
                  projectionLevel:
                    body.projectionLevel === "team" || body.projectionLevel === "private"
                      ? body.projectionLevel
                      : "public_safe",
                  issuer: "pylon.node",
                  audience: typeof body.clientId === "string" ? body.clientId : "client",
                  jti: randomBytes(16).toString("hex"),
                })
                return result.ok
                  ? Response.json({ ok: true, claims: result.claims })
                  : Response.json({ ok: false, reason: result.reason }, { status: 401 })
              } catch {
                return Response.json({ error: "malformed pairing request" }, { status: 400 })
              }
            }

            // CL-14 bridge read endpoint: credential + capability enforced.
            // Auth: `Authorization: Bridge <pairingRef>:<jti>`. Authorization
            // uses the STORED claims (client-sent capabilities are never
            // trusted). Pre-bearer (the bridge credential, not the dev token).
            if (url.pathname === "/bridge" && request.method === "POST") {
              const header = request.headers.get("authorization") ?? ""
              const match = /^Bridge\s+([^:]+):(.+)$/.exec(header)
              if (match === null) {
                return Response.json({ error: "bridge credential required" }, { status: 401 })
              }
              const claims = bridgePairing.authorize(match[1] ?? "", match[2] ?? "", new Date())
              if (claims === null) {
                return Response.json({ error: "invalid or expired pairing" }, { status: 401 })
              }
              try {
                const envelope = (await request.json()) as {
                  verb?: unknown
                  sessionRef?: unknown
                  approvalRef?: unknown
                  decision?: unknown
                  answer?: unknown
                  // Canonical protocol decision shape (buildDecisionResolveEnvelope).
                  requestId?: unknown
                  decisionVerb?: unknown
                  instruction?: unknown
                  timeoutSeconds?: unknown
                }
                const verb = typeof envelope.verb === "string" ? envelope.verb : ""
                if (!verbAllowedByCapabilities(verb as BridgeRequestVerb, claims.capabilities)) {
                  return Response.json({ error: "capability not granted", verb }, { status: 403 })
                }

                // capability.list: echo the credential's granted scope (refs
                // only). Lets a paired client discover what it may do.
                if (verb === "capability.list") {
                  return Response.json({
                    ok: true,
                    result: {
                      pairingRef: claims.pairingRef,
                      projectionLevel: claims.projectionLevel,
                      capabilities: claims.capabilities,
                    },
                  })
                }

                // #5494 (epic #5492 G1): the four remaining steer-actions over
                // the capability-scoped bridge (cancel + decision.resolve were
                // already here). Each is gated above by verbAllowedByCapabilities
                // against the node's STORED claims; here we route to the same
                // node action the dev-token /command path uses. The mobile
                // client presents a scoped capability, not the raw node token.

                // coordinator.pause / coordinator.resume (pause_resume).
                if (verb === "coordinator.pause" || verb === "coordinator.resume") {
                  if (!options.actions.coordinator) {
                    return Response.json({ error: "coordinator unavailable on this node" }, { status: 404 })
                  }
                  const result =
                    verb === "coordinator.pause"
                      ? options.actions.coordinator.pause()
                      : options.actions.coordinator.resume()
                  return Response.json({ ok: true, result })
                }

                // intent.submit (send_instruction): enqueue an ask for the
                // coordinator to plan + fan out.
                if (verb === "intent.submit") {
                  if (!options.actions.intents) {
                    return Response.json({ error: "intents unavailable on this node" }, { status: 404 })
                  }
                  const e = envelope as {
                    title?: unknown
                    body?: unknown
                    scopeHint?: unknown
                    submittedByClientRef?: unknown
                  }
                  if (typeof e.title !== "string" || typeof e.body !== "string") {
                    return Response.json({ error: "title and body required" }, { status: 400 })
                  }
                  const result = await options.actions.intents.submit({
                    title: e.title,
                    body: e.body,
                    ...(typeof e.scopeHint === "string" ? { scopeHint: e.scopeHint } : {}),
                    ...(typeof e.submittedByClientRef === "string"
                      ? { submittedByClientRef: e.submittedByClientRef }
                      : {}),
                  })
                  return Response.json({ ok: true, result })
                }

                // deploy.cloud (deploy_cloud): node-triggered deploy through OUR
                // pipeline. The node still gates execution behind OA_DEPLOY_ENABLE.
                if (verb === "deploy.cloud") {
                  if (!options.actions.deploy) {
                    return Response.json({ error: "deploy unavailable on this node" }, { status: 404 })
                  }
                  const e = envelope as { target?: unknown; ref?: unknown; env?: unknown }
                  const result = await options.actions.deploy.deployCloud({
                    target: e.target,
                    ref: e.ref,
                    ...(e.env === undefined ? {} : { env: e.env }),
                  })
                  return Response.json({ ok: true, result })
                }

                // decision.resolve: exactly-once approval relay. Backed by the
                // node's approval queue, which is itself exactly-once (a repeat
                // resolve is a no-op duplicate). Requires answer_decision.
                if (verb === "decision.resolve") {
                  if (!options.actions.approvals) {
                    return Response.json({ error: "approvals unavailable on this node" }, { status: 404 })
                  }
                  // Accept the canonical protocol envelope (requestId/decisionVerb,
                  // from buildDecisionResolveEnvelope) and the node-shape compat
                  // form (approvalRef/decision). The node's approvalRef IS the
                  // decision requestId.
                  const approvalRef =
                    typeof envelope.requestId === "string" ? envelope.requestId : envelope.approvalRef
                  const decision =
                    typeof envelope.decisionVerb === "string" ? envelope.decisionVerb : envelope.decision
                  if (
                    typeof approvalRef !== "string" ||
                    (decision !== "approve" && decision !== "deny" && decision !== "answer")
                  ) {
                    return Response.json(
                      { error: "requestId/decisionVerb (or approvalRef/decision: approve|deny|answer) required" },
                      { status: 400 },
                    )
                  }
                  const result = await options.actions.approvals.resolve({
                    approvalRef,
                    decision,
                    ...(typeof envelope.answer === "string" ? { answer: envelope.answer } : {}),
                  })
                  return Response.json({ ok: true, result })
                }

                // All remaining bridge verbs operate on sessions.
                if (!options.actions.sessions) {
                  return Response.json({ error: "sessions unavailable on this node" }, { status: 404 })
                }
                if (verb === "session.list") {
                  return Response.json({ ok: true, result: await options.actions.sessions.list() })
                }
                // Cursor-resumable catch-up: return the session's events; the
                // client dedups/resumes via the shared cursor model. Live
                // streaming uses GET /sessions/:ref/events with the bridge cred.
                if (verb === "session.snapshot" || verb === "session.history" || verb === "session.subscribe") {
                  if (typeof envelope.sessionRef !== "string") {
                    return Response.json({ error: "sessionRef required" }, { status: 400 })
                  }
                  return Response.json({ ok: true, result: await options.actions.sessions.events(envelope.sessionRef) })
                }
                // artifact.read: the retained proof/failure artifact (read_artifact).
                if (verb === "artifact.read") {
                  if (typeof envelope.sessionRef !== "string") {
                    return Response.json({ error: "sessionRef required" }, { status: 400 })
                  }
                  return Response.json({ ok: true, result: await options.actions.sessions.artifact(envelope.sessionRef) })
                }
                // session.cancel: request cancellation (cancel capability).
                if (verb === "session.cancel") {
                  if (typeof envelope.sessionRef !== "string") {
                    return Response.json({ error: "sessionRef required" }, { status: 400 })
                  }
                  return Response.json({ ok: true, result: await options.actions.sessions.cancel(envelope.sessionRef) })
                }
                // G4 (#5496): chat / turn.steer over the capability-scoped
                // bridge. The node maps a session-bound instruction onto the
                // existing session.reply continuation path, preserving parent
                // refs, account/workspace context, receipts, and projections.
                if (verb === "turn.steer") {
                  if (typeof envelope.sessionRef !== "string") {
                    return Response.json({ error: "sessionRef required" }, { status: 400 })
                  }
                  const instruction = typeof envelope.instruction === "string" ? envelope.instruction.trim() : ""
                  if (instruction.length === 0) {
                    return Response.json({ error: "instruction required" }, { status: 400 })
                  }
                  const result = await options.actions.sessions.reply({
                    type: "session.reply",
                    sessionRef: envelope.sessionRef,
                    objective: instruction,
                    ...(typeof envelope.timeoutSeconds === "number" && Number.isFinite(envelope.timeoutSeconds)
                      ? { timeoutSeconds: envelope.timeoutSeconds }
                      : {}),
                  })
                  return Response.json({ ok: true, result })
                }
                // #5494 (epic #5492 G1): session.spawn over the bridge
                // (spawn_session). Routes to the same node spawn action the
                // dev-token /command path uses; the node validates the payload.
                if (verb === "session.spawn") {
                  const e = envelope as {
                    adapter?: unknown
                    objective?: unknown
                    verify?: unknown
                    lane?: unknown
                  }
                  if (e.adapter !== "codex" && e.adapter !== "claude_agent") {
                    return Response.json({ error: "adapter must be codex|claude_agent" }, { status: 400 })
                  }
                  if (typeof e.objective !== "string") {
                    return Response.json({ error: "objective required" }, { status: 400 })
                  }
                  const result = await options.actions.sessions.spawn({
                    type: "session.spawn",
                    adapter: e.adapter,
                    objective: e.objective,
                    verify: Array.isArray(e.verify) ? e.verify.filter((v): v is string => typeof v === "string") : [],
                    ...(typeof e.lane === "string" ? { lane: e.lane as never } : {}),
                  })
                  return Response.json({ ok: true, result })
                }
                return Response.json({ error: "unsupported bridge verb", verb }, { status: 501 })
              } catch {
                return Response.json({ error: "malformed bridge request" }, { status: 400 })
              }
            }

            // #5000 cursor-resumable live subscribe over the bridge: a paired
            // client presenting a valid Bridge credential with an observe-class
            // capability streams a session's live events (same SSE the dev-token
            // path serves). Client resumes/dedups via the shared cursor model.
            const bridgeSessionEventsMatch = /^\/sessions\/([^/]+)\/events$/.exec(url.pathname)
            if (
              bridgeSessionEventsMatch &&
              request.method === "GET" &&
              /^Bridge\s+/.test(request.headers.get("authorization") ?? "")
            ) {
              const header = request.headers.get("authorization") ?? ""
              const bridgeMatch = /^Bridge\s+([^:]+):(.+)$/.exec(header)
              if (bridgeMatch === null) {
                return Response.json({ error: "bridge credential required" }, { status: 401 })
              }
              const claims = bridgePairing.authorize(bridgeMatch[1] ?? "", bridgeMatch[2] ?? "", new Date())
              if (claims === null) {
                return Response.json({ error: "invalid or expired pairing" }, { status: 401 })
              }
              if (!verbAllowedByCapabilities("session.subscribe", claims.capabilities)) {
                return Response.json({ error: "capability not granted", verb: "session.subscribe" }, { status: 403 })
              }
              if (!options.actions.sessions) {
                return Response.json({ error: "sessions unavailable on this node" }, { status: 404 })
              }
              try {
                const stream = options.actions.sessions.eventStream(
                  decodeURIComponent(bridgeSessionEventsMatch[1] ?? ""),
                )
                return new Response(stream, {
                  headers: {
                    "content-type": "text/event-stream",
                    "cache-control": "no-cache",
                    connection: "keep-alive",
                  },
                })
              } catch (error) {
                return Response.json(
                  { error: error instanceof Error ? error.message : String(error) },
                  { status: 404 },
                )
              }
            }

            if (url.pathname === "/api/operator/accounts/status" && request.method === "GET") {
              if (!authorized(request)) {
                return Response.json(
                  { error: "operator ledger access required" },
                  { status: 403, headers: { "cache-control": "no-store" } },
                )
              }
              if (!options.actions.accountsStatus) {
                return Response.json({ error: "account status unavailable" }, { status: 404 })
              }
              return Response.json(await options.actions.accountsStatus(), {
                headers: { "cache-control": "no-store" },
              })
            }

            if (!authorized(request)) return unauthorized()

            const sessionEventsMatch = /^\/sessions\/([^/]+)\/events$/.exec(url.pathname)
            if (sessionEventsMatch && request.method === "GET") {
              if (!options.actions.sessions) {
                return Response.json({ error: "sessions unavailable on this node" }, { status: 404 })
              }
              try {
                const stream = options.actions.sessions.eventStream(
                  decodeURIComponent(sessionEventsMatch[1] ?? ""),
                )
                return new Response(stream, {
                  headers: {
                    "content-type": "text/event-stream",
                    "cache-control": "no-cache",
                    connection: "keep-alive",
                  },
                })
              } catch (error) {
                return Response.json(
                  { error: error instanceof Error ? error.message : String(error) },
                  { status: 404 },
                )
              }
            }

            if (url.pathname === "/events" && request.method === "GET") {
              const snapshot = await Effect.runPromise(captureNodeSnapshot(runtime))
              let heartbeat: ReturnType<typeof setInterval> | undefined
              const stream = new ReadableStream<Uint8Array>({
                start(controller) {
                  controller.enqueue(encoder.encode(sseFrame(snapshot)))
                  clients.add(controller)
                  heartbeat = setInterval(() => {
                    try {
                      controller.enqueue(encoder.encode(": ping\n\n"))
                    } catch {
                      clients.delete(controller)
                      if (heartbeat) clearInterval(heartbeat)
                    }
                  }, 15_000)
                },
                cancel(this: unknown) {
                  if (heartbeat) clearInterval(heartbeat)
                },
              })
              return new Response(stream, {
                headers: {
                  "content-type": "text/event-stream",
                  "cache-control": "no-cache",
                  connection: "keep-alive",
                },
              })
            }

            if (url.pathname === "/command" && request.method === "POST") {
              let command: ControlCommand
              try {
                command = (await request.json()) as ControlCommand
              } catch {
                return Response.json({ error: "invalid json" }, { status: 400 })
              }
              try {
                const result = await runCommand(command)
                return Response.json({ ok: true, result: result ?? null })
              } catch (error: unknown) {
                // #5453: a malformed/invalid command is the caller's mistake, not
                // a node fault. Answer 400 with the typed reason so the desktop
                // can surface a clean message instead of a raw `control 500`.
                // Genuine internal failures still return 500.
                const message = error instanceof Error ? error.message : String(error)
                const validation = controlCommandValidationReason(error)
                if (validation !== null) {
                  return Response.json(
                    { ok: false, error: message, reason: validation },
                    { status: 400 },
                  )
                }
                return Response.json({ ok: false, error: message }, { status: 500 })
              }
            }

            return Response.json({ error: "not found" }, { status: 404 })
          },
        })
      },
      catch: (error) => new Error(`control server failed to start: ${String(error)}`),
    })

    yield* Effect.addFinalizer(() =>
      Effect.sync(() => {
        for (const controller of clients) {
          try {
            controller.close()
          } catch {
            // already closed
          }
        }
        clients.clear()
        server.stop(true)
      }),
    )

    return {
      port: server.port ?? options.port ?? defaultControlPort,
      hostname: options.hostname ?? "127.0.0.1",
      url: `http://${options.hostname ?? "127.0.0.1"}:${server.port}`,
      clientCount: () => clients.size,
    }
  })

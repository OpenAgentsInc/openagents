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
import type { PylonEvent, PylonLogEntry, TelemetryPaneState, WalletPaneState } from "./state"
import type { PylonNodeRuntime } from "./runtime"
import type {
  ControlSessionActions,
  ControlSessionCancelCommand,
  ControlSessionEventsCommand,
  ControlSessionListCommand,
  ControlSessionSpawnCommand,
} from "./control-sessions"

export const defaultControlPort = 4716
export const controlTokenFileName = "control-token"
export const snapshotLogTail = 300

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
  | { type: "assignments.poll" }
  | { type: "assignments.accept"; leaseRef: string }
  | ControlSessionSpawnCommand
  | ControlSessionListCommand
  | ControlSessionEventsCommand
  | ControlSessionCancelCommand

export interface ControlCommandActions {
  walletSend: (destinationRef: string, amountSats?: number) => Promise<unknown>
  walletReceive: (amountSats: number) => Promise<unknown>
  walletAdmitPayoutTarget: (kind: string, ref: string) => Promise<unknown>
  assignmentsPoll?: () => Promise<unknown>
  assignmentsAccept?: (leaseRef: string) => Promise<unknown>
  sessions?: ControlSessionActions
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
        case "assignments.poll":
          if (!options.actions.assignmentsPoll) throw new Error("assignments unavailable on this node")
          return options.actions.assignmentsPoll()
        case "assignments.accept":
          if (!options.actions.assignmentsAccept) throw new Error("assignments unavailable on this node")
          return options.actions.assignmentsAccept(command.leaseRef)
        case "session.spawn":
          if (!options.actions.sessions) throw new Error("sessions unavailable on this node")
          return options.actions.sessions.spawn(command)
        case "session.list":
          if (!options.actions.sessions) throw new Error("sessions unavailable on this node")
          return options.actions.sessions.list()
        case "session.events":
          if (!options.actions.sessions) throw new Error("sessions unavailable on this node")
          return options.actions.sessions.events(command.sessionRef)
        case "session.cancel":
          if (!options.actions.sessions) throw new Error("sessions unavailable on this node")
          return options.actions.sessions.cancel(command.sessionRef)
        default:
          throw new Error(`unknown command: ${(command as { type?: string }).type}`)
      }
    }

    const server = yield* Effect.try({
      try: () =>
        Bun.serve({
          hostname: options.hostname ?? "127.0.0.1",
          port: options.port ?? defaultControlPort,
          idleTimeout: 0,
          fetch: async (request) => {
            const url = new URL(request.url)
            if (url.pathname === "/health") {
              return Response.json({ ok: true, schema: "openagents.pylon.control.v0.3" })
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
              } catch (error) {
                return Response.json(
                  { ok: false, error: error instanceof Error ? error.message : String(error) },
                  { status: 500 },
                )
              }
            }

            return Response.json({ error: "not found" }, { status: 404 })
          },
        }),
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

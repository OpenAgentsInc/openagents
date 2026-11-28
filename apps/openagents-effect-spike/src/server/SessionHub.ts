// Firm reason: 1 (Effect UI / Effect v4 browser and web-server prototype for docs/effect/half-day-sprint.md)
import { Context, Effect, Exit, Layer, PubSub, Scope, Stream } from "effect"
import type {
  ApproveRequest,
  HealthResponse,
  StartTurnRequest,
  SteerRequest,
  StopRequest
} from "../shared/api.ts"
import type {
  ChatMessage,
  SessionSnapshot,
  ToolCallEntry,
  WorkerEvent
} from "../shared/wire.ts"
import {
  probeWorkerAuth,
  spawnOaWorker,
  WorkerProcessError,
  type OaWorkerHandle
} from "./OaWorkerProcess.ts"

export class SessionHubError {
  readonly _tag = "SessionHubError"
  readonly message: string
  constructor(message: string) {
    this.message = message
  }
}

interface ManagedSession {
  readonly sessionId: string
  readonly pubsub: PubSub.PubSub<SessionSnapshot>
  readonly scope: Scope.Closeable
  snapshot: SessionSnapshot
  worker: OaWorkerHandle | null
  nextMsgSeq: number
  nextToolSeq: number
}

const defaultCwd = (): string => process.env["OA_SPIKE_CWD"] || process.cwd()

const initialSnapshot = (sessionId: string, cwd: string): SessionSnapshot => ({
  sessionId,
  seq: 0,
  status: "starting",
  activeTurn: 0,
  title: "",
  cwd,
  model: "",
  messages: [],
  pendingPermit: null,
  todos: [],
  stats: null,
  lastError: null,
  workerPid: null
})

const ensureActiveAssistant = (
  session: ManagedSession,
  messages: ReadonlyArray<ChatMessage>,
  turn: number
): Array<ChatMessage> => {
  const next = [...messages]
  const last = next[next.length - 1]
  if (last && last.role === "assistant" && last.streaming) {
    return next
  }
  session.nextMsgSeq += 1
  next.push({
    id: `msg-${session.nextMsgSeq}`,
    role: "assistant",
    turn,
    thinking: "",
    text: "",
    tools: [],
    ending: null,
    streaming: true
  })
  return next
}

const applyWorkerEvent = (
  session: ManagedSession,
  event: WorkerEvent
): SessionSnapshot => {
  const cur = session.snapshot
  const seq = cur.seq + 1

  switch (event.kind) {
    case "ready":
      return {
        ...cur,
        seq,
        status: cur.status === "starting" ? "ready" : cur.status
      }

    case "journal":
      return { ...cur, seq }

    case "turn": {
      const turn = event.turn > 0 ? event.turn : cur.activeTurn + 1
      const messages = ensureActiveAssistant(session, cur.messages, turn)
      return {
        ...cur,
        seq,
        status: "running",
        activeTurn: turn,
        messages,
        pendingPermit: null,
        lastError: null
      }
    }

    case "thought": {
      const messages = ensureActiveAssistant(session, cur.messages, cur.activeTurn)
      const idx = messages.length - 1
      const tail = messages[idx]!
      messages[idx] = { ...tail, thinking: tail.thinking + event.text }
      return { ...cur, seq, status: "running", messages }
    }

    case "reply": {
      const messages = ensureActiveAssistant(session, cur.messages, cur.activeTurn)
      const idx = messages.length - 1
      const tail = messages[idx]!
      messages[idx] = { ...tail, text: tail.text + event.text }
      return { ...cur, seq, status: "running", messages }
    }

    case "command": {
      const messages = ensureActiveAssistant(session, cur.messages, cur.activeTurn)
      const idx = messages.length - 1
      const tail = messages[idx]!
      session.nextToolSeq += 1
      const entry: ToolCallEntry = {
        id: `tool-${session.nextToolSeq}`,
        kind: "command",
        command: event.command,
        path: "",
        found: true,
        exit: event.exit,
        timedOut: event.timedOut,
        output: event.output
      }
      messages[idx] = { ...tail, tools: [...tail.tools, entry] }
      return { ...cur, seq, status: "running", messages, pendingPermit: null }
    }

    case "view": {
      const messages = ensureActiveAssistant(session, cur.messages, cur.activeTurn)
      const idx = messages.length - 1
      const tail = messages[idx]!
      session.nextToolSeq += 1
      const entry: ToolCallEntry = {
        id: `tool-${session.nextToolSeq}`,
        kind: "view",
        command: "",
        path: event.path,
        found: event.found,
        exit: null,
        timedOut: false,
        output: ""
      }
      messages[idx] = { ...tail, tools: [...tail.tools, entry] }
      return { ...cur, seq, status: "running", messages }
    }

    case "ask":
      // In oa-worker, MWW_ASK is emitted as a non-blocking command-started progress notice
      // (proc_start has already fired); real blocking approval gates emit MWW_PERMIT.
      return { ...cur, seq, status: "running" }

    case "permit":
      return {
        ...cur,
        seq,
        status: "waiting_approval",
        pendingPermit: {
          ticket: event.ticket,
          kind: "permit",
          question: event.question,
          title: event.title || (event.question ? "Question" : "Permission required"),
          command: event.command,
          options: [...event.options]
        }
      }

    case "todos":
      return { ...cur, seq, todos: [...event.todos] }

    case "stats":
      return {
        ...cur,
        seq,
        stats: {
          ttftMs: event.ttftMs,
          decodeMs: event.decodeMs,
          outputTokens: event.outputTokens
        }
      }

    case "meta":
      return {
        ...cur,
        seq,
        title: event.title || cur.title
      }

    case "steer":
      return { ...cur, seq }

    case "ended": {
      const messages = [...cur.messages]
      const last = messages[messages.length - 1]
      if (last && last.role === "assistant" && last.streaming) {
        const finalText = last.text.length > 0 ? last.text : event.text
        messages[messages.length - 1] = {
          ...last,
          text: finalText,
          ending: event.ending,
          streaming: false
        }
      } else if (event.text.length > 0) {
        session.nextMsgSeq += 1
        messages.push({
          id: `msg-${session.nextMsgSeq}`,
          role: "assistant",
          turn: cur.activeTurn,
          thinking: "",
          text: event.text,
          tools: [],
          ending: event.ending,
          streaming: false
        })
      }
      const failed =
        event.ending === "failed" ||
        event.ending === "bad_replies" ||
        event.ending === "no_capacity"
      return {
        ...cur,
        seq,
        status: "ready",
        messages,
        pendingPermit: null,
        lastError: failed ? (event.text || event.ending) : null
      }
    }
  }
}

export class SessionHub extends Context.Service<SessionHub, {
  readonly health: () => Effect.Effect<HealthResponse>
  readonly getSnapshot: (sessionId: string) => Effect.Effect<SessionSnapshot, SessionHubError>
  readonly subscribe: (
    sessionId: string
  ) => Effect.Effect<Stream.Stream<SessionSnapshot>, SessionHubError>
  readonly startTurn: (
    sessionId: string,
    req: StartTurnRequest
  ) => Effect.Effect<SessionSnapshot, SessionHubError>
  readonly approve: (
    sessionId: string,
    req: ApproveRequest
  ) => Effect.Effect<SessionSnapshot, SessionHubError>
  readonly steer: (
    sessionId: string,
    req: SteerRequest
  ) => Effect.Effect<SessionSnapshot, SessionHubError>
  readonly stop: (
    sessionId: string,
    req?: StopRequest | undefined
  ) => Effect.Effect<SessionSnapshot, SessionHubError>
}>()("oa-spike/SessionHub") {
  static readonly layer = Layer.effect(
    SessionHub,
    Effect.gen(function*() {
      const sessions = new Map<string, ManagedSession>()
      const pendingBoot = new Map<string, Promise<ManagedSession>>()

      const publish = (session: ManagedSession, next: SessionSnapshot) => {
        session.snapshot = next
        Effect.runSync(PubSub.publish(session.pubsub, next))
      }

      const bootSession = (
        sessionId: string,
        requestedCwd?: string
      ): Effect.Effect<ManagedSession, SessionHubError> =>
        Effect.gen(function*() {
          const existing = sessions.get(sessionId)
          if (existing && existing.worker !== null) {
            return existing
          }
          const inflight = pendingBoot.get(sessionId)
          if (inflight) {
            return yield* Effect.tryPromise({
              try: () => inflight,
              catch: (e) =>
                e instanceof SessionHubError
                  ? e
                  : new SessionHubError(`Session boot failed: ${String(e)}`)
            })
          }

          const cwd = requestedCwd && requestedCwd.length > 0 ? requestedCwd : defaultCwd()
          const sessionScope = yield* Scope.make()
          const pubsub = yield* PubSub.sliding<SessionSnapshot>(256)
          const managed: ManagedSession = {
            sessionId,
            pubsub,
            scope: sessionScope,
            snapshot: initialSnapshot(sessionId, cwd),
            worker: null,
            nextMsgSeq: 0,
            nextToolSeq: 0
          }
          sessions.set(sessionId, managed)

          const bootPromise = Effect.runPromise(
            spawnOaWorker({
              cwd,
              onEvent: (ev) => {
                publish(managed, applyWorkerEvent(managed, ev))
              },
              onExit: (code, signal, stderr) => {
                managed.worker = null
                const cur = managed.snapshot
                const messages = cur.messages.map((m) =>
                  m.streaming ? { ...m, streaming: false, ending: m.ending ?? "exited" } : m
                )
                publish(managed, {
                  ...cur,
                  seq: cur.seq + 1,
                  status: code === 0 ? "exited" : "error",
                  messages,
                  pendingPermit: null,
                  workerPid: null,
                  lastError:
                    code === 0
                      ? cur.lastError
                      : stderr || `oa-worker exited (code=${String(code)}, signal=${String(signal)})`
                })
              }
            }).pipe(
              Effect.provideService(Scope.Scope, sessionScope),
              Effect.map((worker) => {
                managed.worker = worker
                publish(managed, {
                  ...managed.snapshot,
                  seq: managed.snapshot.seq + 1,
                  status: "ready",
                  workerPid: worker.pid
                })
                return managed
              }),
              Effect.mapError((err: WorkerProcessError) => {
                publish(managed, {
                  ...managed.snapshot,
                  seq: managed.snapshot.seq + 1,
                  status: "error",
                  lastError: err.message
                })
                sessions.delete(sessionId)
                return new SessionHubError(err.message)
              })
            )
          )

          pendingBoot.set(sessionId, bootPromise)
          try {
            return yield* Effect.tryPromise({
              try: () => bootPromise,
              catch: (e) =>
                e instanceof SessionHubError
                  ? e
                  : new SessionHubError(`Session boot failed: ${String(e)}`)
            })
          } finally {
            pendingBoot.delete(sessionId)
          }
        })

      yield* Effect.addFinalizer(() =>
        Effect.forEach(
          Array.from(sessions.values()),
          (s) => Scope.close(s.scope, Exit.void).pipe(Effect.ignore),
          { discard: true }
        )
      )

      const ensureWorker = (
        sessionId: string,
        cwd?: string
      ): Effect.Effect<{ readonly session: ManagedSession; readonly worker: OaWorkerHandle }, SessionHubError> =>
        Effect.gen(function*() {
          const session = yield* bootSession(sessionId, cwd)
          if (session.worker === null) {
            return yield* Effect.fail(new SessionHubError("oa-worker is not running"))
          }
          return { session, worker: session.worker }
        })

      return SessionHub.of({
        health: () =>
          Effect.gen(function*() {
            const worker = yield* probeWorkerAuth()
            return {
              ok: worker.ready,
              worker,
              activeSessions: sessions.size
            }
          }),

        getSnapshot: (sessionId) =>
          Effect.map(bootSession(sessionId), (s) => s.snapshot),

        subscribe: (sessionId) =>
          Effect.gen(function*() {
            const session = yield* bootSession(sessionId)
            const live = Stream.fromPubSub(session.pubsub)
            return Stream.concat(Stream.succeed(session.snapshot), live)
          }),

        startTurn: (sessionId, req) =>
          Effect.gen(function*() {
            const task = req.task.trim()
            if (task.length === 0) {
              return yield* Effect.fail(new SessionHubError("task must be non-empty"))
            }
            const { session, worker } = yield* ensureWorker(sessionId, req.cwd)
            const cur = session.snapshot
            const nextTurn = cur.activeTurn + 1
            const cwd = req.cwd && req.cwd.length > 0 ? req.cwd : cur.cwd
            session.nextMsgSeq += 1
            const userMsg: ChatMessage = {
              id: `msg-${session.nextMsgSeq}`,
              role: "user",
              turn: nextTurn,
              thinking: "",
              text: task,
              tools: [],
              ending: null,
              streaming: false
            }
            publish(session, {
              ...cur,
              seq: cur.seq + 1,
              status: "running",
              activeTurn: nextTurn,
              cwd,
              model: req.model ?? cur.model,
              messages: [...cur.messages, userMsg],
              pendingPermit: null,
              lastError: null
            })
            yield* worker
              .sendTask({
                task,
                session: sessionId,
                cwd,
                gated: req.gated ?? true,
                local: req.local,
                model: req.model,
                effort: req.effort,
                mode: req.mode
              })
              .pipe(Effect.mapError((e) => new SessionHubError(e.message)))
            return session.snapshot
          }),

        approve: (sessionId, req) =>
          Effect.gen(function*() {
            const { session, worker } = yield* ensureWorker(sessionId)
            const cur = session.snapshot
            const cleared =
              cur.pendingPermit !== null && cur.pendingPermit.ticket === req.ticket
                ? null
                : cur.pendingPermit
            publish(session, {
              ...cur,
              seq: cur.seq + 1,
              status: cleared === null ? "running" : cur.status,
              pendingPermit: cleared
            })
            yield* worker
              .sendAnswer({
                answer: req.answer,
                ticket: req.ticket,
                why: req.why,
                option: req.option
              })
              .pipe(Effect.mapError((e) => new SessionHubError(e.message)))
            return session.snapshot
          }),

        steer: (sessionId, req) =>
          Effect.gen(function*() {
            const steer = req.steer.trim()
            if (steer.length === 0) {
              return yield* Effect.fail(new SessionHubError("steer must be non-empty"))
            }
            const { session, worker } = yield* ensureWorker(sessionId)
            const id = req.id && req.id.length > 0 ? req.id : `steer-${Date.now()}`
            yield* worker
              .sendSteer({
                id,
                steer,
                turn: req.turn ?? session.snapshot.activeTurn
              })
              .pipe(Effect.mapError((e) => new SessionHubError(e.message)))
            return session.snapshot
          }),

        stop: (sessionId, req) =>
          Effect.gen(function*() {
            const { session, worker } = yield* ensureWorker(sessionId)
            yield* (req?.interrupt
              ? worker.sendInterrupt(req.resubmit ?? false)
              : worker.sendStop()
            ).pipe(Effect.mapError((e) => new SessionHubError(e.message)))
            return session.snapshot
          })
      })
    })
  )
}

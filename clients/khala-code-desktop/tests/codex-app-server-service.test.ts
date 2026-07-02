import { EventEmitter } from "node:events"
import { describe, expect, test } from "bun:test"
import { Cause, Effect, Exit, Option, Schema as S, Stream } from "effect"

import {
  CodexAppServer,
  CodexAppServerLive,
  CodexAppServerDecodeFailure,
  CodexAppServerRpcTimeout,
  CodexAppServerUnavailable,
  makeCodexAppServerService,
} from "../src/bun/codex-app-server-service"

type FakeChild = EventEmitter & {
  readonly pid: number
  readonly stdout: EventEmitter
  readonly stderr: EventEmitter
  readonly stdin: { readonly write: (line: string) => void }
  readonly kill: () => void
  killed: boolean
}

type WireMessage = {
  readonly id?: number | string
  readonly method?: string
  readonly params?: unknown
  readonly result?: unknown
}

const makeChild = (
  onWrite: (message: WireMessage, child: FakeChild) => void,
  pid = 4321,
): FakeChild => {
  const child = new EventEmitter() as FakeChild
  Object.assign(child, {
    killed: false,
    pid,
    stderr: new EventEmitter(),
    stdin: {
      write: (line: string) => {
        const message = JSON.parse(line) as WireMessage
        queueMicrotask(() => onWrite(message, child))
      },
    },
    stdout: new EventEmitter(),
    kill: () => {
      child.killed = true
      queueMicrotask(() => child.emit("close", 0, null))
    },
  })
  return child
}

const respond = (child: FakeChild, id: number | string | undefined, result: unknown): void => {
  if (id === undefined) return
  child.stdout.emit("data", Buffer.from(`${JSON.stringify({ id, result })}\n`))
}

const notify = (
  child: FakeChild,
  method: string,
  params: unknown = {},
  id?: number | string,
): void => {
  child.stdout.emit("data", Buffer.from(`${JSON.stringify({
    ...(id === undefined ? {} : { id }),
    method,
    params,
  })}\n`))
}

const typedFailure = <E>(exit: Exit.Exit<unknown, E>): E => {
  expect(Exit.isFailure(exit)).toBe(true)
  const failure = Exit.isFailure(exit)
    ? Cause.findErrorOption(exit.cause)
    : Option.none()
  if (Option.isNone(failure)) {
    throw new Error("expected a typed Effect failure")
  }
  return failure.value
}

describe("CodexAppServer Effect service", () => {
  test("decodes app-server responses through caller-provided schemas", async () => {
    const service = makeCodexAppServerService({
      spawnFn: () => makeChild((message, child) => {
        if (message.method === "initialize") {
          respond(child, message.id, {})
          return
        }
        if (message.method === "model/list") {
          respond(child, message.id, { models: [{ id: "gpt-5.1-codex" }] })
        }
      }),
    })
    const ModelList = S.Struct({
      models: S.Array(S.Struct({ id: S.String })),
    })

    await Effect.runPromise(service.start)
    const result = await Effect.runPromise(service.requestDecoded(ModelList, "model/list"))

    expect(result.models[0]?.id).toBe("gpt-5.1-codex")
    await Effect.runPromise(service.dispose)
  })

  test("surfaces schema decode failures as tagged service errors", async () => {
    const service = makeCodexAppServerService({
      spawnFn: () => makeChild((message, child) => {
        if (message.method === "initialize") {
          respond(child, message.id, {})
          return
        }
        if (message.method === "model/list") {
          respond(child, message.id, { models: [{ id: 123 }] })
        }
      }),
    })
    const ModelList = S.Struct({
      models: S.Array(S.Struct({ id: S.String })),
    })

    await Effect.runPromise(service.start)
    const exit = await Effect.runPromiseExit(service.requestDecoded(ModelList, "model/list"))

    const failure = typedFailure(exit)
    expect(failure).toBeInstanceOf(CodexAppServerDecodeFailure)
    expect(failure).toMatchObject({
      boundary: "response",
      method: "model/list",
    })
    await Effect.runPromise(service.dispose)
  })

  test("delivers notifications as isolated decoded streams", async () => {
    const service = makeCodexAppServerService({
      spawnFn: () => makeChild((message, child) => {
        if (message.method === "initialize") {
          respond(child, message.id, {})
          queueMicrotask(() => notify(child, "thread/status/changed", { threadId: "thread-1" }))
        }
      }),
    })
    const failingConsumer = Effect.runPromiseExit(
      service.notifications().pipe(
        Stream.runForEach(() => Effect.fail("subscriber failed")),
      ),
    )
    const succeedingConsumer = Effect.runPromise(
      service.notifications().pipe(Stream.runHead),
    )

    await Effect.runPromise(service.start)
    const [failed, notification] = await Promise.all([failingConsumer, succeedingConsumer])

    expect(Exit.isFailure(failed)).toBe(true)
    expect(notification._tag).toBe("Some")
    if (notification._tag === "Some") {
      expect(notification.value.method).toBe("thread/status/changed")
    }
    await Effect.runPromise(service.dispose)
  })

  test("fires turn/interrupt when a request times out with an active turn", async () => {
    const writes: WireMessage[] = []
    const service = makeCodexAppServerService({
      requestTimeoutMs: 5,
      spawnFn: () => makeChild((message, child) => {
        writes.push(message)
        if (message.method === "initialize") {
          respond(child, message.id, {})
          return
        }
        if (message.method === "turn/interrupt") {
          expect(message.params).toEqual({
            threadId: "thread-1",
            turnId: "turn-1",
          })
          respond(child, message.id, {})
        }
      }),
    })

    await Effect.runPromise(service.start)
    const exit = await Effect.runPromiseExit(service.request("turn/start", {
      threadId: "thread-1",
    }, {
      interruptOnTimeout: { threadId: "thread-1", turnId: "turn-1" },
      timeoutMs: 5,
    }))

    expect(writes.map(write => write.method)).toContain("turn/interrupt")
    const failure = typedFailure(exit)
    expect(failure).toBeInstanceOf(CodexAppServerRpcTimeout)
    expect(failure).toMatchObject({
      interruptAttempted: true,
      interruptOk: true,
      method: "turn/start",
    })
    await Effect.runPromise(service.dispose)
  })

  test("scoped service disposal kills the supervised app-server child", async () => {
    let child: FakeChild | null = null
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function*() {
          const service = yield* CodexAppServer
          yield* service.start
        }).pipe(
          Effect.provide(CodexAppServerLive({
            spawnFn: () => makeChild((message, rpcChild) => {
              child = rpcChild
              if (message.method === "initialize") respond(rpcChild, message.id, {})
            }),
          })),
        ),
      ),
    )

    expect(child).not.toBeNull()
    expect((child as unknown as FakeChild).killed).toBe(true)
  })

  test("reports unavailable app-server requests as typed errors", async () => {
    const service = makeCodexAppServerService()
    const exit = await Effect.runPromiseExit(service.request("thread/list"))

    expect(typedFailure(exit)).toBeInstanceOf(CodexAppServerUnavailable)
  })
})

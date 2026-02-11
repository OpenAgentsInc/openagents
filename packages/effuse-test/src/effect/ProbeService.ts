import * as Fs from "node:fs/promises"
import * as Path from "node:path"

import { Chunk, Context, Effect, Layer, Queue, Ref } from "effect"

import type { TestEvent } from "../spec.ts"

const toError = (cause: unknown): Error =>
  cause instanceof Error ? cause : new Error(String(cause))

export class ProbeServiceError extends Error {
  readonly operation: string
  override readonly cause: unknown

  constructor(operation: string, cause: unknown) {
    const err = toError(cause)
    super(`[ProbeService] ${operation}: ${err.message}`)
    this.name = "ProbeServiceError"
    this.operation = operation
    this.cause = cause
  }
}

const tryProbePromise = <A>(operation: string, f: () => Promise<A>) =>
  Effect.tryPromise({
    try: f,
    catch: (cause) => new ProbeServiceError(operation, cause),
  })

type ProbeServiceApi = {
  readonly emit: (event: TestEvent) => Effect.Effect<void>
  readonly flush: Effect.Effect<void>
}

export class ProbeService extends Context.Tag("@openagentsinc/effuse-test/ProbeService")<
  ProbeService,
  ProbeServiceApi
>() {}

export type ProbeOptions = {
  readonly eventsPath: string
  readonly broadcast?: (event: TestEvent) => void
  readonly capacity?: number
}

export const ProbeServiceLive = (
  options: ProbeOptions,
): Layer.Layer<ProbeService, ProbeServiceError> =>
  Layer.scoped(
    ProbeService,
    Effect.gen(function* () {
      const capacity = options.capacity ?? 2048
      const queue = yield* Queue.dropping<TestEvent>(capacity)
      const inFlight = yield* Ref.make(0)

      yield* tryProbePromise("fs.mkdir(events dir)", () =>
        Fs.mkdir(Path.dirname(options.eventsPath), { recursive: true }),
      )
      yield* tryProbePromise("fs.writeFile(events init)", () =>
        Fs.writeFile(options.eventsPath, "", { flag: "w" }),
      )

      const drain = Queue.takeBetween(queue, 1, 256).pipe(
        Effect.flatMap((chunk) =>
          Ref.update(inFlight, (n) => n + 1).pipe(
            Effect.zipRight(
              tryProbePromise("fs.appendFile(events)", () =>
                Fs.appendFile(
                  options.eventsPath,
                  `${Chunk.toReadonlyArray(chunk)
                    .map((e) => JSON.stringify(e))
                    .join("\n")}\n`,
                  "utf8",
                ),
              ).pipe(Effect.asVoid, Effect.catchAll(() => Effect.void)),
            ),
            Effect.zipRight(
              Effect.sync(() => {
                if (!options.broadcast) return
                for (const event of Chunk.toReadonlyArray(chunk)) options.broadcast(event)
              }),
            ),
            Effect.ensuring(Ref.update(inFlight, (n) => Math.max(0, n - 1))),
          ),
        ),
        Effect.forever,
        Effect.forkScoped,
      )

      yield* drain

      const emit: ProbeServiceApi["emit"] = Effect.fn("effuseTest.probe.emit")(
        function* (event: TestEvent) {
          yield* Queue.offer(queue, event).pipe(
            // Dropping queue returns false when full; we drop silently.
            Effect.asVoid,
          )
        },
      )

      const flush: ProbeServiceApi["flush"] = Effect.gen(function* () {
        const deadline = Date.now() + 5_000
        // eslint-disable-next-line no-constant-condition
        while (true) {
          const size = yield* Queue.size(queue)
          const writing = yield* Ref.get(inFlight)
          if (size === 0 && writing === 0) return
          if (Date.now() > deadline) return
          yield* Effect.sleep("50 millis")
        }
      }).pipe(Effect.withSpan("effuseTest.probe.flush"))

      return ProbeService.of({ emit, flush })
    }),
  )

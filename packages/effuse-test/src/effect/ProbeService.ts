import * as Fs from "node:fs/promises"
import * as Path from "node:path"

import { Chunk, Context, Effect, Layer, Queue, Ref } from "effect"

import type { TestEvent } from "../spec.ts"

export class ProbeService extends Context.Tag("@openagentsinc/effuse-test/ProbeService")<
  ProbeService,
  {
    readonly emit: (event: TestEvent) => Effect.Effect<void>
    readonly flush: Effect.Effect<void>
  }
>() {}

export type ProbeOptions = {
  readonly eventsPath: string
  readonly broadcast?: (event: TestEvent) => void
  readonly capacity?: number
}

export const ProbeServiceLive = (options: ProbeOptions): Layer.Layer<ProbeService> =>
  Layer.scoped(
    ProbeService,
    Effect.gen(function* () {
      const capacity = options.capacity ?? 2048
      const queue = yield* Queue.dropping<TestEvent>(capacity)
      const inFlight = yield* Ref.make(0)

      yield* Effect.promise(() => Fs.mkdir(Path.dirname(options.eventsPath), { recursive: true }))
      yield* Effect.promise(() => Fs.writeFile(options.eventsPath, "", { flag: "w" }))

      const drain = Queue.takeBetween(queue, 1, 256).pipe(
        Effect.flatMap((chunk) =>
          Ref.update(inFlight, (n) => n + 1).pipe(
            Effect.zipRight(
              Effect.tryPromise({
                try: () =>
                  Fs.appendFile(
                    options.eventsPath,
                    `${Chunk.toReadonlyArray(chunk)
                      .map((e) => JSON.stringify(e))
                      .join("\n")}\n`,
                    "utf8",
                  ),
                catch: () => undefined,
              }).pipe(Effect.asVoid),
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

      const emit = (event: TestEvent) =>
        Queue.offer(queue, event).pipe(
          // Dropping queue returns false when full; we drop silently.
          Effect.asVoid,
          Effect.catchAll(() => Effect.void),
        )

      const flush = Effect.gen(function* () {
        const deadline = Date.now() + 5_000
        // eslint-disable-next-line no-constant-condition
        while (true) {
          const size = yield* Queue.size(queue)
          const writing = yield* Ref.get(inFlight)
          if (size === 0 && writing === 0) return
          if (Date.now() > deadline) return
          yield* Effect.sleep("50 millis")
        }
      })

      return ProbeService.of({ emit, flush })
    }),
  )

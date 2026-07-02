import { Context, Effect, Layer, Random } from "effect"
import { TestClock } from "effect/testing"

export type KhalaQaTransportRequest = Readonly<{
  method: string
  payload: unknown
}>

export class KhalaQaTransport extends Context.Service<
  KhalaQaTransport,
  {
    readonly call: (
      request: KhalaQaTransportRequest,
    ) => Effect.Effect<unknown, KhalaQaTransportError>
    readonly calls: Effect.Effect<ReadonlyArray<KhalaQaTransportRequest>>
  }
>()("@openagentsinc/khala-qa-harness/KhalaQaTransport") {}

export class KhalaQaTransportError {
  readonly _tag = "KhalaQaTransportError"

  constructor(readonly reason: string) {}
}

export const stubTransportLayer = (
  responses: Readonly<Record<string, unknown>>,
): Layer.Layer<KhalaQaTransport> =>
  Layer.effect(
    KhalaQaTransport,
    Effect.sync(() => {
      const recorded: KhalaQaTransportRequest[] = []
      return {
        call: (request: KhalaQaTransportRequest) =>
          Effect.sync(() => recorded.push(request)).pipe(
            Effect.flatMap(() =>
              request.method in responses
                ? Effect.succeed(responses[request.method])
                : Effect.fail(new KhalaQaTransportError(`unscripted method: ${request.method}`)),
            ),
          ),
        calls: Effect.sync(() => [...recorded]),
      }
    }),
  )

export const TestEnvironmentLayer = (
  responses: Readonly<Record<string, unknown>> = {},
): Layer.Layer<KhalaQaTransport> =>
  Layer.mergeAll(TestClock.layer(), stubTransportLayer(responses))

export const withSeed = <A, E, R>(
  seed: string,
  effect: Effect.Effect<A, E, R>,
): Effect.Effect<A, E, R> => Effect.suspend(() => Random.withSeed(seed)(effect))

export const currentMillis: Effect.Effect<number> = Effect.clockWith(clock =>
  clock.currentTimeMillis,
)

export { TestClock }

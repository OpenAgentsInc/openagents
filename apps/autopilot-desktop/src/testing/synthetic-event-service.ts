// Demonstrator service that consumes the deterministic environment.
//
// This models exactly the kind of seam that was previously written with
// `Date.now()` + `Math.random()` inline (untestable, flaky): minting a synthetic
// in-world event (a timestamp + a seeded id) and shipping it over a transport.
// Here every nondeterministic input is an INJECTED service, so the same seed +
// the same TestClock produce byte-identical output every run, and the transport
// call is observable. This is the pattern other services should follow.

import { Context, Effect, Layer, Random } from "effect"

import { Transport, type TransportError } from "./deterministic-env.js"

export type SyntheticEvent = Readonly<{
  id: string
  // Minted from the (test) Clock — deterministic, not wall-clock.
  generatedAtMillis: number
}>

export class SyntheticEventService extends Context.Service<
  SyntheticEventService,
  {
    // Mint a synthetic event and ship it over the transport. The id is derived
    // from the seeded RNG; the timestamp from the injected Clock.
    readonly emit: (
      kind: string,
    ) => Effect.Effect<SyntheticEvent, TransportError>
  }
>()("autopilot-desktop/testing/SyntheticEventService") {}

export const SyntheticEventServiceLayer = Layer.effect(
  SyntheticEventService,
  Effect.gen(function* () {
    const transport = yield* Transport
    return {
      emit: (kind: string) =>
        Effect.gen(function* () {
          // Injected randomness — reproducible under withSeed.
          const roll = yield* Random.nextIntBetween(0, 1_000_000)
          // Injected clock — TestClock in tests, real clock in production.
          const generatedAtMillis = yield* Effect.clockWith((clock) =>
            clock.currentTimeMillis,
          )
          const id = `${kind}-${generatedAtMillis}-${roll.toString(36)}`
          yield* transport.call({
            method: "world.emitSyntheticEvent",
            payload: { id, kind, generatedAtMillis },
          })
          return { id, generatedAtMillis }
        }),
    }
  }),
)

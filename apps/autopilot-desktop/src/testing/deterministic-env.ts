// Effect Layer-injected deterministic test environment.
//
// The rule (effect-solutions `testing`): code under test must NOT reach for
// `Date.now`, `Math.random`, or `requestAnimationFrame`/wall-clock. Those come
// from INJECTED services so a test fully controls them, and the same input
// always produces the same output. This module provides the small, reusable
// service set those tests share:
//
//   • Clock      → Effect TestClock (advance time explicitly via TestClock.adjust)
//   • Random     → seeded (Random.withSeed) — reproducible "randomness"
//   • Transport  → a scripted stub (no network) injected by Layer
//
// Effect 4 (beta.70) APIs: TestClock lives in `effect/testing`, deterministic
// randomness is `Random.withSeed("seed")`, and services are `Context.Service`.

import { Context, Effect, Layer, Random } from "effect"
import { TestClock } from "effect/testing"

// ── Transport service (stubbed by Layer) ──────────────────────────────────────
//
// A minimal request/response transport. Production wires a real fetch/RPC layer;
// tests wire `stubTransportLayer(scriptedResponses)` so a service under test
// talks to a scripted fake instead of the network. This is the "stub transport"
// the harness reuses.
export type TransportRequest = Readonly<{ method: string; payload: unknown }>

export class Transport extends Context.Service<
  Transport,
  {
    readonly call: (
      request: TransportRequest,
    ) => Effect.Effect<unknown, TransportError>
    // Every request the transport has seen, in order — so a test can assert the
    // exact calls a service made.
    readonly calls: Effect.Effect<ReadonlyArray<TransportRequest>>
  }
>()("autopilot-desktop/testing/Transport") {}

export class TransportError {
  readonly _tag = "TransportError"
  constructor(readonly reason: string) {}
}

// A deterministic stub transport: returns scripted responses keyed by method,
// records every call, and fails (never hangs) on an unscripted method.
export const stubTransportLayer = (
  responses: Readonly<Record<string, unknown>>,
): Layer.Layer<Transport> =>
  Layer.effect(
    Transport,
    Effect.sync(() => {
      const recorded: TransportRequest[] = []
      return {
        call: (request: TransportRequest) =>
          Effect.sync(() => recorded.push(request)).pipe(
            Effect.flatMap(() =>
              request.method in responses
                ? Effect.succeed(responses[request.method])
                : Effect.fail(
                    new TransportError(`unscripted method: ${request.method}`),
                  ),
            ),
          ),
        calls: Effect.sync(() => [...recorded]),
      }
    }),
  )

// ── The deterministic environment layer ───────────────────────────────────────
//
// Combines the TestClock (deterministic, explicit time) with a stub transport.
// Randomness is applied per-effect with `withSeed` (below) rather than baked in,
// so each test can pick its own seed and have it recorded as evidence.
export const TestEnvironmentLayer = (
  responses: Readonly<Record<string, unknown>> = {},
): Layer.Layer<Transport> =>
  Layer.mergeAll(TestClock.layer(), stubTransportLayer(responses))

// Run an effect under a fixed RNG seed so "random" is reproducible. `seed` is
// returned alongside the value so a test can record it as evidence.
export const withSeed = <A, E, R>(
  seed: string,
  effect: Effect.Effect<A, E, R>,
): Effect.Effect<A, E, R> => Random.withSeed(seed)(effect)

// Convenience: read the current TestClock time (ms). Starts at 0 and only moves
// when a test calls `TestClock.adjust`.
export const currentMillis: Effect.Effect<number> = Effect.clockWith((clock) =>
  clock.currentTimeMillis,
)

export { TestClock }

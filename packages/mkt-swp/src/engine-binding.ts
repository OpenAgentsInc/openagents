/**
 * How the wasm engine is loaded and bound (SWAP-0, openagents#9315).
 *
 * The Immortal client crate compiles to `wasm32-unknown-unknown` and is
 * exposed to JavaScript through a thin binding. The binding is not imported
 * statically: a swap surface must be able to render its honest
 * engine-loading and engine-failed states, and a failed load must never be
 * able to read as a pass. So the module arrives through a port —
 * `EngineModuleLoader` — that the host supplies, and `engineLayer` turns
 * whatever that loader returns into the `SwapEngine` service.
 *
 * Consequences of the port shape, all deliberate:
 *
 * - Loading is an Effect with one typed failure (`EngineLoadFailed`), so the
 *   widget's `EngineLoading` and `EngineFailed` states are reachable from
 *   the real lifecycle rather than simulated.
 * - The engine takes key material and entropy as bytes and has none of its
 *   own; `EntropySource` (WebCrypto in the browser) supplies them.
 * - The layer is swappable: `fixtureEngineLayer` satisfies the same service
 *   tag, so dev/staging and every test drive the identical contract.
 * - Nothing here parses a profile record. The loader's only job is to hand
 *   back something that implements the boundary.
 */
import { Context, Effect, Layer, Schema } from "effect";
import { Service, type Interface } from "./swap-engine.js";

export class EngineLoadFailed extends Schema.TaggedErrorClass<EngineLoadFailed>()(
  "MktSwp.EngineLoadFailed",
  {
    /**
     * `unavailable` — the wasm module could not be fetched, compiled, or
     * instantiated on this host. `incompatible` — it loaded but does not
     * satisfy the pinned engine contract version.
     */
    reason: Schema.Literals(["unavailable", "incompatible"]),
    detail: Schema.String,
  },
) {}

/**
 * Supplied by the host. In the browser this dynamically imports the
 * generated wasm-bindgen module, awaits its `init`, and wraps its exports
 * in the boundary; on Node it may load the same module or a fixture.
 */
export interface LoaderInterface {
  readonly load: () => Effect.Effect<Interface, EngineLoadFailed>;
}

export class EngineModuleLoader extends Context.Service<EngineModuleLoader, LoaderInterface>()(
  "@openagentsinc/mkt-swp/EngineModuleLoader",
) {}

/**
 * Bind the loaded module to the `SwapEngine` tag. Acquisition fails loudly
 * with `EngineLoadFailed`: there is no fallback engine, because a fallback
 * would be a second implementation of the thing that authorises funding.
 */
export const engineLayer: Layer.Layer<Service, EngineLoadFailed, EngineModuleLoader> = Layer.effect(
  Service,
  Effect.gen(function* () {
    const loader = yield* EngineModuleLoader;
    const engine = yield* loader.load();
    return Service.of(engine);
  }),
);

/**
 * Build a loader from a plain async factory — the shape a wasm-bindgen entry
 * point has. The host writes
 * `loaderLayer(async () => (await import("...")).makeEngine(...))`.
 */
export const loaderLayer = (factory: () => Promise<Interface>): Layer.Layer<EngineModuleLoader> =>
  Layer.sync(EngineModuleLoader, () =>
    EngineModuleLoader.of({
      load: () =>
        Effect.tryPromise({
          try: factory,
          catch: (cause) =>
            new EngineLoadFailed({
              reason: "unavailable",
              detail: cause instanceof Error ? cause.message : "engine module did not load",
            }),
        }),
    }),
  );

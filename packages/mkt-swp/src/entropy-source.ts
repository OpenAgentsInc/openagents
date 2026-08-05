import { Context, Effect, Layer } from "effect";

/**
 * Host-supplied entropy. The engine has no randomness source of its own and
 * takes key material as bytes, so every random value on this surface —
 * idempotency keys today, key and preimage material when SWAP-4 lands —
 * comes from the host through this service. In the browser and on Node 24
 * that is WebCrypto.
 */
export interface Interface {
  readonly bytes: (length: number) => Effect.Effect<Uint8Array>;
}

export class Service extends Context.Service<Service, Interface>()(
  "@openagentsinc/mkt-swp/EntropySource",
) {}

/** WebCrypto-backed entropy (browser and Node 24 `globalThis.crypto`). */
export const webCryptoLayer = Layer.sync(Service, () =>
  Service.of({
    bytes: (length) => Effect.sync(() => globalThis.crypto.getRandomValues(new Uint8Array(length))),
  }),
);

export * as EntropySource from "./entropy-source.js";

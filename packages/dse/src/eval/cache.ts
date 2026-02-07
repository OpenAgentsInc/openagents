import { Context, Effect, Layer, Schema } from "effect";

import { sha256IdFromCanonicalJson } from "../hashes.js";

export type EvalCacheKeyV1 = {
  readonly signatureId: string;
  readonly compiled_id: string;
  readonly datasetHash: string;
  readonly metricId: string;
  readonly metricVersion: number;
  readonly exampleId: string;
};

export function evalCacheKeyId(key: EvalCacheKeyV1) {
  return sha256IdFromCanonicalJson({
    ...key,
    format: "openagents.dse.eval_cache_key",
    formatVersion: 1
  });
}

export class EvalCacheError extends Schema.TaggedError<EvalCacheError>()(
  "EvalCacheError",
  {
    message: Schema.String,
    cause: Schema.optional(Schema.Defect)
  }
) {}

export type EvalCache<A> = {
  readonly get: (keyId: string) => Effect.Effect<A | null, EvalCacheError>;
  readonly set: (keyId: string, value: A) => Effect.Effect<void, EvalCacheError>;
};

export class EvalCacheService extends Context.Tag("@openagentsinc/dse/EvalCache")<
  EvalCacheService,
  EvalCache<unknown>
>() {}

export function layerNoop(): Layer.Layer<EvalCacheService> {
  return Layer.succeed(
    EvalCacheService,
    EvalCacheService.of({
      get: () => Effect.succeed(null),
      set: () => Effect.void
    })
  );
}

export function layerInMemory(): Layer.Layer<EvalCacheService> {
  return Layer.sync(EvalCacheService, () => {
    const cache = new Map<string, unknown>();
    return EvalCacheService.of({
      get: (keyId) => Effect.sync(() => cache.get(keyId) ?? null),
      set: (keyId, value) => Effect.sync(() => void cache.set(keyId, value))
    });
  });
}


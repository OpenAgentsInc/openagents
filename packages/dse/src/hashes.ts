import { Effect, JSONSchema, Schema } from "effect";

import type { DseParams } from "./params.js";
import type { PromptIR } from "./promptIr.js";
import type { LmMessage } from "./runtime/lm.js";

import {
  sha256IdFromCanonicalJson as sha256IdFromCanonicalJsonUnsafe
} from "./internal/hash.js";

export class HashError extends Schema.TaggedError<HashError>()("HashError", {
  message: Schema.String,
  cause: Schema.optional(Schema.Defect)
}) {}

export function sha256IdFromCanonicalJson(
  value: unknown
): Effect.Effect<string, HashError> {
  return Effect.tryPromise({
    try: () => sha256IdFromCanonicalJsonUnsafe(value),
    catch: (cause) =>
      HashError.make({
        message: "Failed to hash canonical JSON",
        cause
      })
  });
}

export function promptIrHash<I, O>(
  promptIr: PromptIR<I, O>
): Effect.Effect<string, HashError> {
  return sha256IdFromCanonicalJson(promptIr);
}

export function renderedPromptHash(
  messages: ReadonlyArray<LmMessage>
): Effect.Effect<string, HashError> {
  return sha256IdFromCanonicalJson(messages);
}

export function schemaJsonHash(
  schema: Schema.Schema<any>
): Effect.Effect<string, HashError> {
  return sha256IdFromCanonicalJson(JSONSchema.make(schema));
}

export function paramsHash(params: DseParams): Effect.Effect<string, HashError> {
  return sha256IdFromCanonicalJson(params);
}

export function compiledIdForParams(
  params: DseParams
): Effect.Effect<string, HashError> {
  // For now, compiled_id is the canonical policy hash (DseParams).
  // If policy bundles grow beyond params, keep this stable and adjust hashing inputs.
  return paramsHash(params);
}


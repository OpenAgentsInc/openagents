import { Context, Effect, Layer, Schema } from "effect";

import type { BlobRef } from "../blob.js";

import { sha256IdFromString } from "../internal/hash.js";

export class BlobStoreError extends Schema.TaggedError<BlobStoreError>()(
  "BlobStoreError",
  {
    message: Schema.String,
    cause: Schema.optional(Schema.Defect)
  }
) {}

export type BlobStore = {
  readonly putText: (options: {
    readonly text: string;
    readonly mime?: string;
  }) => Effect.Effect<BlobRef, BlobStoreError>;
  readonly getText: (id: string) => Effect.Effect<string | null, BlobStoreError>;
};

export class BlobStoreService extends Context.Tag("@openagentsinc/dse/BlobStore")<
  BlobStoreService,
  BlobStore
>() {}

function byteLengthUtf8(text: string): number {
  return new TextEncoder().encode(text).byteLength;
}

export function layerInMemory(): Layer.Layer<BlobStoreService> {
  return Layer.sync(BlobStoreService, () => {
    const blobs = new Map<string, { readonly text: string; readonly ref: BlobRef }>();

    const putText: BlobStore["putText"] = (options) =>
      Effect.tryPromise({
        try: async () => {
          const hash = await sha256IdFromString(options.text);
          const existing = blobs.get(hash);
          if (existing) return existing.ref;

          const ref: BlobRef = {
            id: hash,
            hash,
            size: byteLengthUtf8(options.text),
            ...(options.mime ? { mime: options.mime } : {})
          };
          blobs.set(hash, { text: options.text, ref });
          return ref;
        },
        catch: (cause) =>
          BlobStoreError.make({
            message: "Failed to put text blob",
            cause
          })
      });

    const getText: BlobStore["getText"] = (id) =>
      Effect.sync(() => blobs.get(id)?.text ?? null);

    return BlobStoreService.of({ putText, getText });
  });
}


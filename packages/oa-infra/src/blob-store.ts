/**
 * BlobStore — owned object-storage primitive (CFG-2, issue #8517, audit §5).
 *
 * Semantics WE define (any backend must pass src/conformance/blob-store.ts):
 * - `get` of a missing key is `null`; `put` overwrites; `delete` is
 *   idempotent.
 * - `list(prefix)` returns ALL keys with that prefix, sorted ascending.
 * - `signedUrl` returns a URL granting time-boxed access to the single key
 *   for the given method ("GET" to read, "PUT" to write). The in-memory
 *   backend returns an inert `memory://` URL with the same shape guarantees
 *   so app code can be exercised without a cloud bucket.
 *
 * Backends: in-memory (blob-store-memory.ts) and GCS (blob-store-gcs.ts).
 * Swap targets per the audit: S3, MinIO on SHC metal.
 */
import { Context, Schema } from "effect"
import type { Effect } from "effect"

/** Unrecoverable backend failure (network, auth, vendor error, ...). */
export class BlobStoreBackendError extends Schema.TaggedErrorClass<BlobStoreBackendError>()(
  "BlobStoreBackendError",
  {
    backend: Schema.String,
    operation: Schema.String,
    cause: Schema.Defect,
  },
) {}

export interface BlobPutOptions {
  readonly contentType?: string
}

export interface BlobSignedUrlOptions {
  /** How long the URL stays valid, in milliseconds. */
  readonly expiresInMs: number
  /** Access mode the URL grants. Default "GET". */
  readonly method?: "GET" | "PUT"
}

export interface BlobStoreShape {
  readonly put: (
    key: string,
    data: Uint8Array,
    options?: BlobPutOptions,
  ) => Effect.Effect<void, BlobStoreBackendError>
  /** `null` when the key is missing. */
  readonly get: (key: string) => Effect.Effect<Uint8Array | null, BlobStoreBackendError>
  /** Idempotent: deleting a missing key succeeds. */
  readonly delete: (key: string) => Effect.Effect<void, BlobStoreBackendError>
  /** All keys with the prefix, sorted ascending. */
  readonly list: (prefix: string) => Effect.Effect<ReadonlyArray<string>, BlobStoreBackendError>
  readonly signedUrl: (
    key: string,
    options: BlobSignedUrlOptions,
  ) => Effect.Effect<string, BlobStoreBackendError>
}

export class BlobStore extends Context.Service<BlobStore, BlobStoreShape>()(
  "@openagentsinc/oa-infra/BlobStore",
) {}

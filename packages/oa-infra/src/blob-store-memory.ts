/**
 * In-memory BlobStore backend — reference implementation and default test
 * Layer. `signedUrl` returns an inert `memory://` URL (correct shape, no
 * network authority) so app flows can be exercised without a cloud bucket.
 */
import { Effect, Layer } from "effect"
import { BlobStore, type BlobStoreShape } from "./blob-store.ts"

interface MemoryBlob {
  readonly data: Uint8Array
  readonly contentType: string | undefined
}

export const makeMemoryBlobStore = (): BlobStoreShape => {
  const blobs = new Map<string, MemoryBlob>()

  const put = (
    key: string,
    data: Uint8Array,
    options?: { readonly contentType?: string },
  ) =>
    Effect.sync(() => {
      blobs.set(key, { data: data.slice(), contentType: options?.contentType })
    })

  const get = (key: string) =>
    Effect.sync(() => {
      const blob = blobs.get(key)
      return blob === undefined ? null : blob.data.slice()
    })

  const del = (key: string) =>
    Effect.sync(() => {
      blobs.delete(key)
    })

  const list = (prefix: string) =>
    Effect.sync(() =>
      [...blobs.keys()].filter((key) => key.startsWith(prefix)).sort(),
    )

  const signedUrl = (
    key: string,
    options: { readonly expiresInMs: number; readonly method?: "GET" | "PUT" },
  ) =>
    Effect.sync(() => {
      const expiresAt = Date.now() + options.expiresInMs
      const method = options.method ?? "GET"
      return `memory://oa-infra-blob/${encodeURIComponent(key)}?method=${method}&expires=${expiresAt}`
    })

  return { put, get, delete: del, list, signedUrl }
}

export const layerMemory = (): Layer.Layer<BlobStore> =>
  Layer.sync(BlobStore, makeMemoryBlobStore)

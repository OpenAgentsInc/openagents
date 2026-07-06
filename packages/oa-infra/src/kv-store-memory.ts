/**
 * In-memory KvStore backend — the reference implementation of the KvStore
 * contract and the default test Layer.
 */
import { Effect, Layer } from "effect"
import { KvStore, type KvStoreShape } from "./kv-store.ts"

interface MemoryKvEntry {
  readonly value: string
  /** Epoch ms; undefined = never expires. */
  readonly expiresAtMs: number | undefined
}

export const makeMemoryKvStore = (): KvStoreShape => {
  const entries = new Map<string, MemoryKvEntry>()

  const get = (key: string) =>
    Effect.sync(() => {
      const entry = entries.get(key)
      if (entry === undefined) return null
      if (entry.expiresAtMs !== undefined && entry.expiresAtMs <= Date.now()) {
        entries.delete(key)
        return null
      }
      return entry.value
    })

  const put = (key: string, value: string, options?: { readonly ttlMs?: number }) =>
    Effect.sync(() => {
      const ttlMs = options?.ttlMs
      entries.set(key, {
        value,
        expiresAtMs: ttlMs === undefined ? undefined : Date.now() + ttlMs,
      })
    })

  const del = (key: string) =>
    Effect.sync(() => {
      entries.delete(key)
    })

  return { get, put, delete: del }
}

export const layerMemory = (): Layer.Layer<KvStore> =>
  Layer.sync(KvStore, makeMemoryKvStore)

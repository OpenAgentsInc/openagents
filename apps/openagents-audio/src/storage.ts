import { Effect } from "effect"
import { RetentionError } from "./model.js"

export interface PrivateObjectStore {
  readonly putIfAbsent: (objectRef: string, bytes: Uint8Array) => Effect.Effect<"created" | "exists", RetentionError>
  readonly get: (objectRef: string) => Effect.Effect<Uint8Array, RetentionError>
  readonly delete: (objectRef: string) => Effect.Effect<void, RetentionError>
  readonly listRefs: (prefix: string) => Effect.Effect<readonly string[], RetentionError>
}

export class MemoryPrivateObjectStore implements PrivateObjectStore {
  readonly objects = new Map<string, Uint8Array>()
  available = true
  putIfAbsent = (ref: string, bytes: Uint8Array) => Effect.try({
    try: () => {
      if (!this.available) throw new RetentionError("storage_unavailable", "object storage unavailable")
      if (this.objects.has(ref)) return "exists" as const
      this.objects.set(ref, bytes.slice())
      return "created" as const
    },
    catch: (error) => error instanceof RetentionError ? error : new RetentionError("storage_unavailable", "object put failed"),
  })
  get = (ref: string) => Effect.try({
    try: () => {
      if (!this.available) throw new RetentionError("storage_unavailable", "object storage unavailable")
      const value = this.objects.get(ref)
      if (!value) throw new RetentionError("not_found", "object not found")
      return value.slice()
    },
    catch: (error) => error instanceof RetentionError ? error : new RetentionError("storage_unavailable", "object read failed"),
  })
  delete = (ref: string) => Effect.sync(() => { this.objects.delete(ref) })
  listRefs = (prefix: string) => Effect.sync(() => [...this.objects.keys()].filter((ref) => ref.startsWith(prefix)).sort())
}

/** Google JSON API adapter. The bucket remains private; no signed/public URL is ever produced. */
export class GcsPrivateObjectStore implements PrivateObjectStore {
  constructor(readonly bucket: string, readonly accessToken: () => Promise<string>) {}
  private objectUrl(ref: string): string {
    return `https://storage.googleapis.com/storage/v1/b/${encodeURIComponent(this.bucket)}/o/${encodeURIComponent(ref)}`
  }
  private request = (url: string, init: RequestInit) => Effect.tryPromise({
    try: async () => fetch(url, { ...init, headers: { ...init.headers, authorization: `Bearer ${await this.accessToken()}` } }),
    catch: () => new RetentionError("storage_unavailable", "private object request failed"),
  })
  putIfAbsent = (ref: string, bytes: Uint8Array) => Effect.flatMap(this.request(
    `https://storage.googleapis.com/upload/storage/v1/b/${encodeURIComponent(this.bucket)}/o?uploadType=media&ifGenerationMatch=0&name=${encodeURIComponent(ref)}`,
    { method: "POST", headers: { "content-type": "application/octet-stream" }, body: new Blob([bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer]) },
  ), (response) => response.ok ? Effect.succeed("created" as const) : response.status === 412 ? Effect.succeed("exists" as const) : Effect.fail(new RetentionError("storage_unavailable", `object put failed (${response.status})`)))
  get = (ref: string) => Effect.flatMap(this.request(`${this.objectUrl(ref)}?alt=media`, {}), (response) => response.ok
    ? Effect.tryPromise({ try: async () => new Uint8Array(await response.arrayBuffer()), catch: () => new RetentionError("storage_unavailable", "object read failed") })
    : Effect.fail(new RetentionError(response.status === 404 ? "not_found" : "storage_unavailable", `object read failed (${response.status})`)))
  delete = (ref: string) => Effect.flatMap(this.request(this.objectUrl(ref), { method: "DELETE" }), (response) => response.ok || response.status === 404 ? Effect.void : Effect.fail(new RetentionError("storage_unavailable", `object delete failed (${response.status})`)))
  listRefs = (prefix: string) => Effect.flatMap(this.request(`https://storage.googleapis.com/storage/v1/b/${encodeURIComponent(this.bucket)}/o?prefix=${encodeURIComponent(prefix)}&fields=items(name),nextPageToken`, {}), (response) => response.ok
    ? Effect.tryPromise({ try: async () => ((await response.json()) as { items?: Array<{ name: string }> }).items?.map(({ name }) => name).sort() ?? [], catch: () => new RetentionError("storage_unavailable", "object list failed") })
    : Effect.fail(new RetentionError("storage_unavailable", `object list failed (${response.status})`)))
}

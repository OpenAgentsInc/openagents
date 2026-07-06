/**
 * GCS BlobStore backend (`@google-cloud/storage`).
 *
 * This is the ONLY file in oa-infra that imports a vendor SDK, and it is
 * reachable only through the BlobStore interface Layer — app code never
 * sees GCS types (audit §5 rule).
 *
 * Config (Effect Config, environment by default):
 * - `OA_INFRA_GCS_BUCKET`      (required) bucket name
 * - `OA_INFRA_GCS_PROJECT_ID`  (optional) GCP project override
 * - `OA_INFRA_GCS_KEY_FILE`    (optional) service-account key file path;
 *                              omitted = Application Default Credentials
 * - `OA_INFRA_GCS_PREFIX`      (optional) key prefix namespacing every
 *                              object this Layer touches
 */
import { Config, Effect, Layer, Option } from "effect"
import { Storage } from "@google-cloud/storage"
import type { Bucket } from "@google-cloud/storage"
import { BlobStore, BlobStoreBackendError, type BlobStoreShape } from "./blob-store.ts"

const BACKEND = "gcs"

const tryGcs = <A>(operation: string, run: () => Promise<A>) =>
  Effect.tryPromise({
    try: run,
    catch: (cause) => new BlobStoreBackendError({ backend: BACKEND, operation, cause }),
  })

export interface GcsBlobStoreOptions {
  readonly bucket: Bucket
  /** Prefix applied to every key (namespacing). Default "". */
  readonly prefix?: string
}

export const makeGcsBlobStore = (options: GcsBlobStoreOptions): BlobStoreShape => {
  const bucket = options.bucket
  const prefix = options.prefix ?? ""
  const objectName = (key: string) => `${prefix}${key}`

  const put = (
    key: string,
    data: Uint8Array,
    putOptions?: { readonly contentType?: string },
  ) =>
    tryGcs("put", async () => {
      const contentType = putOptions?.contentType
      await bucket.file(objectName(key)).save(Buffer.from(data), {
        resumable: false,
        ...(contentType === undefined ? {} : { contentType }),
      })
    })

  const get = (key: string) =>
    tryGcs("get", async (): Promise<Uint8Array | null> => {
      try {
        const [contents] = await bucket.file(objectName(key)).download()
        return new Uint8Array(contents)
      } catch (error) {
        if (isNotFound(error)) return null
        throw error
      }
    })

  const del = (key: string) =>
    tryGcs("delete", async () => {
      await bucket.file(objectName(key)).delete({ ignoreNotFound: true })
    })

  const list = (listPrefix: string) =>
    tryGcs("list", async () => {
      const [files] = await bucket.getFiles({ prefix: objectName(listPrefix) })
      return files
        .map((file) => (prefix === "" ? file.name : file.name.slice(prefix.length)))
        .sort()
    })

  const signedUrl = (
    key: string,
    urlOptions: { readonly expiresInMs: number; readonly method?: "GET" | "PUT" },
  ) =>
    tryGcs("signedUrl", async () => {
      const [url] = await bucket.file(objectName(key)).getSignedUrl({
        version: "v4",
        action: (urlOptions.method ?? "GET") === "PUT" ? "write" : "read",
        expires: Date.now() + urlOptions.expiresInMs,
      })
      return url
    })

  return { put, get, delete: del, list, signedUrl }
}

const isNotFound = (error: unknown): boolean =>
  typeof error === "object" &&
  error !== null &&
  "code" in error &&
  (error as { code: unknown }).code === 404

/** GCS BlobStore Layer configured from the environment (see file header). */
export const layerGcs: Layer.Layer<BlobStore, Config.ConfigError> = Layer.effect(
  BlobStore,
  Effect.gen(function* () {
    const bucketName = yield* Config.string("OA_INFRA_GCS_BUCKET")
    const projectId = yield* Config.option(Config.string("OA_INFRA_GCS_PROJECT_ID"))
    const keyFilename = yield* Config.option(Config.string("OA_INFRA_GCS_KEY_FILE"))
    const prefix = yield* Config.string("OA_INFRA_GCS_PREFIX").pipe(
      Config.withDefault(""),
    )
    const storage = new Storage({
      ...(Option.isSome(projectId) ? { projectId: projectId.value } : {}),
      ...(Option.isSome(keyFilename) ? { keyFilename: keyFilename.value } : {}),
    })
    return makeGcsBlobStore({ bucket: storage.bucket(bucketName), prefix })
  }),
)

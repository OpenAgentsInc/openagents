/**
 * GCS-backed `ARTIFACTS` bucket (CFG-8, issue #8523, epic #8515).
 *
 * The Cloudflare account-level R2 feature was disabled during the
 * Cloudflare→GCP consolidation, so the former `ARTIFACTS` R2 binding is
 * gone (#8516). This module restores artifact/blob persistence on Google
 * Cloud Storage while the app still runs on workerd (until CFG-9 #8524):
 * it adapts the owned `@openagentsinc/oa-infra` GCS HMAC client (pure
 * fetch + WebCrypto SigV4 over the XML/S3-interop API — no GCS SDK, which
 * cannot run on workerd) to the `R2Bucket` call surface the existing
 * consumers use (trace blobs, pylon-codex raw event chunks/archives,
 * thread files, image generation, site assets, packfile/trace archives).
 *
 * Implemented surface (everything the worker actually calls):
 * `head`, `get`, `put`, `delete`, `list`. Anything else (multipart,
 * `onlyIf` preconditions, ranged reads) rejects asynchronously with
 * `ArtifactsGcsUnsupportedOperationError` — same fail-soft shape as the
 * absent-binding stub, never a synchronous crash.
 *
 * Semantics preserved from R2 where consumers depend on them:
 * - `get`/`head` of a missing key resolve `null`.
 * - `put` accepts string | ArrayBuffer | ArrayBufferView | Blob |
 *   ReadableStream (streams are buffered for SigV4).
 * - `delete` is idempotent and accepts a key or an array of keys.
 * - Object results carry `body`, `arrayBuffer()`, `text()`, `json()`,
 *   `size`, `etag`/`httpEtag`, `uploaded`, `httpMetadata.contentType`,
 *   `customMetadata`, and `writeHttpMetadata(headers)`.
 * - CAVEAT vs R2: custom-metadata keys ride `x-amz-meta-*` headers and
 *   come back lowercased. No current consumer reads metadata keys back
 *   case-sensitively (they are write-only audit fields today).
 */
import {
  GcsHmacClient,
  gcsHmacMetadataFromHeaders,
  type GcsHmacClientOptions,
  type GcsHmacObjectMetadata,
} from '@openagentsinc/oa-infra/blob-store-gcs-hmac'

import { currentDate } from './runtime-primitives'

/** Typed rejection for R2Bucket surface the GCS adapter does not implement. */
export class ArtifactsGcsUnsupportedOperationError extends Error {
  override readonly name = 'ArtifactsGcsUnsupportedOperationError'
  readonly operation: string
  constructor(operation: string) {
    super(
      `GCS ARTIFACTS adapter does not implement R2Bucket.${operation} (#8523); ` +
        'migrate the caller to the oa-infra BlobStore interface instead.',
    )
    this.operation = operation
  }
}

export interface GcsArtifactsBucketOptions {
  readonly accessKeyId: string
  readonly secretAccessKey: string
  readonly bucket: string
  /** Default `https://storage.googleapis.com`. */
  readonly endpoint?: string | undefined
  /** Injection point for tests (same seam as the oa-infra client). */
  readonly fetch?: GcsHmacClientOptions['fetch'] | undefined
}

const writeHttpMetadataInto = (
  metadata: GcsHmacObjectMetadata,
  headers: Headers,
): void => {
  if (metadata.contentType !== undefined) {
    headers.set('content-type', metadata.contentType)
  }
  if (metadata.cacheControl !== undefined) {
    headers.set('cache-control', metadata.cacheControl)
  }
  if (metadata.contentEncoding !== undefined) {
    headers.set('content-encoding', metadata.contentEncoding)
  }
  if (metadata.contentLanguage !== undefined) {
    headers.set('content-language', metadata.contentLanguage)
  }
  if (metadata.contentDisposition !== undefined) {
    headers.set('content-disposition', metadata.contentDisposition)
  }
}

const r2HttpMetadataFrom = (
  metadata: GcsHmacObjectMetadata,
): R2HTTPMetadata => ({
  ...(metadata.contentType === undefined
    ? {}
    : { contentType: metadata.contentType }),
  ...(metadata.cacheControl === undefined
    ? {}
    : { cacheControl: metadata.cacheControl }),
  ...(metadata.contentEncoding === undefined
    ? {}
    : { contentEncoding: metadata.contentEncoding }),
  ...(metadata.contentLanguage === undefined
    ? {}
    : { contentLanguage: metadata.contentLanguage }),
  ...(metadata.contentDisposition === undefined
    ? {}
    : { contentDisposition: metadata.contentDisposition }),
})

const emptyChecksums: R2Checksums = {
  toJSON: () => ({}),
} as R2Checksums

const r2ObjectFrom = (key: string, metadata: GcsHmacObjectMetadata): R2Object =>
  ({
    checksums: emptyChecksums,
    customMetadata: metadata.customMetadata as Record<string, string>,
    etag: metadata.etag,
    httpEtag: metadata.httpEtag,
    httpMetadata: r2HttpMetadataFrom(metadata),
    key,
    size: metadata.size,
    storageClass: 'Standard',
    uploaded: metadata.uploaded,
    version: metadata.etag,
    writeHttpMetadata: (headers: Headers) =>
      writeHttpMetadataInto(metadata, headers),
  }) as R2Object

type GcsObjectResponse = NonNullable<
  Awaited<ReturnType<GcsHmacClient['getObject']>>
>

const r2ObjectBodyFrom = (
  key: string,
  response: GcsObjectResponse,
): R2ObjectBody => {
  const metadata = gcsHmacMetadataFromHeaders(response.headers)
  const base = r2ObjectFrom(key, metadata)
  const body =
    response.body ?? new Response(new Uint8Array(0)).body ?? new ReadableStream()
  return Object.assign(base, {
    arrayBuffer: () => response.arrayBuffer(),
    blob: () => response.blob(),
    body,
    get bodyUsed() {
      return response.bodyUsed
    },
    bytes: async () => new Uint8Array(await response.arrayBuffer()),
    json: <T>() => response.json() as Promise<T>,
    text: () => response.text(),
  }) as R2ObjectBody
}

type R2PutValue =
  | string
  | ArrayBuffer
  | ArrayBufferView
  | Blob
  | ReadableStream
  | null

const putValueToBytes = async (value: R2PutValue): Promise<Uint8Array> => {
  if (value === null) return new Uint8Array(0)
  if (typeof value === 'string') return new TextEncoder().encode(value)
  if (value instanceof ArrayBuffer) return new Uint8Array(value)
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength)
  }
  if (value instanceof Blob) return new Uint8Array(await value.arrayBuffer())
  // ReadableStream: SigV4 header auth needs the whole payload; buffer it.
  return new Uint8Array(await new Response(value).arrayBuffer())
}

/**
 * Build an `R2Bucket`-shaped object backed by GCS. Unimplemented members
 * reject asynchronously (Proxy fallback), mirroring the absent-binding
 * stub in `artifacts-binding.ts` so caller `.catch` handling keeps working.
 */
export const makeGcsArtifactsBucket = (
  options: GcsArtifactsBucketOptions,
): R2Bucket => {
  const client = new GcsHmacClient({
    accessKeyId: options.accessKeyId,
    bucket: options.bucket,
    secretAccessKey: options.secretAccessKey,
    ...(options.endpoint === undefined ? {} : { endpoint: options.endpoint }),
    ...(options.fetch === undefined ? {} : { fetch: options.fetch }),
  })

  const head = async (key: string): Promise<R2Object | null> => {
    const metadata = await client.headObject(key)
    return metadata === null ? null : r2ObjectFrom(key, metadata)
  }

  const get = async (key: string): Promise<R2ObjectBody | null> => {
    const response = await client.getObject(key)
    return response === null ? null : r2ObjectBodyFrom(key, response)
  }

  const put = async (
    key: string,
    value: R2PutValue,
    putOptions?: R2PutOptions,
  ): Promise<R2Object> => {
    const bytes = await putValueToBytes(value)
    const httpMetadata =
      putOptions?.httpMetadata instanceof Headers
        ? Object.fromEntries(putOptions.httpMetadata.entries())
        : (putOptions?.httpMetadata ?? {})
    const result = await client.putObject(key, bytes, {
      ...(httpMetadata.contentType === undefined
        ? {}
        : { contentType: httpMetadata.contentType }),
      ...(httpMetadata.cacheControl === undefined
        ? {}
        : { cacheControl: httpMetadata.cacheControl }),
      ...(httpMetadata.contentEncoding === undefined
        ? {}
        : { contentEncoding: httpMetadata.contentEncoding }),
      ...(httpMetadata.contentLanguage === undefined
        ? {}
        : { contentLanguage: httpMetadata.contentLanguage }),
      ...(httpMetadata.contentDisposition === undefined
        ? {}
        : { contentDisposition: httpMetadata.contentDisposition }),
      ...(putOptions?.customMetadata === undefined
        ? {}
        : { customMetadata: putOptions.customMetadata }),
    })
    return r2ObjectFrom(key, {
      cacheControl: httpMetadata.cacheControl,
      contentDisposition: httpMetadata.contentDisposition,
      contentEncoding: httpMetadata.contentEncoding,
      contentLanguage: httpMetadata.contentLanguage,
      contentType: httpMetadata.contentType,
      customMetadata: putOptions?.customMetadata ?? {},
      etag: result.etag,
      httpEtag: `"${result.etag}"`,
      size: result.size,
      uploaded: currentDate(),
    })
  }

  const del = async (keys: string | Array<string>): Promise<void> => {
    const list = typeof keys === 'string' ? [keys] : keys
    for (const key of list) {
      await client.deleteObject(key)
    }
  }

  const list = async (listOptions?: R2ListOptions): Promise<R2Objects> => {
    const page = await client.listPage(listOptions?.prefix ?? '', {
      ...(listOptions?.cursor === undefined
        ? {}
        : { cursor: listOptions.cursor }),
      ...(listOptions?.limit === undefined
        ? {}
        : { maxKeys: listOptions.limit }),
    })
    const objects = page.entries.map(entry =>
      r2ObjectFrom(entry.key, {
        cacheControl: undefined,
        contentDisposition: undefined,
        contentEncoding: undefined,
        contentLanguage: undefined,
        contentType: undefined,
        customMetadata: {},
        etag: entry.etag,
        httpEtag: `"${entry.etag}"`,
        size: entry.size,
        uploaded: entry.uploaded,
      }),
    )
    return (page.truncated && page.cursor !== undefined
      ? {
          cursor: page.cursor,
          delimitedPrefixes: [],
          objects,
          truncated: true,
        }
      : { delimitedPrefixes: [], objects, truncated: false }) as R2Objects
  }

  const implemented: Record<string, unknown> = {
    delete: del,
    get,
    head,
    list,
    put,
  }

  return new Proxy(implemented, {
    get: (target, property) => {
      if (typeof property !== 'string' || property === 'then') {
        return undefined
      }
      const member = target[property]
      if (member !== undefined) return member
      return (..._args: ReadonlyArray<unknown>) =>
        Promise.reject(new ArtifactsGcsUnsupportedOperationError(property))
    },
  }) as unknown as R2Bucket
}

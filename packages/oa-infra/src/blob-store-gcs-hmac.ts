/**
 * GCS BlobStore backend over the XML/S3-interoperability API with HMAC
 * credentials (CFG-8, issue #8523).
 *
 * Why this exists next to `blob-store-gcs.ts`: the `@google-cloud/storage`
 * SDK only runs on Node/Bun-class runtimes. This backend is pure
 * `fetch` + WebCrypto (AWS SigV4 via the tiny `aws4fetch` signer), so the
 * SAME Layer works on Cloudflare workerd today and on Cloud Run/Bun after
 * the CFG-9 runtime move — no vendor SDK, no bundling hazards.
 *
 * It passes the identical BlobStore conformance suite
 * (src/conformance/blob-store.ts) as every other backend.
 *
 * Interop notes (validated against a live bucket 2026-07-06):
 * - GCS accepts AWS SigV4 (`AWS4-HMAC-SHA256`) with service "s3" and an
 *   arbitrary region string ("auto" works); credentials are GCS HMAC keys
 *   bound to a service account (`gcloud storage hmac create`).
 * - Custom metadata MUST use `x-amz-meta-*` headers — GCS rejects requests
 *   mixing `x-goog-*` and `x-amz-*` headers, and the signer adds
 *   `X-Amz-Date`/`X-Amz-Content-Sha256`. Metadata keys round-trip
 *   lowercased (HTTP header semantics).
 * - `DELETE` of a missing object returns 404; treated as success
 *   (idempotent delete).
 * - Listing is the S3 V1 `?prefix=&marker=` XML shape.
 *
 * Config (Effect Config, environment by default; `layerGcsHmac`):
 * - `OA_INFRA_GCS_HMAC_ACCESS_KEY_ID` (required) HMAC access id
 * - `OA_INFRA_GCS_HMAC_SECRET`        (required) HMAC secret
 * - `OA_INFRA_GCS_BUCKET`             (required) bucket name (shared with
 *                                     the SDK-backed GCS Layer)
 * - `OA_INFRA_GCS_PREFIX`             (optional) key prefix namespacing
 * - `OA_INFRA_GCS_ENDPOINT`           (optional) default
 *                                     `https://storage.googleapis.com`
 * - `OA_INFRA_GCS_HMAC_REGION`        (optional) SigV4 region, default "auto"
 */
import { AwsClient } from "aws4fetch"
import { Config, Effect, Layer, Redacted } from "effect"
import { BlobStore, BlobStoreBackendError, type BlobStoreShape } from "./blob-store.ts"

const BACKEND = "gcs-hmac"

const DEFAULT_ENDPOINT = "https://storage.googleapis.com"
const DEFAULT_REGION = "auto"
const CUSTOM_METADATA_HEADER_PREFIX = "x-amz-meta-"

/** Typed failure for any non-OK GCS XML API response. */
export class GcsHmacError extends Error {
  override readonly name = "GcsHmacError"
  readonly operation: string
  readonly status: number
  readonly key: string | undefined
  constructor(options: {
    readonly operation: string
    readonly status: number
    readonly key?: string
    readonly bodySnippet?: string
  }) {
    super(
      `GCS XML API ${options.operation} failed with HTTP ${options.status}` +
        (options.key === undefined ? "" : ` for key ${JSON.stringify(options.key)}`) +
        (options.bodySnippet === undefined || options.bodySnippet === ""
          ? ""
          : `: ${options.bodySnippet}`),
    )
    this.operation = options.operation
    this.status = options.status
    this.key = options.key
  }
}

export interface GcsHmacClientOptions {
  readonly accessKeyId: string
  readonly secretAccessKey: string
  readonly bucket: string
  /** Default `https://storage.googleapis.com`. */
  readonly endpoint?: string
  /** SigV4 credential-scope region. GCS accepts any; default `auto`. */
  readonly region?: string
  /** Injection point for tests. Default: global fetch. */
  readonly fetch?: (request: Request) => Promise<Response>
}

export interface GcsHmacPutOptions {
  readonly contentType?: string
  readonly cacheControl?: string
  readonly contentEncoding?: string
  readonly contentLanguage?: string
  readonly contentDisposition?: string
  /**
   * Stored via `x-amz-meta-*` headers. Keys are case-INSENSITIVE and come
   * back lowercased (HTTP header semantics) — do not rely on key casing.
   */
  readonly customMetadata?: Readonly<Record<string, string>>
}

/** Header-derived metadata of a stored object (HEAD/GET responses). */
export interface GcsHmacObjectMetadata {
  readonly size: number
  /** ETag without surrounding quotes. */
  readonly etag: string
  /** ETag exactly as returned (usually quoted). */
  readonly httpEtag: string
  readonly uploaded: Date
  readonly contentType: string | undefined
  readonly cacheControl: string | undefined
  readonly contentEncoding: string | undefined
  readonly contentLanguage: string | undefined
  readonly contentDisposition: string | undefined
  readonly customMetadata: Readonly<Record<string, string>>
}

export interface GcsHmacListEntry {
  readonly key: string
  readonly size: number
  readonly etag: string
  readonly uploaded: Date
}

export interface GcsHmacListPage {
  readonly entries: ReadonlyArray<GcsHmacListEntry>
  readonly truncated: boolean
  /** Marker for the next page when truncated. */
  readonly cursor: string | undefined
}

export interface GcsHmacSignedUrlOptions {
  readonly expiresInMs: number
  /** Default "GET". */
  readonly method?: "GET" | "PUT"
}

/** Encode an object key for the URL path, preserving `/` separators. */
const encodeKeyPath = (key: string): string =>
  key.split("/").map(encodeURIComponent).join("/")

const XML_ENTITIES: Readonly<Record<string, string>> = {
  "&amp;": "&",
  "&apos;": "'",
  "&gt;": ">",
  "&lt;": "<",
  "&quot;": '"',
}

const decodeXmlText = (text: string): string =>
  text.replace(/&(?:amp|lt|gt|quot|apos|#x[0-9a-fA-F]+|#[0-9]+);/g, (entity) => {
    const named = XML_ENTITIES[entity]
    if (named !== undefined) return named
    const code = entity.startsWith("&#x")
      ? Number.parseInt(entity.slice(3, -1), 16)
      : Number.parseInt(entity.slice(2, -1), 10)
    return Number.isNaN(code) ? entity : String.fromCodePoint(code)
  })

const xmlTagText = (xml: string, tag: string): string | undefined => {
  const match = new RegExp(`<${tag}>([\\s\\S]*?)</${tag}>`).exec(xml)
  return match === undefined || match === null ? undefined : match[1]
}

/** Strip surrounding double quotes from an ETag value. */
const unquoteEtag = (etag: string): string => etag.replace(/^"|"$/g, "")

export const gcsHmacMetadataFromHeaders = (
  headers: Headers,
): GcsHmacObjectMetadata => {
  const customMetadata: Record<string, string> = {}
  headers.forEach((value, headerName) => {
    if (headerName.startsWith(CUSTOM_METADATA_HEADER_PREFIX)) {
      customMetadata[headerName.slice(CUSTOM_METADATA_HEADER_PREFIX.length)] = value
    }
  })
  const httpEtag = headers.get("etag") ?? ""
  const lastModified = headers.get("last-modified")
  const uploadedMs = lastModified === null ? Number.NaN : Date.parse(lastModified)
  return {
    cacheControl: headers.get("cache-control") ?? undefined,
    contentDisposition: headers.get("content-disposition") ?? undefined,
    contentEncoding: headers.get("content-encoding") ?? undefined,
    contentLanguage: headers.get("content-language") ?? undefined,
    contentType: headers.get("content-type") ?? undefined,
    customMetadata,
    etag: unquoteEtag(httpEtag),
    httpEtag,
    size: Number.parseInt(headers.get("content-length") ?? "0", 10) || 0,
    uploaded: Number.isNaN(uploadedMs) ? new Date(0) : new Date(uploadedMs),
  }
}

/**
 * Minimal signed-fetch client for one GCS bucket over the XML API.
 * Runtime-agnostic: `fetch` + WebCrypto only (workerd, Bun, Node 18+).
 */
export class GcsHmacClient {
  readonly bucket: string
  private readonly endpoint: string
  private readonly aws: AwsClient
  private readonly fetchImpl: (request: Request) => Promise<Response>

  constructor(options: GcsHmacClientOptions) {
    this.bucket = options.bucket
    this.endpoint = (options.endpoint ?? DEFAULT_ENDPOINT).replace(/\/+$/, "")
    this.aws = new AwsClient({
      accessKeyId: options.accessKeyId,
      region: options.region ?? DEFAULT_REGION,
      secretAccessKey: options.secretAccessKey,
      service: "s3",
    })
    this.fetchImpl = options.fetch ?? ((request) => fetch(request))
  }

  objectUrl(key: string): string {
    return `${this.endpoint}/${this.bucket}/${encodeKeyPath(key)}`
  }

  private async signedFetch(
    url: string,
    init: {
      readonly method?: string
      readonly body?: BodyInit
      readonly headers?: Readonly<Record<string, string>>
    } = {},
  ): Promise<Response> {
    const request = await this.aws.sign(url, {
      method: init.method ?? "GET",
      ...(init.body === undefined ? {} : { body: init.body }),
      ...(init.headers === undefined ? {} : { headers: init.headers }),
    })
    return this.fetchImpl(request)
  }

  private async failFrom(
    operation: string,
    response: Response,
    key?: string,
  ): Promise<never> {
    const body = await response.text().catch(() => "")
    throw new GcsHmacError({
      bodySnippet: body.slice(0, 300),
      ...(key === undefined ? {} : { key }),
      operation,
      status: response.status,
    })
  }

  async putObject(
    key: string,
    body: Uint8Array | string,
    options: GcsHmacPutOptions = {},
  ): Promise<{ readonly etag: string; readonly size: number }> {
    const headers: Record<string, string> = {}
    if (options.contentType !== undefined) headers["content-type"] = options.contentType
    if (options.cacheControl !== undefined) headers["cache-control"] = options.cacheControl
    if (options.contentEncoding !== undefined) {
      headers["content-encoding"] = options.contentEncoding
    }
    if (options.contentLanguage !== undefined) {
      headers["content-language"] = options.contentLanguage
    }
    if (options.contentDisposition !== undefined) {
      headers["content-disposition"] = options.contentDisposition
    }
    for (const [metaKey, metaValue] of Object.entries(options.customMetadata ?? {})) {
      headers[`${CUSTOM_METADATA_HEADER_PREFIX}${metaKey.toLowerCase()}`] = metaValue
    }
    const payload = typeof body === "string" ? new TextEncoder().encode(body) : body
    const response = await this.signedFetch(this.objectUrl(key), {
      // Pass an ArrayBuffer-backed copy so BodyInit typing is exact across
      // runtimes (workerd/Bun both accept BufferSource).
      body: payload.slice().buffer as ArrayBuffer,
      headers,
      method: "PUT",
    })
    if (!response.ok) return this.failFrom("put", response, key)
    await response.body?.cancel()
    return {
      etag: unquoteEtag(response.headers.get("etag") ?? ""),
      size: payload.byteLength,
    }
  }

  /** `null` when the object is missing. Caller owns the Response body. */
  async getObject(key: string): Promise<Response | null> {
    const response = await this.signedFetch(this.objectUrl(key))
    if (response.status === 404) {
      await response.body?.cancel()
      return null
    }
    if (!response.ok) return this.failFrom("get", response, key)
    return response
  }

  /** `null` when the object is missing. */
  async headObject(key: string): Promise<GcsHmacObjectMetadata | null> {
    const response = await this.signedFetch(this.objectUrl(key), {
      method: "HEAD",
    })
    await response.body?.cancel()
    if (response.status === 404) return null
    if (!response.ok) {
      throw new GcsHmacError({
        key,
        operation: "head",
        status: response.status,
      })
    }
    return gcsHmacMetadataFromHeaders(response.headers)
  }

  /** Idempotent: deleting a missing object succeeds (GCS returns 404). */
  async deleteObject(key: string): Promise<void> {
    const response = await this.signedFetch(this.objectUrl(key), {
      method: "DELETE",
    })
    await response.body?.cancel()
    if (response.ok || response.status === 404) return
    return this.failFrom("delete", response, key)
  }

  async listPage(
    prefix: string,
    options: { readonly cursor?: string; readonly maxKeys?: number } = {},
  ): Promise<GcsHmacListPage> {
    const url = new URL(`${this.endpoint}/${this.bucket}/`)
    url.searchParams.set("prefix", prefix)
    if (options.cursor !== undefined) url.searchParams.set("marker", options.cursor)
    if (options.maxKeys !== undefined) {
      url.searchParams.set("max-keys", String(options.maxKeys))
    }
    const response = await this.signedFetch(url.toString())
    if (!response.ok) return this.failFrom("list", response)
    const xml = await response.text()
    const entries: Array<GcsHmacListEntry> = []
    for (const match of xml.matchAll(/<Contents>([\s\S]*?)<\/Contents>/g)) {
      const contents = match[1] ?? ""
      const rawKey = xmlTagText(contents, "Key")
      if (rawKey === undefined) continue
      const uploadedMs = Date.parse(xmlTagText(contents, "LastModified") ?? "")
      entries.push({
        etag: unquoteEtag(decodeXmlText(xmlTagText(contents, "ETag") ?? "")),
        key: decodeXmlText(rawKey),
        size: Number.parseInt(xmlTagText(contents, "Size") ?? "0", 10) || 0,
        uploaded: Number.isNaN(uploadedMs) ? new Date(0) : new Date(uploadedMs),
      })
    }
    const truncated = xmlTagText(xml, "IsTruncated") === "true"
    const nextMarker = xmlTagText(xml, "NextMarker")
    const lastEntry = entries[entries.length - 1]
    const cursor = truncated
      ? (nextMarker === undefined ? lastEntry?.key : decodeXmlText(nextMarker))
      : undefined
    return { cursor, entries, truncated }
  }

  /** All keys under the prefix, ascending (paginates internally). */
  async listAllKeys(prefix: string): Promise<Array<string>> {
    const keys: Array<string> = []
    let cursor: string | undefined = undefined
    for (;;) {
      const page: GcsHmacListPage = await this.listPage(prefix, {
        ...(cursor === undefined ? {} : { cursor }),
      })
      for (const entry of page.entries) keys.push(entry.key)
      if (!page.truncated || page.cursor === undefined) break
      cursor = page.cursor
    }
    return keys.sort()
  }

  /** V4 presigned URL (query-string auth) for a single object. */
  async signedUrl(key: string, options: GcsHmacSignedUrlOptions): Promise<string> {
    const url = new URL(this.objectUrl(key))
    url.searchParams.set(
      "X-Amz-Expires",
      String(Math.max(1, Math.floor(options.expiresInMs / 1000))),
    )
    const signed = await this.aws.sign(new Request(url, { method: options.method ?? "GET" }), {
      aws: { signQuery: true },
    })
    return signed.url
  }
}

export interface GcsHmacBlobStoreOptions extends GcsHmacClientOptions {
  /** Prefix applied to every key (namespacing). Default "". */
  readonly prefix?: string
}

export const makeGcsHmacBlobStore = (options: GcsHmacBlobStoreOptions): BlobStoreShape => {
  const client = new GcsHmacClient(options)
  const prefix = options.prefix ?? ""
  const objectName = (key: string) => `${prefix}${key}`

  const tryGcs = <A>(operation: string, run: () => Promise<A>) =>
    Effect.tryPromise({
      catch: (cause) => new BlobStoreBackendError({ backend: BACKEND, cause, operation }),
      try: run,
    })

  return {
    delete: (key) => tryGcs("delete", () => client.deleteObject(objectName(key))),
    get: (key) =>
      tryGcs("get", async (): Promise<Uint8Array | null> => {
        const response = await client.getObject(objectName(key))
        if (response === null) return null
        return new Uint8Array(await response.arrayBuffer())
      }),
    list: (listPrefix) =>
      tryGcs("list", async () => {
        const keys = await client.listAllKeys(objectName(listPrefix))
        return keys.map((key) => (prefix === "" ? key : key.slice(prefix.length)))
      }),
    put: (key, data, putOptions) =>
      tryGcs("put", async () => {
        await client.putObject(objectName(key), data, {
          ...(putOptions?.contentType === undefined
            ? {}
            : { contentType: putOptions.contentType }),
        })
      }),
    signedUrl: (key, urlOptions) =>
      tryGcs("signedUrl", () =>
        client.signedUrl(objectName(key), {
          expiresInMs: urlOptions.expiresInMs,
          method: urlOptions.method ?? "GET",
        }),
      ),
  }
}

/** HMAC-interop GCS BlobStore Layer configured from the environment. */
export const layerGcsHmac: Layer.Layer<BlobStore, Config.ConfigError> = Layer.effect(
  BlobStore,
  Effect.gen(function* () {
    const accessKeyId = yield* Config.string("OA_INFRA_GCS_HMAC_ACCESS_KEY_ID")
    const secretAccessKey = yield* Config.redacted("OA_INFRA_GCS_HMAC_SECRET")
    const bucket = yield* Config.string("OA_INFRA_GCS_BUCKET")
    const prefix = yield* Config.string("OA_INFRA_GCS_PREFIX").pipe(Config.withDefault(""))
    const endpoint = yield* Config.string("OA_INFRA_GCS_ENDPOINT").pipe(
      Config.withDefault(DEFAULT_ENDPOINT),
    )
    const region = yield* Config.string("OA_INFRA_GCS_HMAC_REGION").pipe(
      Config.withDefault(DEFAULT_REGION),
    )
    return makeGcsHmacBlobStore({
      accessKeyId,
      bucket,
      endpoint,
      prefix,
      region,
      secretAccessKey: Redacted.value(secretAccessKey),
    })
  }),
)

/**
 * GCS HMAC-interop BlobStore backend tests (CFG-8, issue #8523).
 *
 * Two tiers:
 * 1. Offline unit tests against an injected fetch — request shape (SigV4
 *    headers, key encoding, metadata headers), XML list parsing (entities,
 *    pagination), and idempotent-delete/404 semantics. Always run.
 * 2. The shared BlobStore conformance suite against a REAL bucket, gated on
 *    `OA_INFRA_GCS_TEST_BUCKET` + `OA_INFRA_GCS_HMAC_ACCESS_KEY_ID` +
 *    `OA_INFRA_GCS_HMAC_SECRET` (same pattern as the SDK-backed GCS suite
 *    in postgres-backends.test.ts). Skips cleanly without credentials.
 */
import { describe, expect, test } from "bun:test"
import { Layer } from "effect"
import { BlobStore } from "./blob-store.ts"
import {
  GcsHmacClient,
  gcsHmacMetadataFromHeaders,
  makeGcsHmacBlobStore,
} from "./blob-store-gcs-hmac.ts"
import { runBlobStoreConformance } from "./conformance/blob-store.ts"

// ---------------------------------------------------------------------------
// Tier 1: offline unit tests (injected fetch)
// ---------------------------------------------------------------------------

interface RecordedRequest {
  readonly method: string
  readonly url: URL
  readonly headers: Headers
  readonly body: Uint8Array | null
}

const makeRecordingFetch = (
  respond: (request: RecordedRequest) => Response,
): {
  readonly requests: Array<RecordedRequest>
  readonly fetch: (request: Request) => Promise<Response>
} => {
  const requests: Array<RecordedRequest> = []
  return {
    fetch: async (request: Request) => {
      const body =
        request.body === null ? null : new Uint8Array(await request.arrayBuffer())
      const recorded: RecordedRequest = {
        body,
        headers: request.headers,
        method: request.method,
        url: new URL(request.url),
      }
      requests.push(recorded)
      return respond(recorded)
    },
    requests,
  }
}

const makeClient = (fetchImpl: (request: Request) => Promise<Response>) =>
  new GcsHmacClient({
    accessKeyId: "GOOG1ETEST",
    bucket: "test-bucket",
    fetch: fetchImpl,
    secretAccessKey: "test-secret",
  })

describe("GcsHmacClient (offline)", () => {
  test("putObject signs with SigV4, sends metadata as x-amz-meta-*, encodes the key path", async () => {
    const { fetch: fetchImpl, requests } = makeRecordingFetch(() =>
      new Response(null, { headers: { etag: '"abc123"' }, status: 200 }),
    )
    const client = makeClient(fetchImpl)
    const result = await client.putObject(
      "dir with space/a+b/file.json",
      JSON.stringify({ ok: true }),
      {
        cacheControl: "private, max-age=60",
        contentType: "application/json; charset=utf-8",
        customMetadata: { ownerUserId: "user-1" },
      },
    )
    expect(result.etag).toBe("abc123")
    expect(result.size).toBe(11)
    const request = requests[0]!
    expect(request.method).toBe("PUT")
    expect(request.url.pathname).toBe(
      "/test-bucket/dir%20with%20space/a%2Bb/file.json",
    )
    expect(request.headers.get("authorization")).toStartWith("AWS4-HMAC-SHA256")
    expect(request.headers.get("x-amz-date")).not.toBeNull()
    expect(request.headers.get("content-type")).toBe(
      "application/json; charset=utf-8",
    )
    expect(request.headers.get("cache-control")).toBe("private, max-age=60")
    // Metadata keys are lowercased header names.
    expect(request.headers.get("x-amz-meta-owneruserid")).toBe("user-1")
    // No x-goog-* request headers: GCS rejects mixed x-amz/x-goog requests.
    for (const headerName of request.headers.keys()) {
      expect(headerName.startsWith("x-goog-")).toBe(false)
    }
  })

  test("getObject returns null on 404 and the Response on 200", async () => {
    const { fetch: fetchImpl } = makeRecordingFetch((request) =>
      request.url.pathname.endsWith("missing")
        ? new Response("<Error/>", { status: 404 })
        : new Response("payload", { status: 200 }),
    )
    const client = makeClient(fetchImpl)
    expect(await client.getObject("some/missing")).toBeNull()
    const response = await client.getObject("some/present")
    expect(response).not.toBeNull()
    expect(await response!.text()).toBe("payload")
  })

  test("headObject parses size, etags, uploaded, contentType, and custom metadata", async () => {
    const { fetch: fetchImpl } = makeRecordingFetch(() =>
      new Response(null, {
        headers: {
          "cache-control": "no-store",
          "content-length": "42",
          "content-type": "text/plain",
          etag: '"deadbeef"',
          "last-modified": "Mon, 06 Jul 2026 17:11:08 GMT",
          "x-amz-meta-turnindex": "3",
        },
        status: 200,
      }),
    )
    const metadata = await makeClient(fetchImpl).headObject("k")
    expect(metadata).not.toBeNull()
    expect(metadata!.size).toBe(42)
    expect(metadata!.etag).toBe("deadbeef")
    expect(metadata!.httpEtag).toBe('"deadbeef"')
    expect(metadata!.contentType).toBe("text/plain")
    expect(metadata!.cacheControl).toBe("no-store")
    expect(metadata!.customMetadata).toEqual({ turnindex: "3" })
    expect(metadata!.uploaded.toISOString()).toBe("2026-07-06T17:11:08.000Z")
  })

  test("deleteObject treats 404 as success and rejects other failures", async () => {
    const { fetch: fetchImpl } = makeRecordingFetch((request) =>
      request.url.pathname.endsWith("forbidden")
        ? new Response("<Error/>", { status: 403 })
        : new Response(null, { status: 404 }),
    )
    const client = makeClient(fetchImpl)
    await client.deleteObject("gone-already")
    await expect(client.deleteObject("forbidden")).rejects.toThrow(
      /delete failed with HTTP 403/,
    )
  })

  test("listPage parses V1 XML incl. escaped keys; listAllKeys paginates with marker", async () => {
    const page1 = `<?xml version='1.0' encoding='UTF-8'?><ListBucketResult>
      <IsTruncated>true</IsTruncated>
      <Contents><Key>p/a&amp;b.json</Key><LastModified>2026-07-06T17:11:08.894Z</LastModified><ETag>"e1"</ETag><Size>11</Size></Contents>
      <Contents><Key>p/second</Key><LastModified>2026-07-06T17:11:09.000Z</LastModified><ETag>"e2"</ETag><Size>7</Size></Contents>
    </ListBucketResult>`
    const page2 = `<?xml version='1.0' encoding='UTF-8'?><ListBucketResult>
      <IsTruncated>false</IsTruncated>
      <Contents><Key>p/third</Key><LastModified>2026-07-06T17:11:10.000Z</LastModified><ETag>"e3"</ETag><Size>1</Size></Contents>
    </ListBucketResult>`
    const { fetch: fetchImpl, requests } = makeRecordingFetch((request) =>
      new Response(request.url.searchParams.has("marker") ? page2 : page1, {
        status: 200,
      }),
    )
    const client = makeClient(fetchImpl)
    const keys = await client.listAllKeys("p/")
    expect(keys).toEqual(["p/a&b.json", "p/second", "p/third"])
    expect(requests[0]!.url.searchParams.get("prefix")).toBe("p/")
    // Second page used the last key of page 1 as the marker (no NextMarker).
    expect(requests[1]!.url.searchParams.get("marker")).toBe("p/second")
  })

  test("signedUrl produces query-string SigV4 auth with the requested expiry", async () => {
    const client = makeClient(async () => new Response(null, { status: 200 }))
    const url = new URL(await client.signedUrl("a/b.txt", { expiresInMs: 300_000 }))
    expect(url.pathname).toBe("/test-bucket/a/b.txt")
    expect(url.searchParams.get("X-Amz-Expires")).toBe("300")
    expect(url.searchParams.get("X-Amz-Algorithm")).toBe("AWS4-HMAC-SHA256")
    expect(url.searchParams.get("X-Amz-Signature")).not.toBeNull()
  })

  test("gcsHmacMetadataFromHeaders tolerates absent optional headers", () => {
    const metadata = gcsHmacMetadataFromHeaders(new Headers({ etag: '"x"' }))
    expect(metadata.size).toBe(0)
    expect(metadata.contentType).toBeUndefined()
    expect(metadata.customMetadata).toEqual({})
  })

  test("BlobStoreShape put surfaces backend failures as BlobStoreBackendError", async () => {
    const store = makeGcsHmacBlobStore({
      accessKeyId: "GOOG1ETEST",
      bucket: "test-bucket",
      fetch: async () => new Response("<Error/>", { status: 500 }),
      secretAccessKey: "test-secret",
    })
    const { Effect } = await import("effect")
    const exit = await Effect.runPromiseExit(
      store.put("k", new TextEncoder().encode("v")),
    )
    expect(exit._tag).toBe("Failure")
  })
})

// ---------------------------------------------------------------------------
// Tier 2: conformance against a real bucket (env-gated)
// ---------------------------------------------------------------------------

const testBucket = process.env["OA_INFRA_GCS_TEST_BUCKET"]
const hmacAccessKeyId = process.env["OA_INFRA_GCS_HMAC_ACCESS_KEY_ID"]
const hmacSecret = process.env["OA_INFRA_GCS_HMAC_SECRET"]
const haveRealBucket =
  testBucket !== undefined &&
  testBucket !== "" &&
  hmacAccessKeyId !== undefined &&
  hmacAccessKeyId !== "" &&
  hmacSecret !== undefined &&
  hmacSecret !== ""

runBlobStoreConformance({
  label: "gcs-hmac",
  makeLayer: () =>
    Layer.succeed(
      BlobStore,
      makeGcsHmacBlobStore({
        accessKeyId: hmacAccessKeyId as string,
        bucket: testBucket as string,
        prefix: `oa-infra-conformance/${crypto.randomUUID()}/`,
        secretAccessKey: hmacSecret as string,
      }),
    ),
  skip: !haveRealBucket,
})

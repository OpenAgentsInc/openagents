import { describe, expect, it } from 'vitest'

import {
  ArtifactsGcsUnsupportedOperationError,
  makeGcsArtifactsBucket,
} from './gcs-artifacts-bucket'

interface Recorded {
  readonly method: string
  readonly url: URL
  readonly headers: Headers
  readonly body: Uint8Array | null
}

const recordingBucket = (respond: (recorded: Recorded) => Response) => {
  const requests: Array<Recorded> = []
  const bucket = makeGcsArtifactsBucket({
    accessKeyId: 'GOOG1ETEST',
    bucket: 'artifacts-test',
    fetch: async request => {
      const body =
        request.body === null
          ? null
          : new Uint8Array(await request.arrayBuffer())
      const recorded: Recorded = {
        body,
        headers: request.headers,
        method: request.method,
        url: new URL(request.url),
      }
      requests.push(recorded)
      return respond(recorded)
    },
    secretAccessKey: 'secret',
  })
  return { bucket, requests }
}

describe('makeGcsArtifactsBucket', () => {
  it('put sends SigV4-signed PUT with contentType and x-amz-meta custom metadata', async () => {
    const { bucket, requests } = recordingBucket(
      () => new Response(null, { headers: { etag: '"e1"' }, status: 200 }),
    )
    const object = await bucket.put('trace-blobs/uuid/file.json', '{"a":1}', {
      customMetadata: { ownerUserId: 'u1' },
      httpMetadata: { contentType: 'application/json; charset=utf-8' },
    })
    expect(object.etag).toBe('e1')
    expect(object.size).toBe(7)
    const request = requests[0]!
    expect(request.method).toBe('PUT')
    expect(request.url.pathname).toBe('/artifacts-test/trace-blobs/uuid/file.json')
    expect(request.headers.get('authorization')).toMatch(/^AWS4-HMAC-SHA256/)
    expect(request.headers.get('content-type')).toBe(
      'application/json; charset=utf-8',
    )
    expect(request.headers.get('x-amz-meta-owneruserid')).toBe('u1')
    expect(new TextDecoder().decode(request.body!)).toBe('{"a":1}')
  })

  it('put buffers ArrayBuffer, typed-array view, and Blob values', async () => {
    const { bucket, requests } = recordingBucket(
      () => new Response(null, { headers: { etag: '"e"' }, status: 200 }),
    )
    const bytes = new TextEncoder().encode('binary-value')
    await bucket.put('k/a', bytes.buffer as ArrayBuffer)
    await bucket.put('k/b', bytes.subarray(0, 6))
    await bucket.put('k/c', new Blob([bytes]))
    expect(new TextDecoder().decode(requests[0]!.body!)).toBe('binary-value')
    expect(new TextDecoder().decode(requests[1]!.body!)).toBe('binary')
    expect(new TextDecoder().decode(requests[2]!.body!)).toBe('binary-value')
  })

  it('get resolves null on 404 and a body-bearing object on 200', async () => {
    const { bucket } = recordingBucket(recorded =>
      recorded.url.pathname.endsWith('missing')
        ? new Response('<Error/>', { status: 404 })
        : new Response('payload-bytes', {
            headers: {
              'content-length': '13',
              'content-type': 'text/plain',
              etag: '"abc"',
              'last-modified': 'Mon, 06 Jul 2026 17:11:08 GMT',
              'x-amz-meta-turnindex': '2',
            },
            status: 200,
          }),
    )
    expect(await bucket.get('chunks/missing')).toBeNull()
    const object = await bucket.get('chunks/present')
    expect(object).not.toBeNull()
    expect(object!.size).toBe(13)
    expect(object!.etag).toBe('abc')
    expect(object!.httpEtag).toBe('"abc"')
    expect(object!.httpMetadata?.contentType).toBe('text/plain')
    expect(object!.customMetadata).toEqual({ turnindex: '2' })
    const headers = new Headers()
    object!.writeHttpMetadata(headers)
    expect(headers.get('content-type')).toBe('text/plain')
    expect(object!.body).toBeInstanceOf(ReadableStream)
    expect(await object!.text()).toBe('payload-bytes')
  })

  it('head resolves null on 404 and metadata (no body) on 200', async () => {
    const { bucket, requests } = recordingBucket(recorded =>
      recorded.url.pathname.endsWith('missing')
        ? new Response(null, { status: 404 })
        : new Response(null, {
            headers: { 'content-length': '9', etag: '"h"' },
            status: 200,
          }),
    )
    expect(await bucket.head('raw/missing')).toBeNull()
    const object = await bucket.head('raw/present')
    expect(object!.size).toBe(9)
    expect(requests.every(request => request.method === 'HEAD')).toBe(true)
  })

  it('delete tolerates 404 (idempotent) and accepts arrays of keys', async () => {
    const { bucket, requests } = recordingBucket(
      () => new Response(null, { status: 404 }),
    )
    await bucket.delete('gone')
    await bucket.delete(['a', 'b'])
    expect(requests).toHaveLength(3)
    expect(requests.every(request => request.method === 'DELETE')).toBe(true)
  })

  it('list maps the XML page to R2Objects with cursor pagination fields', async () => {
    const xml = `<?xml version='1.0' encoding='UTF-8'?><ListBucketResult>
      <IsTruncated>true</IsTruncated>
      <Contents><Key>p/one</Key><LastModified>2026-07-06T17:11:08.894Z</LastModified><ETag>"e1"</ETag><Size>3</Size></Contents>
    </ListBucketResult>`
    const { bucket, requests } = recordingBucket(
      () => new Response(xml, { status: 200 }),
    )
    const listed = await bucket.list({ limit: 1, prefix: 'p/' })
    expect(listed.objects.map(object => object.key)).toEqual(['p/one'])
    expect(listed.truncated).toBe(true)
    expect(listed.truncated && listed.cursor).toBe('p/one')
    expect(requests[0]!.url.searchParams.get('prefix')).toBe('p/')
    expect(requests[0]!.url.searchParams.get('max-keys')).toBe('1')
  })

  it('rejects unimplemented R2 surface asynchronously with the typed error', async () => {
    const { bucket } = recordingBucket(() => new Response(null, { status: 200 }))
    await expect(
      bucket.createMultipartUpload('big-object'),
    ).rejects.toBeInstanceOf(ArtifactsGcsUnsupportedOperationError)
    // Not thenable: awaiting the bucket must not hang.
    expect(await Promise.resolve(bucket)).toBe(bucket)
  })

  it('surfaces non-404 backend failures as rejections (fail-soft catch shape)', async () => {
    const { bucket } = recordingBucket(
      () => new Response('<Error/>', { status: 500 }),
    )
    const fallback = await bucket.get('any/key').catch(() => 'caught')
    expect(fallback).toBe('caught')
  })
})

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PUBLIC_REPLAY_CLIP_JOBS_PROJECTION_CONTRACT,
  PUBLIC_REPLAY_CLIP_JOBS_ROUTE,
  handlePublicReplayClipJobReadApi,
  handlePublicReplayClipJobsApi,
} from './replay-clip-job-routes'
import { makeInMemoryReplayClipJobStore } from './replay-clip-jobs'

const NOW_ISO = '2026-06-18T18:00:00.000Z'

type JsonValue =
  | string
  | number
  | boolean
  | null
  | Array<JsonValue>
  | { [key: string]: JsonValue }

const obj = (
  value: JsonValue | undefined,
): Record<string, JsonValue> => {
  if (
    value !== undefined &&
    typeof value === 'object' &&
    value !== null &&
    !Array.isArray(value)
  ) {
    return value
  }
  throw new Error('Expected a JSON object')
}

const readJsonObject = async (
  response: Response,
): Promise<Record<string, JsonValue>> => {
  const value: JsonValue = await response.json()
  return obj(value)
}

const arr = (value: JsonValue | undefined): Array<JsonValue> => {
  if (value !== undefined && Array.isArray(value)) {
    return value
  }
  throw new Error('Expected a JSON array')
}

const validRequestBody = {
  schemaVersion: 'openagents.replay_clip_job.v1',
  source: { kind: 'replay_bundle', bundleRef: 'first-real-settlement' },
  render: {
    startSecond: 20,
    durationSecond: 5,
    fps: 12,
    width: 1280,
    height: 720,
    outputKind: 'mp4',
  },
  cameraPath: {
    schemaVersion: 'openagents.replay_camera_path.v1',
    keyframes: [
      { second: 0, verb: 'hold' },
      { second: 2, verb: 'orbit', fov: 55 },
      { second: 4, verb: 'frame_settlement' },
    ],
  },
  sourceRefs: [
    'https://openagents.com/api/public/tassadar-replays/first-real-settlement',
  ],
}

const postRequest = (body: unknown) =>
  new Request(`https://openagents.com${PUBLIC_REPLAY_CLIP_JOBS_ROUTE}`, {
    method: 'POST',
    body: typeof body === 'string' ? body : JSON.stringify(body),
  })

const getRequest = () =>
  new Request(`https://openagents.com${PUBLIC_REPLAY_CLIP_JOBS_ROUTE}`, {
    method: 'GET',
  })

const run = (effect: Effect.Effect<Response, never, never>) =>
  Effect.runPromise(effect)

describe('public replay clip job API (#5432)', () => {
  test('POST creates a queued job and returns a staleness-declared envelope', async () => {
    const store = makeInMemoryReplayClipJobStore()
    const response = await run(
      handlePublicReplayClipJobsApi(postRequest(validRequestBody), {
        store,
        nowIso: () => NOW_ISO,
        newJobRef: () => 'fixed-0001',
      }),
    )
    expect(response.status).toBe(201)
    const body = await readJsonObject(response)
    expect(body['contractVersion']).toBe(
      PUBLIC_REPLAY_CLIP_JOBS_PROJECTION_CONTRACT,
    )
    expect(body['generatedAt']).toBe(NOW_ISO)
    const staleness = obj(body['staleness'])
    expect(staleness['contractVersion']).toBe('projection_staleness.v1')
    expect(staleness['composition']).toBe('live_at_read')
    expect(body['publicSafe']).toBe(true)
    const job = obj(body['job'])
    expect(job['status']).toBe('queued')
    expect(job['jobRef']).toBe('replay_clip_job.fixed-0001')
    expect(job['claimScope']).toBe('evidence_presentation_only')
    expect(job['manifestRef']).toBeNull()
    expect(arr(job['caveatRefs']).length).toBeGreaterThan(0)
  })

  test('POST rejects invalid JSON with 400', async () => {
    const response = await run(
      handlePublicReplayClipJobsApi(postRequest('{not json'), {
        store: makeInMemoryReplayClipJobStore(),
        nowIso: () => NOW_ISO,
      }),
    )
    expect(response.status).toBe(400)
    const body = await readJsonObject(response)
    expect(body['error']).toBe('replay_clip_job_invalid_json')
  })

  test('POST fails closed on an out-of-bounds render spec', async () => {
    const response = await run(
      handlePublicReplayClipJobsApi(
        postRequest({
          ...validRequestBody,
          render: { ...validRequestBody.render, durationSecond: 0 },
        }),
        { store: makeInMemoryReplayClipJobStore(), nowIso: () => NOW_ISO },
      ),
    )
    expect(response.status).toBe(400)
    const body = await readJsonObject(response)
    expect(body['error']).toBe('replay_clip_job_invalid_request')
  })

  test('POST fails closed on raw/private material in source refs', async () => {
    const response = await run(
      handlePublicReplayClipJobsApi(
        postRequest({
          ...validRequestBody,
          sourceRefs: ['/Users/chris/secret/trace.json'],
        }),
        { store: makeInMemoryReplayClipJobStore(), nowIso: () => NOW_ISO },
      ),
    )
    expect(response.status).toBe(400)
  })

  test('GET lists recent jobs through the store', async () => {
    const store = makeInMemoryReplayClipJobStore()
    await run(
      handlePublicReplayClipJobsApi(postRequest(validRequestBody), {
        store,
        nowIso: () => NOW_ISO,
        newJobRef: () => 'list-0001',
      }),
    )
    const response = await run(
      handlePublicReplayClipJobsApi(getRequest(), {
        store,
        nowIso: () => NOW_ISO,
      }),
    )
    expect(response.status).toBe(200)
    const body = await readJsonObject(response)
    expect(body['kind']).toBe('replay_clip_jobs')
    const jobs = arr(body['jobs'])
    expect(jobs.length).toBe(1)
    expect(obj(jobs[0]!)['jobRef']).toBe('replay_clip_job.list-0001')
  })

  test('read route returns a created job and 404 for unknown', async () => {
    const store = makeInMemoryReplayClipJobStore()
    await run(
      handlePublicReplayClipJobsApi(postRequest(validRequestBody), {
        store,
        nowIso: () => NOW_ISO,
        newJobRef: () => 'read-0001',
      }),
    )

    const found = await run(
      handlePublicReplayClipJobReadApi(
        getRequest(),
        'replay_clip_job.read-0001',
        { store, nowIso: () => NOW_ISO },
      ),
    )
    expect(found.status).toBe(200)
    const foundBody = await readJsonObject(found)
    expect(obj(foundBody['job'])['jobRef']).toBe('replay_clip_job.read-0001')

    const missing = await run(
      handlePublicReplayClipJobReadApi(getRequest(), 'replay_clip_job.nope', {
        store,
        nowIso: () => NOW_ISO,
      }),
    )
    expect(missing.status).toBe(404)
    const missingBody = await readJsonObject(missing)
    expect(missingBody['error']).toBe('replay_clip_job_not_found')
  })

  test('POST rejects unsupported methods on the read route', async () => {
    const response = await run(
      handlePublicReplayClipJobReadApi(
        new Request('https://openagents.com/api/public/replay-clips/x', {
          method: 'DELETE',
        }),
        'x',
        { store: makeInMemoryReplayClipJobStore(), nowIso: () => NOW_ISO },
      ),
    )
    expect(response.status).toBe(405)
  })

  test('no Worker code renders frames or references native binaries', async () => {
    // The route + store modules must not import Playwright/ffmpeg/child_process.
    // This is a guard: rendering belongs on the render box (#5431), never the
    // Worker.
    const { readFileSync } = await import('node:fs')
    const { fileURLToPath } = await import('node:url')
    const nativeImportPattern =
      /(import|require)[^\n]*(playwright|node:child_process|'child_process')/i
    for (const file of [
      './replay-clip-job-routes.ts',
      './replay-clip-jobs.ts',
    ]) {
      const source = readFileSync(
        fileURLToPath(new URL(file, import.meta.url)),
        'utf8',
      )
      expect(source).not.toMatch(nativeImportPattern)
      expect(source).not.toMatch(/\bspawn\s*\(|execSync|\.screenshot\(/i)
    }
  })
})

import { mkdtemp, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import { describe, expect, test } from 'vitest'
import { assertReplayClipManifestSafe } from '@openagentsinc/replay-clips'

import {
  buildManifest,
  objectKeysForRender,
  preflightUpload,
  publicStorageUrl,
  r2ObjectEndpoint,
  uploadReplayClipOutputs,
} from './render-job.mjs'

const job = {
  schemaVersion: 'openagents.replay_clip_job.v1',
  jobRef: 'replay_clip_job.first-real-settlement.example',
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
      { second: 4, verb: 'frame_settlement' },
    ],
  },
  sourceRefs: [
    'https://openagents.com/api/public/tassadar-replays/first-real-settlement',
  ],
}

const upload = {
  configured: true,
  accessKeyId: 'AKIATEST',
  accountId: 'account123',
  bucket: 'oa-replay-clips',
  prefix: 'replay-clips',
  publicHost: 'https://clips.openagents.test',
  secretAccessKey: 'secret',
}

describe('render-job R2 upload path (#5431)', () => {
  test('preflight fails closed without printing secret values', () => {
    const result = preflightUpload({
      R2_REPLAY_CLIPS_ACCESS_KEY_ID: 'AKIATEST',
      R2_REPLAY_CLIPS_SECRET_ACCESS_KEY: 'do-not-print',
    })

    expect(result.configured).toBe(false)
    expect(result.blockerRef).toBe(
      'needs_owner.replay_clip.r2_bucket_not_provisioned',
    )
    expect(result.detail).toContain('R2_REPLAY_CLIPS_BUCKET')
    expect(result.detail).not.toContain('do-not-print')
  })

  test('preflight accepts owner-provisioned R2 settings only with https public host', () => {
    const configured = preflightUpload({
      R2_REPLAY_CLIPS_ACCESS_KEY_ID: 'AKIATEST',
      R2_REPLAY_CLIPS_ACCOUNT_ID: 'account123',
      R2_REPLAY_CLIPS_BUCKET: 'oa-replay-clips',
      R2_REPLAY_CLIPS_PREFIX: 'clips',
      R2_REPLAY_CLIPS_PUBLIC_HOST: 'https://clips.openagents.test/',
      R2_REPLAY_CLIPS_SECRET_ACCESS_KEY: 'secret',
    })
    expect(configured).toMatchObject({
      configured: true,
      bucket: 'oa-replay-clips',
      prefix: 'clips',
      publicHost: 'https://clips.openagents.test',
    })

    const invalid = preflightUpload({
      R2_REPLAY_CLIPS_ACCESS_KEY_ID: 'AKIATEST',
      R2_REPLAY_CLIPS_ACCOUNT_ID: 'account123',
      R2_REPLAY_CLIPS_BUCKET: 'oa-replay-clips',
      R2_REPLAY_CLIPS_PUBLIC_HOST: 'http://clips.openagents.test',
      R2_REPLAY_CLIPS_SECRET_ACCESS_KEY: 'secret',
    })
    expect(invalid.configured).toBe(false)
    expect(invalid.blockerRef).toBe(
      'config.replay_clip.r2_public_host_invalid',
    )
  })

  test('builds schema-valid public manifests with R2 storage URLs', () => {
    const { mp4ObjectKey } = objectKeysForRender({
      job,
      out: '/tmp/clip.mp4',
      prefix: 'clips',
    })
    const manifest = buildManifest({
      artifacts: [
        {
          byteSize: 1482944,
          objectKey: mp4ObjectKey,
          sha256:
            '9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08',
        },
      ],
      bundleRef: 'proof_replay_bundle.first-real-settlement',
      frameCount: 60,
      job,
      publicHost: upload.publicHost,
    })

    const safe = assertReplayClipManifestSafe(manifest)
    expect(safe.artifacts[0]?.storageUrl).toBe(
      'https://clips.openagents.test/clips/replay_clip_job.first-real-settlement.example/clip.mp4',
    )
  })

  test('uploads mp4 and manifest objects through the R2 S3-compatible endpoint', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'render-job-r2-'))
    const mp4Path = join(dir, 'clip.mp4')
    const manifestPath = join(dir, 'clip.mp4.clip-manifest.json')
    await writeFile(mp4Path, 'mp4-bytes')
    await writeFile(manifestPath, '{"ok":true}\n')

    const calls = []
    const client = {
      fetch: async (url, init) => {
        calls.push({
          body: Buffer.from(init.body).toString('utf8'),
          contentType: init.headers['content-type'],
          method: init.method,
          url,
        })
        return new Response('', {
          headers: { etag: '"test-etag"' },
          status: 200,
        })
      },
    }

    const result = await uploadReplayClipOutputs({
      client,
      manifestObjectKey:
        'clips/replay_clip_job.first-real-settlement.example/clip.mp4.clip-manifest.json',
      manifestPath,
      mp4ObjectKey:
        'clips/replay_clip_job.first-real-settlement.example/clip.mp4',
      mp4Path,
      upload,
    })

    expect(calls.map(call => call.method)).toEqual(['PUT', 'PUT'])
    expect(calls.map(call => call.contentType)).toEqual([
      'video/mp4',
      'application/json; charset=utf-8',
    ])
    expect(calls.map(call => call.url)).toEqual([
      r2ObjectEndpoint(
        upload,
        'clips/replay_clip_job.first-real-settlement.example/clip.mp4',
      ),
      r2ObjectEndpoint(
        upload,
        'clips/replay_clip_job.first-real-settlement.example/clip.mp4.clip-manifest.json',
      ),
    ])
    expect(result.mp4.storageUrl).toBe(
      publicStorageUrl(
        upload.publicHost,
        'clips/replay_clip_job.first-real-settlement.example/clip.mp4',
      ),
    )
    expect(result.manifest.etag).toBe('"test-etag"')
  })
})

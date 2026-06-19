import { deflateSync } from 'node:zlib'
import { describe, expect, test } from 'vitest'

import {
  analyzePngBytes,
  assertClipManifestComplete,
  assertFrameNonblank,
  generatedReplayUrl,
  normalizeSourceRefs,
} from './render-regression-smoke.mjs'

const chunk = (type, data = Buffer.alloc(0)) => {
  const header = Buffer.alloc(8)
  header.writeUInt32BE(data.length, 0)
  header.write(type, 4, 4, 'ascii')
  return Buffer.concat([header, data, Buffer.alloc(4)])
}

const png = ({ height, pixel, width }) => {
  const ihdr = Buffer.alloc(13)
  ihdr.writeUInt32BE(width, 0)
  ihdr.writeUInt32BE(height, 4)
  ihdr[8] = 8
  ihdr[9] = 6
  ihdr[10] = 0
  ihdr[11] = 0
  ihdr[12] = 0

  const rows = []
  for (let y = 0; y < height; y += 1) {
    const row = Buffer.alloc(1 + width * 4)
    row[0] = 0
    for (let x = 0; x < width; x += 1) {
      const [red, green, blue, alpha] = pixel(x, y)
      const offset = 1 + x * 4
      row[offset] = red
      row[offset + 1] = green
      row[offset + 2] = blue
      row[offset + 3] = alpha
    }
    rows.push(row)
  }

  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(Buffer.concat(rows))),
    chunk('IEND'),
  ])
}

describe('render-regression-smoke (#5434)', () => {
  test('detects nonblank PNG frames and rejects blank frames', () => {
    const colorful = analyzePngBytes(
      png({
        height: 8,
        pixel: (x, y) => [
          (x * 31) % 256,
          (y * 47) % 256,
          ((x + y) * 19) % 256,
          255,
        ],
        width: 8,
      }),
    )
    expect(colorful.distinctColorBuckets).toBeGreaterThan(3)
    expect(() => assertFrameNonblank(colorful)).not.toThrow()

    const blank = analyzePngBytes(
      png({
        height: 8,
        pixel: () => [0, 0, 0, 255],
        width: 8,
      }),
    )
    expect(() => assertFrameNonblank(blank)).toThrow(/blank|contrast/)
  })

  test('builds the generated public activity replay URL', () => {
    const url = generatedReplayUrl({
      generatedFrom: '2026-06-18T00:00:00.000Z',
      generatedLimit: 20,
      generatedTo: '2026-06-19T00:00:00.000Z',
      origin: 'https://openagents.com',
    })
    expect(url).toContain('/api/public/proof-replays')
    expect(url).toContain('mode=activity-timeline')
    expect(url).toContain('limit=20')
  })

  test('normalizes object source refs without private material', () => {
    expect(
      normalizeSourceRefs([
        {
          kind: 'run',
          ref: 'run.tassadar.executor.20260615',
          url: 'https://openagents.com/api/public/tassadar-run-summary',
        },
        { kind: 'issue', ref: 'issue.github.openagents.5006' },
      ]),
    ).toEqual([
      'https://openagents.com/api/public/tassadar-run-summary',
      'issue:issue.github.openagents.5006',
    ])
    expect(() =>
      normalizeSourceRefs(['/Users/chris/private-trace.json']),
    ).toThrow(/private\/raw/)
  })

  test('requires source refs, caveats, and public https artifacts in manifests', () => {
    const manifest = {
      artifacts: [
        {
          byteSize: 1024,
          kind: 'mp4',
          sha256:
            '9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08',
          storageUrl: 'https://clips.openagents.com/regression/clip.mp4',
        },
      ],
      caveatRefs: [
        'Clip is evidence-presentation only and grants no settlement or payout authority.',
      ],
      claimScope: 'evidence_presentation_only',
      frameCount: 1,
      schemaVersion: 'openagents.replay_clip_manifest.v1',
      sourceRefs: ['https://openagents.com/api/public/proof-replays'],
    }

    expect(assertClipManifestComplete(manifest)).toBe(manifest)
    expect(() =>
      assertClipManifestComplete({ ...manifest, caveatRefs: [] }),
    ).toThrow(/caveat/)
    expect(() =>
      assertClipManifestComplete({
        ...manifest,
        artifacts: [
          { ...manifest.artifacts[0], storageUrl: 'local:clip.mp4' },
        ],
      }),
    ).toThrow(/public https/)
  })
})

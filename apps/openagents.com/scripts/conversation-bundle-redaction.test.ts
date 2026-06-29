import { sha256Hex } from '@openagentsinc/nip90'
import { describe, expect, test } from 'bun:test'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import {
  ConversationBundleRefusal,
  buildConversationBundle,
  writeConversationBundle,
} from './conversation-bundle-redaction.ts'

const fixture = (name: string) =>
  join(import.meta.dir, 'fixtures', 'conversation-bundle-redaction', name)

describe('conversation bundle redaction', () => {
  test('creates a deterministic public-safe bundle manifest with a matching NIP-DS digest', async () => {
    const result = await buildConversationBundle({
      createdAt: '2026-06-10T12:00:00.000Z',
      d: 'fixture-conversation-bundle',
      inputs: [fixture('clean.jsonl')],
      summary: 'Fixture summary',
      title: 'Fixture Conversation Bundle',
    })

    expect(result.manifest.bundleDigest).toBe(sha256Hex(result.bundlePayload))
    expect(result.manifest.nipDs.listingDigest).toBe(
      result.manifest.bundleDigest,
    )
    expect(result.manifest.nipDs.listingTags).toContainEqual([
      'x',
      result.manifest.bundleDigest,
    ])
    expect(result.manifest.recordCount).toBe(3)
    expect(result.bundlePayload).not.toContain('fixture-user@example.com')
    expect(result.bundlePayload).not.toContain('/Users/fixture/private')
    expect(result.bundlePayload).not.toContain('privateMetadata')
    expect(result.bundlePayload).not.toContain('provider')
    expect(result.bundlePayload).toContain('[redacted-email]')
    expect(result.bundlePayload).toContain('[redacted-path]')
  })

  test('writes the bundle and manifest artifacts', async () => {
    const outDir = await mkdtemp(join(tmpdir(), 'conversation-bundle-'))
    try {
      const result = await writeConversationBundle({
        createdAt: '2026-06-10T12:00:00.000Z',
        d: 'fixture-conversation-bundle',
        inputs: [fixture('clean.jsonl')],
        outDir,
        summary: 'Fixture summary',
        title: 'Fixture Conversation Bundle',
      })
      const manifest = JSON.parse(await readFile(result.manifestFile, 'utf8'))
      const bundle = await readFile(result.bundleFile, 'utf8')

      expect(manifest.bundleDigest).toBe(sha256Hex(bundle))
      expect(bundle.endsWith('\n')).toBe(false)
      expect(manifest.nipDs.listingDigest).toBe(manifest.bundleDigest)
      expect(manifest.recordCount).toBe(3)
    } finally {
      await rm(outDir, { recursive: true, force: true })
    }
  })

  test('refuses seeded credential material before writing a sellable bundle', async () => {
    await expect(
      buildConversationBundle({
        createdAt: '2026-06-10T12:00:00.000Z',
        d: 'seeded-secret',
        inputs: [fixture('seeded-secret.jsonl')],
        summary: 'Fixture summary',
        title: 'Seeded Secret',
      }),
    ).rejects.toBeInstanceOf(ConversationBundleRefusal)
  })
})

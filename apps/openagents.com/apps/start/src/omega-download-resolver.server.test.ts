// @vitest-environment node
/**
 * Omega download resolver tests (#9280).
 *
 * The tampering/fixture cases run against an in-process FIXTURE Ed25519
 * keypair — the production private key is never read, loaded, or printed.
 * One suite additionally proves the CHECKED-IN generated manifest verifies
 * against the pinned PRODUCTION PUBLIC key, so a stale or hand-edited
 * payload/signature pair can never reach a deploy with green tests.
 */
import { describe, expect, test } from 'vitest'

import {
  signReleasePayload,
  type ReleaseSigningKey,
} from '@openagentsinc/release-contract/release-publish'
import { PRODUCTION_RELEASE_KEY_PIN } from '@openagentsinc/release-contract/update-contract'
import { generateKeyPairSync } from 'node:crypto'

import {
  OMEGA_DOWNLOAD_ARTIFACT_PATH,
  OMEGA_DOWNLOAD_RESOLUTION_PATH,
  createOmegaDownloadResolver,
  omegaDownloadFormats,
  omegaDownloadTargets,
  verifySignedOmegaDownloadManifest,
} from './omega-download-resolver.server'
import {
  omegaDownloadManifestPayload,
  omegaDownloadManifestSignature,
} from './omega-release/omega-download-manifest.gen'
import {
  omegaDownloadFormats as clientFormats,
  omegaDownloadTargets as clientTargets,
} from './routes/-omega-download-data'

// --- fixture key (NEVER the production key) --------------------------------

const pair = generateKeyPairSync('ed25519')
const seed = (pair.privateKey.export({ format: 'jwk' }) as { d?: string }).d ?? ''
const signingKey: ReleaseSigningKey = { d: seed, kid: 'fixture-omega-manifest' }

const otherPair = generateKeyPairSync('ed25519')
const otherSeed = (otherPair.privateKey.export({ format: 'jwk' }) as { d?: string }).d ?? ''
const otherKey: ReleaseSigningKey = { d: otherSeed, kid: 'fixture-omega-manifest' }

const fixtureManifest = {
  schema: 'openagents.omega.download_manifest.v1',
  product: 'omega',
  productName: 'Omega',
  channel: 'alpha',
  version: '0.2.0-rc26',
  releaseTag: 'v0.2.0-rc26',
  releasedAt: '2026-07-29T20:57:30Z',
  releaseNotes: 'Fixture notes.',
  releasePageUrl: 'https://github.com/OpenAgentsInc/omega/releases/tag/v0.2.0-rc26',
  sourceRepository: 'https://github.com/OpenAgentsInc/omega',
  sourceRevision: '7142e3404ae59af22a7a975615935e807c3e1288',
  signingPolicy: { alg: 'ed25519', keyId: 'fixture-omega-manifest' },
  artifacts: [
    {
      target: 'darwin-arm64',
      format: 'dmg',
      name: 'Omega-v0.2.0-rc26-macos-arm64.dmg',
      url: 'https://github.com/OpenAgentsInc/omega/releases/download/v0.2.0-rc26/Omega-v0.2.0-rc26-macos-arm64.dmg',
      sha256: 'e39cd7b822fc143f03e1d54d68a36ec0fe150bc33334d2bb50573342de3fcee3',
      byteLength: 193720537,
      minimumOs: '11.0',
      apple: {
        teamId: 'HQWSG26L43',
        signingIdentity: 'Developer ID Application: OpenAgents, Inc. (HQWSG26L43)',
        notarization: 'not_notarized',
      },
    },
  ],
  unavailableTargets: [
    { target: 'linux-x64', statement: 'No Omega build is published for Linux in this alpha.' },
  ],
  knownLimitations: ['Alpha prerelease.'],
}

const sign = (manifest: unknown, key: ReleaseSigningKey = signingKey) => {
  const payloadText = JSON.stringify(manifest, null, 2)
  const { envelope, pin } = signReleasePayload(new TextEncoder().encode(payloadText), key)
  return { payloadText, envelope, pin }
}

// ---------------------------------------------------------------------------

describe('verifySignedOmegaDownloadManifest', () => {
  test('accepts a correctly signed manifest', () => {
    const { payloadText, envelope, pin } = sign(fixtureManifest)
    const result = verifySignedOmegaDownloadManifest(payloadText, envelope, pin)
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.manifest.version).toBe('0.2.0-rc26')
      expect(result.manifest.artifacts).toHaveLength(1)
    }
  })

  test('rejects a tampered payload (version mutation)', () => {
    const { payloadText, envelope, pin } = sign(fixtureManifest)
    const tampered = payloadText.replace('0.2.0-rc26', '0.2.0-rc27')
    expect(verifySignedOmegaDownloadManifest(tampered, envelope, pin)).toEqual({
      ok: false,
      reason: 'payload_sha256_mismatch',
    })
  })

  test('rejects a signature from a different key with the same kid', () => {
    const { payloadText } = sign(fixtureManifest)
    const forged = sign(fixtureManifest, otherKey)
    const { pin } = sign(fixtureManifest)
    expect(
      verifySignedOmegaDownloadManifest(payloadText, forged.envelope, pin),
    ).toEqual({ ok: false, reason: 'signature_invalid' })
  })

  test('rejects an unpinned kid', () => {
    const { payloadText, envelope, pin } = sign(fixtureManifest)
    expect(
      verifySignedOmegaDownloadManifest(payloadText, { ...envelope, kid: 'other-kid' }, pin),
    ).toEqual({ ok: false, reason: 'kid_not_pinned' })
  })

  test('rejects a malformed signature envelope', () => {
    const { payloadText, pin } = sign(fixtureManifest)
    expect(verifySignedOmegaDownloadManifest(payloadText, { alg: 'none' }, pin)).toEqual({
      ok: false,
      reason: 'malformed_signature_envelope',
    })
  })

  test('rejects a signing-policy keyId that disagrees with the pin', () => {
    const { payloadText, envelope, pin } = sign({
      ...fixtureManifest,
      signingPolicy: { alg: 'ed25519', keyId: 'some-other-key' },
    })
    void payloadText
    const result = verifySignedOmegaDownloadManifest(payloadText, envelope, pin)
    expect(result).toEqual({ ok: false, reason: 'signing_policy_mismatch' })
  })

  test('rejects an artifact URL outside the named GitHub release', () => {
    const { payloadText, envelope, pin } = sign({
      ...fixtureManifest,
      artifacts: [
        {
          ...fixtureManifest.artifacts[0],
          url: 'https://evil.example.com/Omega-v0.2.0-rc26-macos-arm64.dmg',
        },
      ],
    })
    expect(verifySignedOmegaDownloadManifest(payloadText, envelope, pin)).toEqual({
      ok: false,
      reason: 'artifact_url_outside_release',
    })
  })

  test('rejects a target listed both as published and unavailable', () => {
    const { payloadText, envelope, pin } = sign({
      ...fixtureManifest,
      unavailableTargets: [
        { target: 'darwin-arm64', statement: 'Contradiction: also published above.' },
      ],
    })
    expect(verifySignedOmegaDownloadManifest(payloadText, envelope, pin)).toEqual({
      ok: false,
      reason: 'target_conflict',
    })
  })

  test('rejects a schema-invalid manifest (wrong product)', () => {
    const { payloadText, envelope, pin } = sign({ ...fixtureManifest, product: 'desktop' })
    expect(verifySignedOmegaDownloadManifest(payloadText, envelope, pin)).toEqual({
      ok: false,
      reason: 'manifest_schema_invalid',
    })
  })
})

describe('createOmegaDownloadResolver', () => {
  const fixtureResolver = () => {
    const { payloadText, envelope, pin } = sign(fixtureManifest)
    return createOmegaDownloadResolver({ payloadText, signature: envelope, pin })
  }

  test('serves the available resolution with per-cell truth', async () => {
    const response = await fixtureResolver().handle(
      new Request(`https://openagents.com${OMEGA_DOWNLOAD_RESOLUTION_PATH}`),
    )
    expect(response?.status).toBe(200)
    expect(response?.headers.get('cache-control')).toBe('no-store')
    const body = (await response?.json()) as Record<string, unknown>
    expect(body['schema']).toBe('openagents.omega.download_resolution.v1')
    expect(body['availability']).toBe('available')
    expect(body['product']).toBe('omega')
    expect(body['channel']).toBe('alpha')
    const artifacts = body['artifacts'] as Array<Record<string, unknown>>
    expect(artifacts[0]?.['sha256']).toBe(
      'e39cd7b822fc143f03e1d54d68a36ec0fe150bc33334d2bb50573342de3fcee3',
    )
    expect(body['unavailableTargets']).toHaveLength(1)
  })

  test('fails closed to unavailable on a tampered payload', async () => {
    const { payloadText, envelope, pin } = sign(fixtureManifest)
    const resolver = createOmegaDownloadResolver({
      payloadText: payloadText.replace('rc26', 'rc99'),
      signature: envelope,
      pin,
    })
    const resolution = resolver.resolve()
    expect(resolution.availability).toBe('unavailable')
    if (resolution.availability === 'unavailable') {
      expect(resolution.reason).toBe('payload_sha256_mismatch')
    }
    const response = await resolver.handle(
      new Request(`https://openagents.com${OMEGA_DOWNLOAD_ARTIFACT_PATH}?target=darwin-arm64&format=dmg`),
    )
    expect(response?.status).toBe(503)
  })

  test('redirects the artifact CTA to the signed URL only', async () => {
    const response = await fixtureResolver().handle(
      new Request(`https://openagents.com${OMEGA_DOWNLOAD_ARTIFACT_PATH}?target=darwin-arm64&format=dmg`),
    )
    expect(response?.status).toBe(302)
    expect(response?.headers.get('location')).toBe(
      'https://github.com/OpenAgentsInc/omega/releases/download/v0.2.0-rc26/Omega-v0.2.0-rc26-macos-arm64.dmg',
    )
  })

  test('404s a target/format not in the signed manifest', async () => {
    const response = await fixtureResolver().handle(
      new Request(`https://openagents.com${OMEGA_DOWNLOAD_ARTIFACT_PATH}?target=linux-x64&format=appimage`),
    )
    expect(response?.status).toBe(404)
  })

  test('400s an invalid or missing target/format query', async () => {
    const resolver = fixtureResolver()
    const responses = await Promise.all(
      ['', '?target=darwin-arm64', '?target=solaris&format=dmg'].map(query =>
        resolver.handle(
          new Request(`https://openagents.com${OMEGA_DOWNLOAD_ARTIFACT_PATH}${query}`),
        ),
      ),
    )
    for (const response of responses) expect(response?.status).toBe(400)
  })

  test('rejects non-GET methods and ignores other paths', async () => {
    const resolver = fixtureResolver()
    const post = await resolver.handle(
      new Request(`https://openagents.com${OMEGA_DOWNLOAD_RESOLUTION_PATH}`, { method: 'POST' }),
    )
    expect(post?.status).toBe(405)
    expect(
      await resolver.handle(new Request('https://openagents.com/api/public/other')),
    ).toBeUndefined()
  })
})

describe('checked-in signed manifest', () => {
  test('verifies against the pinned PRODUCTION public key', () => {
    const result = verifySignedOmegaDownloadManifest(
      omegaDownloadManifestPayload,
      omegaDownloadManifestSignature,
      PRODUCTION_RELEASE_KEY_PIN,
    )
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.manifest.product).toBe('omega')
      expect(result.manifest.channel).toBe('alpha')
      expect(result.manifest.artifacts.length).toBeGreaterThan(0)
      // Never relabel the Electron Desktop artifact as Omega: every published
      // Omega artifact must come from the Omega repository's own release.
      for (const artifact of result.manifest.artifacts) {
        expect(artifact.url.startsWith('https://github.com/OpenAgentsInc/omega/releases/download/')).toBe(true)
        expect(artifact.name.startsWith('Omega-')).toBe(true)
      }
    }
  })

  test('the default resolver serves the checked-in manifest as available', () => {
    const resolution = createOmegaDownloadResolver().resolve()
    expect(resolution.availability).toBe('available')
  })
})

describe('client vocab lockstep', () => {
  test('the client-safe literal vocab matches the server contract', () => {
    expect([...clientTargets]).toEqual([...omegaDownloadTargets])
    expect([...clientFormats]).toEqual([...omegaDownloadFormats])
  })
})

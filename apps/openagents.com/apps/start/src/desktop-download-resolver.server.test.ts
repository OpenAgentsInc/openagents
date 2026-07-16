// @vitest-environment node
//
// The resolver is server-only. The app default happy-dom environment applies
// the BROWSER forbidden-header guard to `new Request(...)`, silently dropping
// `sec-ch-ua-*`, `user-agent`, and `referer` — headers the real Cloud Run
// server request carries. Node's undici preserves them, matching production.
/**
 * DIST-10 (#8923) — Desktop download resolver tests.
 *
 * Everything runs against an in-process FIXTURE Ed25519 keypair and the
 * checked-in ReleaseSet v2 fixture from the desktop contract suite. The
 * production private key is never read, loaded, or printed.
 */
import { generateKeyPairSync } from 'node:crypto'
import { readFileSync } from 'node:fs'
import path from 'node:path'
import { describe, expect, test } from 'vitest'

import {
  signReleasePayload,
  type ReleaseSigningKey,
} from '../../../../../apps/openagents-desktop/src/release-publish.ts'
import {
  canonicalizeReleaseSet,
} from '../../../../../apps/openagents-desktop/src/release-set-contract.ts'
import {
  DESKTOP_DOWNLOAD_ARTIFACT_PATH,
  DESKTOP_DOWNLOAD_RESOLUTION_PATH,
  DESKTOP_DOWNLOAD_TELEMETRY_SCHEMA_ID,
  createDesktopDownloadResolver,
  detectDesktopClient,
  emitDesktopDownloadTelemetry,
  referrerCategory,
  releaseSetPayloadPath,
  releaseSetSignaturePath,
  v1ManifestPath,
  v1PointerPath,
  v1SignaturePath,
  type DesktopDownloadTelemetryEvent,
} from './desktop-download-resolver.server'

// --- fixture key (NEVER the production key) --------------------------------

const pair = generateKeyPairSync('ed25519')
const seed = (pair.privateKey.export({ format: 'jwk' }) as { d?: string }).d ?? ''
// kid must equal the fixture set's signingPolicy.keyId for the v2 crosscheck.
const signingKey: ReleaseSigningKey = { d: seed, kid: 'fixture-release-set-v2' }
const pin = signReleasePayload(new Uint8Array([1]), signingKey).pin

const otherPair = generateKeyPairSync('ed25519')
const otherSeed = (otherPair.privateKey.export({ format: 'jwk' }) as { d?: string }).d ?? ''
const otherKey: ReleaseSigningKey = { d: otherSeed, kid: 'fixture-release-set-v2' }

// --- fixture release sets ---------------------------------------------------

const fixtureRaw = readFileSync(
  path.resolve(
    import.meta.dirname,
    '../../../../../apps/openagents-desktop/tests/fixtures/release-set-v2.json',
  ),
  'utf8',
)
const RC_VERSION = '2.4.0-rc.3'
const RC_NEXT_VERSION = '2.4.0-rc.4'
const STABLE_VERSION = '2.4.0'

const rcNextRaw = fixtureRaw.replaceAll(RC_VERSION, RC_NEXT_VERSION)
const stableRaw = fixtureRaw
  .replaceAll(RC_VERSION, STABLE_VERSION)
  .replaceAll('-rc-', '-stable-')
  .replace('"channel": "rc"', '"channel": "stable"')

const sign = (raw: string, key: ReleaseSigningKey = signingKey) => {
  const payloadBytes = canonicalizeReleaseSet(JSON.parse(raw))
  const { envelope } = signReleasePayload(payloadBytes, key)
  return { payload: payloadBytes, signature: JSON.stringify(envelope) }
}

// --- fixture v1 manifest (bounded migration path) ---------------------------

const v1Manifest = {
  schema: 'openagents.desktop.update_manifest.v1',
  app: 'openagents-desktop',
  channel: 'rc',
  version: '0.1.0-rc.13',
  artifactName: 'OpenAgents-0.1.0-rc.13-arm64.dmg',
  artifactSha256: 'e1'.repeat(32),
  artifactByteLength: 303959067,
  releasedAt: '2026-07-14T21:05:24.119Z',
  notesRef: 'release.notes.0.1.0-rc.13',
} as const
const v1Bytes = new TextEncoder().encode(JSON.stringify(v1Manifest))
const v1Envelope = JSON.stringify(signReleasePayload(v1Bytes, signingKey).envelope)
const v1ArtifactUrl =
  'https://storage.googleapis.com/fixture-updates/desktop/OpenAgents-0.1.0-rc.13-arm64.dmg'
const v1Pointer = JSON.stringify({
  channel: 'rc',
  version: '0.1.0-rc.13',
  artifactName: 'OpenAgents-0.1.0-rc.13-arm64.dmg',
  artifactUrl: v1ArtifactUrl,
})

// --- harness -----------------------------------------------------------------

type FeedFile = string | Uint8Array
const BASE = 'https://updates.fixture.test'

const makeFetch = (files: Map<string, FeedFile>): typeof fetch =>
  ((input: RequestInfo | URL) => {
    const pathname = new URL(String(input)).pathname
    const body = files.get(pathname)
    return Promise.resolve(
      body === undefined
        ? new Response('not found', { status: 404 })
        : new Response(body as BodyInit, { status: 200 }),
    )
  }) as typeof fetch

const rcFeed = (raw: string = fixtureRaw): Map<string, FeedFile> => {
  const { payload, signature } = sign(raw)
  return new Map<string, FeedFile>([
    [releaseSetPayloadPath('rc'), payload],
    [releaseSetSignaturePath('rc'), signature],
  ])
}

const NOW = Date.parse('2026-07-16T12:00:00.000Z')

const makeResolver = (input?: {
  files?: Map<string, FeedFile>
  nowMs?: () => number
  events?: DesktopDownloadTelemetryEvent[]
  defaultChannel?: 'stable' | 'rc'
}) => {
  const events = input?.events ?? []
  return createDesktopDownloadResolver({
    config: {
      baseUrl: BASE,
      defaultChannel: input?.defaultChannel ?? 'rc',
      pin,
      cacheTtlMs: 60_000,
    },
    fetchFn: makeFetch(input?.files ?? rcFeed()),
    nowMs: input?.nowMs ?? (() => NOW),
    telemetrySink: event => {
      events.push(event)
    },
  })
}

const request = (input?: {
  headers?: Record<string, string>
  path?: string
  query?: string
  method?: string
}): Request =>
  new Request(
    `https://openagents.com${input?.path ?? DESKTOP_DOWNLOAD_RESOLUTION_PATH}${input?.query ?? ''}`,
    { headers: input?.headers ?? {}, method: input?.method ?? 'GET' },
  )

const resolveJson = async (
  resolver: ReturnType<typeof createDesktopDownloadResolver>,
  input?: Parameters<typeof request>[0],
) => {
  const response = await resolver.handle(request(input))
  if (response === undefined) throw new Error('route not handled')
  return { response, body: (await response.json()) as Record<string, unknown> }
}

// --- detection ---------------------------------------------------------------

describe('client detection (bounded field parsing)', () => {
  const hintCases: ReadonlyArray<
    [string, Record<string, string>, string | null, string | null]
  > = [
    ['macOS arm hints', { 'sec-ch-ua-platform': '"macOS"', 'sec-ch-ua-arch': '"arm"' }, 'darwin', 'arm64'],
    ['macOS x86-64 hints', { 'sec-ch-ua-platform': '"macOS"', 'sec-ch-ua-arch': '"x86"', 'sec-ch-ua-bitness': '"64"' }, 'darwin', 'x64'],
    ['Windows arm hints', { 'sec-ch-ua-platform': '"Windows"', 'sec-ch-ua-arch': '"arm"' }, 'win32', 'arm64'],
    ['Windows x86-64 hints', { 'sec-ch-ua-platform': '"Windows"', 'sec-ch-ua-arch': '"x86"', 'sec-ch-ua-bitness': '"64"' }, 'win32', 'x64'],
    ['Linux arm hints', { 'sec-ch-ua-platform': '"Linux"', 'sec-ch-ua-arch': '"arm"' }, 'linux', 'arm64'],
    ['Linux x86-64 hints', { 'sec-ch-ua-platform': '"Linux"', 'sec-ch-ua-arch': '"x86"', 'sec-ch-ua-bitness': '"64"' }, 'linux', 'x64'],
    ['x86 32-bit is unsupported', { 'sec-ch-ua-platform': '"Windows"', 'sec-ch-ua-arch': '"x86"', 'sec-ch-ua-bitness': '"32"' }, 'win32', null],
    ['unsupported hint platform', { 'sec-ch-ua-platform': '"Android"' }, null, null],
  ]
  for (const [label, headers, platform, architecture] of hintCases) {
    test(label, () => {
      const detection = detectDesktopClient(new Headers(headers))
      expect(detection.platform).toBe(platform)
      expect(detection.architecture).toBe(architecture)
    })
  }

  const uaCases: ReadonlyArray<[string, string, string | null, string | null]> = [
    ['Windows x64 UA', 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/126', 'win32', 'x64'],
    ['Windows ARM64 UA', 'Mozilla/5.0 (Windows NT 10.0; ARM64) Chrome/126', 'win32', 'arm64'],
    ['Linux x86_64 UA', 'Mozilla/5.0 (X11; Linux x86_64) Firefox/128', 'linux', 'x64'],
    ['Linux aarch64 UA', 'Mozilla/5.0 (X11; Linux aarch64) Firefox/128', 'linux', 'arm64'],
    // `Intel Mac OS X` is reported by Apple-silicon browsers too — the arch
    // is deliberately NOT inferred from it.
    ['Safari mac UA leaves arch unknown', 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Safari/605', 'darwin', null],
    ['iPhone is not a desktop client', 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_5)', null, null],
    ['Android is not a desktop client', 'Mozilla/5.0 (Linux; Android 15) Chrome/126', null, null],
  ]
  for (const [label, ua, platform, architecture] of uaCases) {
    test(label, () => {
      const detection = detectDesktopClient(new Headers({ 'user-agent': ua }))
      expect(detection.platform).toBe(platform)
      expect(detection.architecture).toBe(architecture)
    })
  }

  test('client hints take precedence over the user-agent field', () => {
    const detection = detectDesktopClient(
      new Headers({
        'sec-ch-ua-platform': '"Linux"',
        'sec-ch-ua-arch': '"arm"',
        'user-agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)',
      }),
    )
    expect(detection).toEqual({ platform: 'linux', architecture: 'arm64', method: 'client_hints' })
  })

  test('no signals yields the honest none detection', () => {
    expect(detectDesktopClient(new Headers())).toEqual({
      platform: null,
      architecture: null,
      method: 'none',
    })
  })
})

// --- resolution: six targets, alternatives, overrides ------------------------

describe('resolution from the verified release set', () => {
  const targetMatrix: ReadonlyArray<[Record<string, string>, string, string]> = [
    [{ 'sec-ch-ua-platform': '"macOS"', 'sec-ch-ua-arch': '"arm"' }, 'darwin-arm64', 'dmg'],
    [{ 'sec-ch-ua-platform': '"macOS"', 'sec-ch-ua-arch': '"x86"', 'sec-ch-ua-bitness': '"64"' }, 'darwin-x64', 'dmg'],
    [{ 'sec-ch-ua-platform': '"Windows"', 'sec-ch-ua-arch': '"arm"' }, 'win32-arm64', 'nsis'],
    [{ 'sec-ch-ua-platform': '"Windows"', 'sec-ch-ua-arch': '"x86"', 'sec-ch-ua-bitness': '"64"' }, 'win32-x64', 'nsis'],
    [{ 'sec-ch-ua-platform': '"Linux"', 'sec-ch-ua-arch': '"arm"' }, 'linux-arm64', 'appimage'],
    [{ 'sec-ch-ua-platform': '"Linux"', 'sec-ch-ua-arch': '"x86"', 'sec-ch-ua-bitness': '"64"' }, 'linux-x64', 'appimage'],
  ]

  for (const [headers, target, format] of targetMatrix) {
    test(`${target} resolves its preferred ${format} artifact`, async () => {
      const { body } = await resolveJson(makeResolver(), { headers })
      expect(body.availability).toBe('available')
      expect(body.source).toBe('release_set_v2')
      expect(body.version).toBe(RC_VERSION)
      const selected = body.selected as Record<string, unknown>
      expect(selected.target).toBe(target)
      expect(selected.format).toBe(format)
      expect(selected.preferred).toBe(true)
      expect(String(selected.url)).toContain(target)
      expect(String(selected.url)).toContain(RC_VERSION)
      expect(String(selected.sha256)).toMatch(/^[0-9a-f]{64}$/)
      expect(Number(selected.byteLength)).toBeGreaterThan(0)
      expect(typeof selected.minimumOs).toBe('string')
    })
  }

  test('alternatives list same-target formats then other targets preferred artifacts', async () => {
    const { body } = await resolveJson(makeResolver(), {
      headers: { 'sec-ch-ua-platform': '"macOS"', 'sec-ch-ua-arch': '"arm"' },
    })
    const alternatives = body.alternatives as Array<Record<string, unknown>>
    expect(alternatives).toHaveLength(6)
    expect(alternatives[0]).toMatchObject({ target: 'darwin-arm64', format: 'zip' })
    expect(alternatives.slice(1).map(row => row.target)).toEqual([
      'darwin-x64',
      'win32-arm64',
      'win32-x64',
      'linux-arm64',
      'linux-x64',
    ])
    for (const row of alternatives.slice(1)) expect(row.preferred).toBe(true)
  })

  test('unknown client fails open to choose_manually with the full verified catalog', async () => {
    const { body } = await resolveJson(makeResolver())
    expect(body.availability).toBe('choose_manually')
    expect(body.reason).toBe('unknown_client')
    expect(body.selected).toBeUndefined()
    const options = body.options as Array<Record<string, unknown>>
    expect(options).toHaveLength(12)
    for (const option of options) {
      expect(String(option.url)).toMatch(/^https:\/\//)
      expect(option.version).toBe(RC_VERSION)
    }
  })

  test('mac platform without a provable architecture chooses manually instead of guessing', async () => {
    const { body } = await resolveJson(makeResolver(), {
      headers: { 'user-agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Safari/605' },
    })
    expect(body.availability).toBe('choose_manually')
    expect((body.detection as Record<string, unknown>).platform).toBe('darwin')
    expect((body.detection as Record<string, unknown>).architecture).toBeNull()
  })

  test('explicit target/format override wins over detection', async () => {
    const { body } = await resolveJson(makeResolver(), {
      headers: { 'sec-ch-ua-platform': '"macOS"', 'sec-ch-ua-arch': '"arm"' },
      query: '?target=linux-x64&format=deb',
    })
    expect(body.availability).toBe('available')
    const selected = body.selected as Record<string, unknown>
    expect(selected.target).toBe('linux-x64')
    expect(selected.format).toBe('deb')
    expect(selected.preferred).toBe(false)
    expect((body.detection as Record<string, unknown>).method).toBe('override')
  })

  test('a format the target does not offer chooses manually, never a wrong link', async () => {
    const { body } = await resolveJson(makeResolver(), {
      query: '?target=win32-x64&format=dmg',
    })
    expect(body.availability).toBe('choose_manually')
    expect(body.reason).toBe('format_unavailable')
  })

  test('a malformed override is a typed 400, not a guess', async () => {
    const { response } = await resolveJson(makeResolver(), { query: '?target=win95-x64' })
    expect(response.status).toBe(400)
  })

  test('non-GET is refused', async () => {
    const response = await makeResolver().handle(request({ method: 'POST' }))
    expect(response?.status).toBe(405)
  })

  test('other paths are not handled', async () => {
    const response = await makeResolver().handle(request({ path: '/api/public/qa-board' }))
    expect(response).toBeUndefined()
  })

  test('responses are no-store and advertise client hints', async () => {
    const { response } = await resolveJson(makeResolver())
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(response.headers.get('accept-ch')).toContain('Sec-CH-UA-Platform')
    expect(response.headers.get('vary')).toContain('User-Agent')
  })
})

// --- fail closed: signatures, schema, mutation -------------------------------

describe('fail-closed verification', () => {
  test('feed unreachable yields unavailable with NO url anywhere', async () => {
    const { response, body } = await resolveJson(
      makeResolver({ files: new Map() }),
      { headers: { 'sec-ch-ua-platform': '"macOS"', 'sec-ch-ua-arch': '"arm"' } },
    )
    expect(response.status).toBe(200)
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('feed_unreachable')
    // The committed-hard-coded-URL guard: an unavailable projection carries
    // no artifact URL of any kind — there is no fallback constant to leak.
    expect(JSON.stringify(body)).not.toContain('https://')
  })

  test('a signature from a different key under the pinned kid is rejected', async () => {
    const { payload } = sign(fixtureRaw)
    const forged = sign(fixtureRaw, otherKey)
    const files = new Map<string, FeedFile>([
      [releaseSetPayloadPath('rc'), payload],
      [releaseSetSignaturePath('rc'), forged.signature],
    ])
    const { body } = await resolveJson(makeResolver({ files }))
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('release_set_verification_failed')
  })

  const mutations: ReadonlyArray<[string, string, string]> = [
    ['url', 'OpenAgents-2.4.0-rc.3-rc-darwin-arm64.dmg', 'OpenAgents-2.4.0-rc.3-rc-darwin-arm64-evil.dmg'],
    ['hash', '4b672044ba1e15584f48e3b7716454dc3c08f8badb1c7d05912e9db607dfbdca', 'f'.repeat(64)],
    // Canonical JSON is compact (no spaces around separators).
    ['target', '"target":"darwin-arm64"', '"target":"darwin-x64"'],
  ]
  for (const [field, needle, replacement] of mutations) {
    test(`mutation test: a changed ${field} cannot pass verification`, async () => {
      const { payload, signature } = sign(fixtureRaw)
      const mutated = new TextEncoder().encode(
        new TextDecoder().decode(payload).replace(needle, replacement),
      )
      expect(mutated).not.toEqual(payload)
      const files = new Map<string, FeedFile>([
        [releaseSetPayloadPath('rc'), mutated],
        [releaseSetSignaturePath('rc'), signature],
      ])
      const { body } = await resolveJson(makeResolver({ files }))
      expect(body.availability).toBe('unavailable')
      expect(body.reason).toBe('release_set_verification_failed')
      expect(JSON.stringify(body)).not.toContain('https://')
    })
  }

  test('a stable set served on the rc channel pointer is a channel mismatch', async () => {
    const { payload, signature } = sign(stableRaw)
    const files = new Map<string, FeedFile>([
      [releaseSetPayloadPath('rc'), payload],
      [releaseSetSignaturePath('rc'), signature],
    ])
    const { body } = await resolveJson(makeResolver({ files }))
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('release_set_verification_failed')
  })

  test('unparseable signature JSON is feed_schema_invalid', async () => {
    const { payload } = sign(fixtureRaw)
    const files = new Map<string, FeedFile>([
      [releaseSetPayloadPath('rc'), payload],
      [releaseSetSignaturePath('rc'), 'not json'],
    ])
    const { body } = await resolveJson(makeResolver({ files }))
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('feed_schema_invalid')
  })
})

// --- cache, freshness, pointer changes ---------------------------------------

describe('cache and freshness', () => {
  test('pointer change: after the TTL the new set is served whole — never mixed', async () => {
    let now = NOW
    const files = rcFeed()
    const resolver = makeResolver({ files, nowMs: () => now })
    const headers = { 'sec-ch-ua-platform': '"macOS"', 'sec-ch-ua-arch': '"arm"' }

    const first = await resolveJson(resolver, { headers })
    expect(first.body.version).toBe(RC_VERSION)

    // Promotion happens: the channel pointer now serves rc.4.
    const next = sign(rcNextRaw)
    files.set(releaseSetPayloadPath('rc'), next.payload)
    files.set(releaseSetSignaturePath('rc'), next.signature)

    // Within the TTL the cached rc.3 snapshot is still served (consistent).
    now += 30_000
    const cached = await resolveJson(resolver, { headers })
    expect(cached.body.version).toBe(RC_VERSION)
    expect(String((cached.body.selected as Record<string, unknown>).url)).toContain(RC_VERSION)

    // Past the TTL the resolver revalidates and serves rc.4 atomically.
    now += 31_000
    const refreshed = await resolveJson(resolver, { headers })
    expect(refreshed.body.version).toBe(RC_NEXT_VERSION)
    const selected = refreshed.body.selected as Record<string, unknown>
    expect(selected.version).toBe(RC_NEXT_VERSION)
    expect(String(selected.url)).toContain(RC_NEXT_VERSION)
    expect(String(selected.url)).not.toContain(RC_VERSION)
    for (const row of refreshed.body.alternatives as Array<Record<string, unknown>>) {
      expect(row.version).toBe(RC_NEXT_VERSION)
    }
  })

  test('an expired cache entry is never served when revalidation fails', async () => {
    let now = NOW
    const files = rcFeed()
    const resolver = makeResolver({ files, nowMs: () => now })
    const headers = { 'sec-ch-ua-platform': '"macOS"', 'sec-ch-ua-arch': '"arm"' }

    expect((await resolveJson(resolver, { headers })).body.availability).toBe('available')

    files.clear() // feed goes down (and no v1 either)
    now += 61_000
    const { body } = await resolveJson(resolver, { headers })
    expect(body.availability).toBe('unavailable')
    expect(JSON.stringify(body)).not.toContain('https://')
  })

  test('channels are cached separately and never cross-serve', async () => {
    const files = rcFeed()
    const stable = sign(stableRaw)
    files.set(releaseSetPayloadPath('stable'), stable.payload)
    files.set(releaseSetSignaturePath('stable'), stable.signature)
    const resolver = makeResolver({ files })
    const headers = { 'sec-ch-ua-platform': '"macOS"', 'sec-ch-ua-arch': '"arm"' }

    const rc = await resolveJson(resolver, { headers })
    expect(rc.body.channel).toBe('rc')
    expect(rc.body.version).toBe(RC_VERSION)

    const stableBody = (await resolveJson(resolver, { headers, query: '?channel=stable' })).body
    expect(stableBody.channel).toBe('stable')
    expect(stableBody.version).toBe(STABLE_VERSION)
    expect(String((stableBody.selected as Record<string, unknown>).url)).toContain('-stable-')

    // The rc cache entry is untouched by the stable fetch.
    const rcAgain = await resolveJson(resolver, { headers })
    expect(rcAgain.body.version).toBe(RC_VERSION)
    expect(String((rcAgain.body.selected as Record<string, unknown>).url)).toContain('-rc-')
  })
})

// --- bounded v1 migration path ------------------------------------------------

describe('bounded v1 darwin-arm64 migration (release-set feed not yet published)', () => {
  const v1Files = (): Map<string, FeedFile> =>
    new Map<string, FeedFile>([
      [v1ManifestPath('rc'), v1Bytes],
      [v1SignaturePath('rc'), v1Envelope],
      [v1PointerPath('rc'), v1Pointer],
    ])

  test('serves the signed v1 manifest truth for darwin-arm64', async () => {
    const { body } = await resolveJson(makeResolver({ files: v1Files() }), {
      headers: { 'sec-ch-ua-platform': '"macOS"', 'sec-ch-ua-arch': '"arm"' },
    })
    expect(body.availability).toBe('available')
    expect(body.source).toBe('v1_darwin_arm64_migration')
    expect(body.version).toBe('0.1.0-rc.13')
    const selected = body.selected as Record<string, unknown>
    expect(selected.target).toBe('darwin-arm64')
    expect(selected.format).toBe('dmg')
    expect(selected.url).toBe(v1ArtifactUrl)
    expect(selected.sha256).toBe(v1Manifest.artifactSha256)
    expect(selected.byteLength).toBe(v1Manifest.artifactByteLength)
    expect(body.alternatives).toEqual([])
  })

  test('a target the v1 set does not carry is honestly unavailable-to-choose', async () => {
    const { body } = await resolveJson(makeResolver({ files: v1Files() }), {
      headers: { 'sec-ch-ua-platform': '"Windows"', 'sec-ch-ua-arch': '"x86"', 'sec-ch-ua-bitness': '"64"' },
    })
    expect(body.availability).toBe('choose_manually')
    expect(body.reason).toBe('target_unavailable')
    expect(body.options as Array<unknown>).toHaveLength(1)
  })

  test('an unsigned pointer whose artifact name mismatches the signed manifest is rejected', async () => {
    const files = v1Files()
    files.set(
      v1PointerPath('rc'),
      JSON.stringify({
        channel: 'rc',
        version: '0.1.0-rc.13',
        artifactName: 'OpenAgents-0.1.0-rc.13-arm64.dmg',
        artifactUrl: 'https://storage.googleapis.com/fixture-updates/desktop/EvilPayload.dmg',
      }),
    )
    const { body } = await resolveJson(makeResolver({ files }))
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('v1_pointer_mismatch')
    expect(JSON.stringify(body)).not.toContain('EvilPayload')
  })

  test('a tampered v1 manifest fails verification', async () => {
    const files = v1Files()
    files.set(
      v1ManifestPath('rc'),
      new TextEncoder().encode(JSON.stringify({ ...v1Manifest, version: '9.9.9-rc.1' })),
    )
    const { body } = await resolveJson(makeResolver({ files }))
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('v1_manifest_verification_failed')
  })

  test('past the migration window the v1 path is closed, not revived', async () => {
    const { body } = await resolveJson(
      makeResolver({ files: v1Files(), nowMs: () => Date.parse('2026-10-15T00:00:01Z') }),
    )
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('v1_selection_rejected')
  })

  test('a broken v2 feed never downgrades to v1 (fallback is 404-only)', async () => {
    const files = v1Files()
    const { payload, signature } = sign(fixtureRaw)
    const mutated = new TextEncoder().encode(
      new TextDecoder().decode(payload).replace(RC_VERSION, '9.9.9-rc.9'),
    )
    files.set(releaseSetPayloadPath('rc'), mutated)
    files.set(releaseSetSignaturePath('rc'), signature)
    const { body } = await resolveJson(makeResolver({ files }))
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('release_set_verification_failed')
  })
})

// --- artifact redirect ---------------------------------------------------------

describe('verified artifact redirect', () => {
  test('302s to the exact artifact URL bound to the promoted set', async () => {
    const resolver = makeResolver()
    const response = await resolver.handle(
      request({
        path: DESKTOP_DOWNLOAD_ARTIFACT_PATH,
        query: '?target=darwin-arm64&format=dmg',
      }),
    )
    expect(response?.status).toBe(302)
    expect(response?.headers.get('location')).toContain('darwin-arm64')
    expect(response?.headers.get('location')).toContain(RC_VERSION)
    expect(response?.headers.get('cache-control')).toBe('no-store')
  })

  test('requires explicit target and format', async () => {
    const response = await makeResolver().handle(
      request({ path: DESKTOP_DOWNLOAD_ARTIFACT_PATH, query: '?target=darwin-arm64' }),
    )
    expect(response?.status).toBe(400)
  })

  test('feed failure is a typed 503, never a stale or guessed location', async () => {
    const response = await makeResolver({ files: new Map() }).handle(
      request({
        path: DESKTOP_DOWNLOAD_ARTIFACT_PATH,
        query: '?target=darwin-arm64&format=dmg',
      }),
    )
    expect(response?.status).toBe(503)
    expect(response?.headers.get('location')).toBeNull()
  })

  test('a format valid globally but absent for the target is a 404', async () => {
    const response = await makeResolver().handle(
      request({
        path: DESKTOP_DOWNLOAD_ARTIFACT_PATH,
        query: '?target=win32-x64&format=rpm',
      }),
    )
    expect(response?.status).toBe(404)
  })
})

// --- telemetry -------------------------------------------------------------------

describe('download telemetry', () => {
  test('a successful resolution emits one validated public-safe event', async () => {
    const events: DesktopDownloadTelemetryEvent[] = []
    const resolver = makeResolver({ events })
    await resolver.handle(
      request({
        headers: {
          'sec-ch-ua-platform': '"macOS"',
          'sec-ch-ua-arch': '"arm"',
          'user-agent': 'Mozilla/5.0 (Macintosh) SecretBuild/1.2',
          referer: 'https://openagents.com/',
        },
      }),
    )
    expect(events).toHaveLength(1)
    const event = events[0]!
    expect(event.schema).toBe(DESKTOP_DOWNLOAD_TELEMETRY_SCHEMA_ID)
    expect(event.day).toBe('2026-07-16')
    expect(event.channel).toBe('rc')
    expect(event.outcome).toBe('available')
    expect(event.version).toBe(RC_VERSION)
    expect(event.target).toBe('darwin-arm64')
    expect(event.format).toBe('dmg')
    expect(event.referrer).toBe('homepage')
    // Public-safe: no raw user agent, no per-request instant, no identity.
    expect(JSON.stringify(event)).not.toContain('SecretBuild')
    expect(Object.keys(event).toSorted()).toEqual([
      'channel', 'day', 'eventRef', 'format', 'outcome', 'referrer', 'schema', 'target', 'version',
    ])
  })

  test('unavailable outcomes are counted without version/target claims', async () => {
    const events: DesktopDownloadTelemetryEvent[] = []
    await makeResolver({ files: new Map(), events }).handle(request())
    expect(events).toHaveLength(1)
    expect(events[0]).toMatchObject({
      outcome: 'unavailable',
      version: null,
      target: null,
      format: null,
    })
  })

  test('artifact redirects are counted with their bound identity', async () => {
    const events: DesktopDownloadTelemetryEvent[] = []
    await makeResolver({ events }).handle(
      request({
        path: DESKTOP_DOWNLOAD_ARTIFACT_PATH,
        query: '?target=linux-x64&format=deb',
        headers: { referer: 'https://openagents.com/download' },
      }),
    )
    expect(events).toHaveLength(1)
    expect(events[0]).toMatchObject({
      outcome: 'artifact_redirect',
      target: 'linux-x64',
      format: 'deb',
      version: RC_VERSION,
      referrer: 'download-page',
    })
  })

  test('an invalid event is dropped by schema validation, never emitted', () => {
    const events: DesktopDownloadTelemetryEvent[] = []
    const accepted = emitDesktopDownloadTelemetry(
      {
        schema: DESKTOP_DOWNLOAD_TELEMETRY_SCHEMA_ID,
        eventRef: 'not-a-uuid',
        day: '2026-07-16',
        channel: 'rc',
        outcome: 'available',
        version: null,
        target: 'darwin-arm64',
        format: 'dmg',
        referrer: 'homepage',
      },
      event => events.push(event),
    )
    expect(accepted).toBe(false)
    expect(events).toHaveLength(0)
  })

  test('referrer categorization is bounded and deterministic', () => {
    expect(referrerCategory(null, 'openagents.com')).toBe('none')
    expect(referrerCategory('https://openagents.com/', 'openagents.com')).toBe('homepage')
    expect(referrerCategory('https://openagents.com/download', 'openagents.com')).toBe('download-page')
    expect(referrerCategory('https://openagents.com/docs', 'openagents.com')).toBe('internal')
    expect(referrerCategory('https://example.com/post', 'openagents.com')).toBe('external')
    expect(referrerCategory('::garbage::', 'openagents.com')).toBe('none')
  })
})

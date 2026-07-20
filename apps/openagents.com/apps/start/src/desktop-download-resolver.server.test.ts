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
 *
 * The feed harness below reproduces the REAL, now-landed `apps/oa-updates`
 * ReleaseSet v2 feed shape (`apps/oa-updates/src/release-set-feed.ts`):
 * a mutable pointer naming an immutable candidate by SHA-256 generation, with
 * the candidate responses carrying `x-openagents-release-generation`.
 */
import { createHash, generateKeyPairSync } from 'node:crypto'
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
  releaseSetCandidatePayloadPath,
  releaseSetCandidateSignaturePath,
  releaseSetPointerPath,
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

const sha256Hex = (bytes: Uint8Array): string => createHash('sha256').update(bytes).digest('hex')

// --- harness: real pointer + immutable-candidate feed shape ------------------

type FeedFile = string | Uint8Array | { body: string | Uint8Array; headers: Record<string, string> }
type Channel = 'stable' | 'rc'
const BASE = 'https://updates.fixture.test'

/**
 * `makeFetch` auto-derives `x-openagents-release-generation` from the
 * `candidates/<generation>/` path segment (exactly what the real feed does),
 * so ordinary tests never have to set it by hand. A test may override the
 * served body/headers explicitly (an object entry) to simulate a mismatched
 * or malicious origin.
 */
const makeFetch = (files: Map<string, FeedFile>): typeof fetch =>
  ((input: RequestInfo | URL) => {
    const pathname = new URL(String(input)).pathname
    const entry = files.get(pathname)
    if (entry === undefined) return Promise.resolve(new Response('not found', { status: 404 }))
    const isOverride =
      typeof entry === 'object' && !(entry instanceof Uint8Array) && 'body' in entry
    const body = isOverride ? (entry as { body: string | Uint8Array }).body : entry
    const candidateMatch = pathname.match(/\/candidates\/([0-9a-f]{64})\//)
    const autoHeaders: Record<string, string> =
      candidateMatch !== null ? { 'x-openagents-release-generation': candidateMatch[1]! } : {}
    const headers = isOverride
      ? { ...autoHeaders, ...(entry as { headers: Record<string, string> }).headers }
      : autoHeaders
    return Promise.resolve(new Response(body as BodyInit, { status: 200, headers }))
  }) as typeof fetch

/** Publish one signed candidate + its pointer into the feed file map. */
const publishCandidate = (
  files: Map<string, FeedFile>,
  channel: Channel,
  raw: string,
  options?: Readonly<{
    key?: ReleaseSigningKey
    revision?: number
    previousGeneration?: string | null
    publishedAt?: string
  }>,
): Readonly<{ generation: string; payload: Uint8Array; signature: string }> => {
  const { payload, signature } = sign(raw, options?.key)
  const signatureBytes = new TextEncoder().encode(signature)
  const generation = sha256Hex(payload)
  const pointer = {
    schema: 'openagents.desktop.release_pointer.v2',
    channel,
    revision: options?.revision ?? 1,
    generation,
    previousGeneration: options?.previousGeneration ?? null,
    payloadSha256: generation,
    signatureSha256: sha256Hex(signatureBytes),
    publishedAt: options?.publishedAt ?? '2026-07-16T11:00:00.000Z',
  }
  files.set(releaseSetPointerPath(channel), JSON.stringify(pointer))
  files.set(releaseSetCandidatePayloadPath(channel, generation), payload)
  files.set(releaseSetCandidateSignaturePath(channel, generation), signature)
  return { generation, payload, signature }
}

const rcFeed = (raw: string = fixtureRaw): Map<string, FeedFile> => {
  const files = new Map<string, FeedFile>()
  publishCandidate(files, 'rc', raw, { revision: 1 })
  return files
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

// --- resolution: four signed targets, alternatives, overrides ----------------
//
// Owner amendment 2026-07-20 (#8920, DIST-01): the signed ReleaseSet carries
// exactly the four mac+linux cells; `win32-x64` is an OPTIONAL experimental
// portable excluded from the signed feed, so a Windows host resolves to an
// honest target_unavailable state (covered below) rather than a signed
// artifact.

describe('resolution from the verified release set', () => {
  const targetMatrix: ReadonlyArray<[Record<string, string>, string, string]> = [
    [{ 'sec-ch-ua-platform': '"macOS"', 'sec-ch-ua-arch': '"arm"' }, 'darwin-arm64', 'dmg'],
    [{ 'sec-ch-ua-platform': '"macOS"', 'sec-ch-ua-arch': '"x86"', 'sec-ch-ua-bitness': '"64"' }, 'darwin-x64', 'dmg'],
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

  test('alternatives list same-target formats, then the FULL catalog of every other target', async () => {
    const { body } = await resolveJson(makeResolver(), {
      headers: { 'sec-ch-ua-platform': '"macOS"', 'sec-ch-ua-arch': '"arm"' },
    })
    const alternatives = body.alternatives as Array<Record<string, unknown>>
    // Signed catalog is 10 artifacts (darwin-arm64:2, darwin-x64:2,
    // linux-arm64:3, linux-x64:3) minus the 1 selected = 9. `win32-x64` is
    // optional/experimental and never enters the signed feed (#8920).
    expect(alternatives).toHaveLength(9)
    expect(alternatives[0]).toMatchObject({ target: 'darwin-arm64', format: 'zip' })
    // Every OTHER target's full format list is present — not just its
    // preferred format. A detected client must never have promoted formats
    // (Intel ZIP, Linux DEB/RPM, ...) silently hidden from its alternatives.
    const others = alternatives.slice(1)
    expect(others.map(row => `${row.target}:${row.format}`).toSorted()).toEqual(
      [
        'darwin-x64:dmg',
        'darwin-x64:zip',
        'linux-arm64:appimage',
        'linux-arm64:deb',
        'linux-arm64:rpm',
        'linux-x64:appimage',
        'linux-x64:deb',
        'linux-x64:rpm',
      ].toSorted(),
    )
    // No win32 artifact ever appears in the signed alternatives.
    expect(others.some(row => String(row.target).startsWith('win32'))).toBe(false)
    const preferredCount = others.filter(row => row.preferred === true).length
    expect(preferredCount).toBe(3) // one preferred format per other signed target
  })

  test('unknown client fails open to choose_manually with the full verified catalog', async () => {
    const { body } = await resolveJson(makeResolver())
    expect(body.availability).toBe('choose_manually')
    expect(body.reason).toBe('unknown_client')
    expect(body.selected).toBeUndefined()
    const options = body.options as Array<Record<string, unknown>>
    expect(options).toHaveLength(10)
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

  // #8920 (DIST-01): Windows on either architecture is detected but the signed
  // ReleaseSet carries no win32 cell, so both resolve to an honest
  // target_unavailable and no win32 artifact appears in the offered catalog.
  for (const [label, arch] of [
    ['x64', '"x86"'],
    ['ARM64', '"arm"'],
  ] as const) {
    test(`Windows ${label} is detected but never admitted as a signed release target`, async () => {
      const { body } = await resolveJson(makeResolver(), {
        headers: {
          'sec-ch-ua-platform': '"Windows"',
          'sec-ch-ua-arch': arch,
          ...(arch === '"x86"' ? { 'sec-ch-ua-bitness': '"64"' } : {}),
        },
      })
      expect(body.availability).toBe('choose_manually')
      expect(body.reason).toBe('target_unavailable')
      expect((body.detection as Record<string, unknown>).platform).toBe('win32')
      expect(body.selected).toBeUndefined()
      expect(
        (body.options as Array<Record<string, unknown>>).some(row =>
          String(row.target).startsWith('win32'),
        ),
      ).toBe(false)
    })
  }

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
    // darwin-arm64 offers dmg/zip, never deb — a globally valid format that is
    // absent for the selected target chooses manually instead of a wrong link.
    const { body } = await resolveJson(makeResolver(), {
      query: '?target=darwin-arm64&format=deb',
    })
    expect(body.availability).toBe('choose_manually')
    expect(body.reason).toBe('format_unavailable')
  })

  test('an explicit win32-x64 override is refused as an unknown target, never a signed link', async () => {
    // `win32-x64` is no longer a signed override target (#8920), so the typed
    // override schema rejects it as a malformed request rather than resolving a
    // signed artifact.
    const { response } = await resolveJson(makeResolver(), { query: '?target=win32-x64&format=nsis' })
    expect(response.status).toBe(400)
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
    const files = new Map<string, FeedFile>()
    publishCandidate(files, 'rc', fixtureRaw, { key: otherKey })
    const { body } = await resolveJson(makeResolver({ files }))
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('release_set_verification_failed')
  })

  test('a stable set served on the rc channel pointer is a channel mismatch', async () => {
    const files = new Map<string, FeedFile>()
    publishCandidate(files, 'rc', stableRaw)
    const { body } = await resolveJson(makeResolver({ files }))
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('release_set_verification_failed')
  })

  test('unparseable signature JSON is feed_schema_invalid', async () => {
    // The pointer's signatureSha256 must match the ACTUALLY served signature
    // bytes ('not json') or the SHA-256 binding check would reject it first
    // (as `release_candidate_mismatch`) before the JSON parser ever runs.
    const { payload } = sign(fixtureRaw)
    const generation = sha256Hex(payload)
    const unparseableSignature = 'not json'
    const files = new Map<string, FeedFile>([
      [
        releaseSetPointerPath('rc'),
        JSON.stringify({
          schema: 'openagents.desktop.release_pointer.v2',
          channel: 'rc',
          revision: 1,
          generation,
          previousGeneration: null,
          payloadSha256: generation,
          signatureSha256: sha256Hex(new TextEncoder().encode(unparseableSignature)),
          publishedAt: '2026-07-16T11:00:00.000Z',
        }),
      ],
      [releaseSetCandidatePayloadPath('rc', generation), payload],
      [releaseSetCandidateSignaturePath('rc', generation), unparseableSignature],
    ])
    const { body } = await resolveJson(makeResolver({ files }))
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('feed_schema_invalid')
  })

  test('a malformed pointer (missing field) is rejected before any candidate fetch', async () => {
    const files = new Map<string, FeedFile>()
    publishCandidate(files, 'rc', fixtureRaw)
    files.set(
      releaseSetPointerPath('rc'),
      JSON.stringify({ schema: 'openagents.desktop.release_pointer.v2', channel: 'rc' }),
    )
    const { body } = await resolveJson(makeResolver({ files }))
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('release_pointer_invalid')
  })

  test('a pointer naming a candidate that was never published is feed_unreachable', async () => {
    const files = new Map<string, FeedFile>()
    const { payload, signature } = sign(fixtureRaw)
    const generation = sha256Hex(payload)
    files.set(
      releaseSetPointerPath('rc'),
      JSON.stringify({
        schema: 'openagents.desktop.release_pointer.v2',
        channel: 'rc',
        revision: 1,
        generation,
        previousGeneration: null,
        payloadSha256: generation,
        signatureSha256: sha256Hex(new TextEncoder().encode(signature)),
        publishedAt: '2026-07-16T11:00:00.000Z',
      }),
    )
    // The candidate objects are never written to `files`.
    const { body } = await resolveJson(makeResolver({ files }))
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('feed_unreachable')
  })
})

// --- the required mutation test: tampered content cannot pass verification ---

describe('mutation test: tampered artifacts cannot pass verification', () => {
  // The attacker in this scenario controls BOTH the unsigned pointer AND the
  // storage layer (a compromised bucket, a cache-poisoning MITM) — everything
  // except the Ed25519 private key. They rewrite the payload's URL, hash, or
  // target, then relabel the unsigned pointer's generation/payloadSha256 to
  // match their own tampered bytes exactly (trivial, since nothing signs the
  // pointer itself) and reuse the ORIGINAL valid signature bytes/hash. If the
  // resolver only checked the SHA-256 pointer<->candidate binding, this would
  // pass. It must still be rejected by Ed25519 verification of the (now
  // mismatched) payload against the original signature.
  const mutations: ReadonlyArray<[string, string, string]> = [
    ['url', 'OpenAgents-2.4.0-rc.3-rc-darwin-arm64.dmg', 'OpenAgents-2.4.0-rc.3-rc-darwin-arm64-evil.dmg'],
    ['hash', '4b672044ba1e15584f48e3b7716454dc3c08f8badb1c7d05912e9db607dfbdca', 'f'.repeat(64)],
    // Canonical JSON is compact (no spaces around separators).
    ['target', '"target":"darwin-arm64"', '"target":"darwin-x64"'],
  ]
  for (const [field, needle, replacement] of mutations) {
    test(`a changed ${field} cannot pass verification even with a self-consistent pointer`, async () => {
      const { payload, signature } = sign(fixtureRaw)
      const mutatedPayload = new TextEncoder().encode(
        new TextDecoder().decode(payload).replace(needle, replacement),
      )
      expect(mutatedPayload).not.toEqual(payload)
      const signatureBytes = new TextEncoder().encode(signature)
      const mutatedGeneration = sha256Hex(mutatedPayload)

      const files = new Map<string, FeedFile>([
        [
          releaseSetPointerPath('rc'),
          JSON.stringify({
            schema: 'openagents.desktop.release_pointer.v2',
            channel: 'rc',
            revision: 1,
            generation: mutatedGeneration,
            previousGeneration: null,
            // Self-consistent: the attacker-controlled pointer matches the
            // attacker-controlled bytes exactly.
            payloadSha256: mutatedGeneration,
            signatureSha256: sha256Hex(signatureBytes),
            publishedAt: '2026-07-16T11:00:00.000Z',
          }),
        ],
        [releaseSetCandidatePayloadPath('rc', mutatedGeneration), mutatedPayload],
        [releaseSetCandidateSignaturePath('rc', mutatedGeneration), signature],
      ])
      const { body } = await resolveJson(makeResolver({ files }))
      expect(body.availability).toBe('unavailable')
      expect(body.reason).toBe('release_set_verification_failed')
      expect(JSON.stringify(body)).not.toContain('https://')
      expect(JSON.stringify(body)).not.toContain('evil')
    })
  }

  test('a candidate served at the wrong generation path (header mismatch) is rejected', async () => {
    // Two DIFFERENT valid, independently signed candidates. The pointer names
    // generation A, but the origin (compromised CDN cache/object swap) serves
    // candidate B's bytes at that path, with B's own generation header.
    const a = sign(fixtureRaw)
    const b = sign(rcNextRaw)
    const genA = sha256Hex(a.payload)
    const genB = sha256Hex(b.payload)
    const files = new Map<string, FeedFile>([
      [
        releaseSetPointerPath('rc'),
        JSON.stringify({
          schema: 'openagents.desktop.release_pointer.v2',
          channel: 'rc',
          revision: 1,
          generation: genA,
          previousGeneration: null,
          payloadSha256: genA,
          signatureSha256: sha256Hex(new TextEncoder().encode(a.signature)),
          publishedAt: '2026-07-16T11:00:00.000Z',
        }),
      ],
      // Served AT the genA path, but it is really candidate B's bytes, with
      // its own (mismatched) generation header — makeFetch's override form
      // lets us set that header explicitly.
      [
        releaseSetCandidatePayloadPath('rc', genA),
        { body: b.payload, headers: { 'x-openagents-release-generation': genB } },
      ],
      [releaseSetCandidateSignaturePath('rc', genA), a.signature],
    ])
    const { body } = await resolveJson(makeResolver({ files }))
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('release_candidate_mismatch')
  })

  test('a candidate whose bytes hash does not match the pointer is rejected even with a matching header', async () => {
    const { payload, signature } = sign(fixtureRaw)
    const generation = sha256Hex(payload)
    const tampered = new TextEncoder().encode(
      new TextDecoder().decode(payload).replace(RC_VERSION, '9.9.9-rc.9'),
    )
    const files = new Map<string, FeedFile>([
      [
        releaseSetPointerPath('rc'),
        JSON.stringify({
          schema: 'openagents.desktop.release_pointer.v2',
          channel: 'rc',
          revision: 1,
          generation,
          previousGeneration: null,
          payloadSha256: generation, // pins the ORIGINAL hash
          signatureSha256: sha256Hex(new TextEncoder().encode(signature)),
          publishedAt: '2026-07-16T11:00:00.000Z',
        }),
      ],
      // The header still (correctly) claims `generation`, but the actual
      // bytes served no longer hash to it.
      [
        releaseSetCandidatePayloadPath('rc', generation),
        { body: tampered, headers: { 'x-openagents-release-generation': generation } },
      ],
      [releaseSetCandidateSignaturePath('rc', generation), signature],
    ])
    const { body } = await resolveJson(makeResolver({ files }))
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('release_candidate_mismatch')
  })
})

// --- replay / rollback protection --------------------------------------------

describe('replay and rollback protection', () => {
  test('a pointer whose revision goes backward is rejected, not silently served', async () => {
    let now = NOW
    const files = rcFeed() // revision 1
    const resolver = makeResolver({ files, nowMs: () => now })
    const headers = { 'sec-ch-ua-platform': '"macOS"', 'sec-ch-ua-arch': '"arm"' }

    const first = await resolveJson(resolver, { headers })
    expect(first.body.version).toBe(RC_VERSION)

    // Promote forward once so the resolver has a revision-2 baseline.
    const first0 = publishCandidate(files, 'rc', fixtureRaw, { revision: 1 })
    publishCandidate(files, 'rc', rcNextRaw, {
      revision: 2,
      previousGeneration: first0.generation,
      publishedAt: '2026-07-16T11:05:00.000Z',
    })
    now += 61_000
    const forward = await resolveJson(resolver, { headers })
    expect(forward.body.version).toBe(RC_NEXT_VERSION)

    // Attacker/stale-cache replays the OLD revision-1 pointer+candidate.
    publishCandidate(files, 'rc', fixtureRaw, { revision: 1, publishedAt: '2026-07-16T11:00:00.000Z' })
    now += 61_000
    const replayed = await resolveJson(resolver, { headers })
    expect(replayed.body.availability).toBe('unavailable')
    expect(replayed.body.reason).toBe('release_pointer_replayed')
    expect(JSON.stringify(replayed.body)).not.toContain('https://')
  })

  test('a same-revision fork (different content, same revision number) is rejected', async () => {
    let now = NOW
    const files = rcFeed() // revision 1, some generation G1
    const resolver = makeResolver({ files, nowMs: () => now })
    await resolveJson(resolver) // establishes the TOFU baseline at revision 1

    // A different candidate claiming the SAME revision number (a fork/replay
    // of a sibling promotion) must not silently replace the trusted one.
    now += 61_000
    publishCandidate(files, 'rc', rcNextRaw, { revision: 1, publishedAt: '2026-07-16T11:00:00.000Z' })
    const { body } = await resolveJson(resolver)
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('release_pointer_replayed')
  })

  test('a broken previousGeneration chain at revision N+1 is rejected', async () => {
    let now = NOW
    const files = rcFeed() // revision 1
    const resolver = makeResolver({ files, nowMs: () => now })
    await resolveJson(resolver)

    now += 61_000
    // Revision 2 claims a previousGeneration that does not match the
    // resolver's actually-observed revision-1 generation.
    publishCandidate(files, 'rc', rcNextRaw, {
      revision: 2,
      previousGeneration: 'a'.repeat(64),
      publishedAt: '2026-07-16T11:05:00.000Z',
    })
    const { body } = await resolveJson(resolver)
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('release_pointer_replayed')
  })

  test('a first-ever pointer for a channel is trusted on sight (TOFU) once fully verified', async () => {
    const { body } = await resolveJson(makeResolver(), {
      headers: { 'sec-ch-ua-platform': '"macOS"', 'sec-ch-ua-arch': '"arm"' },
    })
    expect(body.availability).toBe('available')
    expect(body.version).toBe(RC_VERSION)
  })
})

// --- cache, freshness, pointer changes, concurrency ---------------------------

describe('cache, freshness, and concurrency', () => {
  test('pointer change: after the TTL the new set is served whole — never mixed', async () => {
    let now = NOW
    const files = rcFeed()
    const resolver = makeResolver({ files, nowMs: () => now })
    const headers = { 'sec-ch-ua-platform': '"macOS"', 'sec-ch-ua-arch': '"arm"' }

    const first = await resolveJson(resolver, { headers })
    expect(first.body.version).toBe(RC_VERSION)

    // Promotion happens: the channel pointer now serves rc.4.
    const initial = publishCandidate(files, 'rc', fixtureRaw, { revision: 1 })

    // Within the TTL the cached rc.3 snapshot is still served (consistent).
    now += 30_000
    const cached = await resolveJson(resolver, { headers })
    expect(cached.body.version).toBe(RC_VERSION)
    expect(String((cached.body.selected as Record<string, unknown>).url)).toContain(RC_VERSION)

    publishCandidate(files, 'rc', rcNextRaw, {
      revision: 2,
      previousGeneration: initial.generation,
      publishedAt: '2026-07-16T11:05:00.000Z',
    })

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

    files.clear() // feed goes down
    now += 61_000
    const { body } = await resolveJson(resolver, { headers })
    expect(body.availability).toBe('unavailable')
    expect(JSON.stringify(body)).not.toContain('https://')
  })

  test('channels are cached separately and never cross-serve', async () => {
    const files = rcFeed()
    publishCandidate(files, 'stable', stableRaw, { revision: 1 })
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

  test('concurrent revalidations singleflight — 8 callers share exactly one fetch chain', async () => {
    const files = rcFeed()
    let totalCalls = 0
    let inFlightFetches = 0
    let maxConcurrent = 0
    const baseFetch = makeFetch(files)
    const trackedFetch: typeof fetch = (async (...args: Parameters<typeof fetch>) => {
      totalCalls += 1
      inFlightFetches += 1
      maxConcurrent = Math.max(maxConcurrent, inFlightFetches)
      try {
        return await baseFetch(...args)
      } finally {
        inFlightFetches -= 1
      }
    }) as typeof fetch

    const resolver = createDesktopDownloadResolver({
      config: { baseUrl: BASE, defaultChannel: 'rc', pin, cacheTtlMs: 60_000 },
      fetchFn: trackedFetch,
      nowMs: () => NOW,
    })
    const headers = { 'sec-ch-ua-platform': '"macOS"', 'sec-ch-ua-arch': '"arm"' }

    // Fire 8 concurrent cold requests for the same channel; they must all
    // resolve to the SAME verified snapshot from ONE underlying fetch chain
    // (singleflight), never a torn mix and never 8 independent races.
    const results = await Promise.all(
      Array.from({ length: 8 }, () => resolveJson(resolver, { headers })),
    )
    for (const { body } of results) {
      expect(body.availability).toBe('available')
      expect(body.version).toBe(RC_VERSION)
    }
    // Exactly one pointer + one payload + one signature fetch for all 8
    // concurrent callers — proof of singleflight, not just bounded
    // concurrency.
    expect(totalCalls).toBe(3)
    expect(maxConcurrent).toBeLessThanOrEqual(2)
  })
})

// --- bounded body / stream error handling -------------------------------------

describe('bounded body and stream error handling', () => {
  test('an oversized pointer body is rejected without buffering it whole', async () => {
    const files = rcFeed()
    files.set(releaseSetPointerPath('rc'), 'x'.repeat(64 * 1024))
    const { body } = await resolveJson(makeResolver({ files }))
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('feed_schema_invalid')
  })

  test('a declared content-length larger than the cap is rejected before reading the body', async () => {
    const files = new Map<string, FeedFile>()
    publishCandidate(files, 'rc', fixtureRaw)
    files.set(releaseSetPointerPath('rc'), {
      body: JSON.stringify({ schema: 'openagents.desktop.release_pointer.v2' }),
      headers: { 'content-length': String(64 * 1024 * 1024) },
    })
    const { body } = await resolveJson(makeResolver({ files }))
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('feed_schema_invalid')
  })

  test('a streaming body read failure fails closed, never throws through the route', async () => {
    const files = rcFeed()
    const baseFetch = makeFetch(files)
    const failingFetch: typeof fetch = (async (input: RequestInfo | URL) => {
      const pathname = new URL(String(input)).pathname
      if (pathname === releaseSetPointerPath('rc')) {
        return new Response(
          new ReadableStream({
            start(controller) {
              controller.error(new Error('simulated network failure mid-stream'))
            },
          }),
          { status: 200 },
        )
      }
      return baseFetch(input)
    }) as typeof fetch
    const resolver = createDesktopDownloadResolver({
      config: { baseUrl: BASE, defaultChannel: 'rc', pin, cacheTtlMs: 60_000 },
      fetchFn: failingFetch,
      nowMs: () => NOW,
    })
    const { body } = await resolveJson(resolver)
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('feed_schema_invalid')
  })

  test('an oversized candidate payload is rejected', async () => {
    const files = new Map<string, FeedFile>()
    const { signature } = sign(fixtureRaw)
    const oversized = new TextEncoder().encode(
      JSON.stringify({ padding: 'x'.repeat(600 * 1024) }),
    )
    const generation = sha256Hex(oversized)
    files.set(
      releaseSetPointerPath('rc'),
      JSON.stringify({
        schema: 'openagents.desktop.release_pointer.v2',
        channel: 'rc',
        revision: 1,
        generation,
        previousGeneration: null,
        payloadSha256: generation,
        signatureSha256: sha256Hex(new TextEncoder().encode(signature)),
        publishedAt: '2026-07-16T11:00:00.000Z',
      }),
    )
    files.set(releaseSetCandidatePayloadPath('rc', generation), oversized)
    files.set(releaseSetCandidateSignaturePath('rc', generation), signature)
    const { body } = await resolveJson(makeResolver({ files }))
    expect(body.availability).toBe('unavailable')
    expect(body.reason).toBe('feed_schema_invalid')
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
    // rpm is a valid global format but darwin-arm64 never offers it.
    const response = await makeResolver().handle(
      request({
        path: DESKTOP_DOWNLOAD_ARTIFACT_PATH,
        query: '?target=darwin-arm64&format=rpm',
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

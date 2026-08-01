import {
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { hashPayloadBytes } from 'nostr-effect/nip98'
import { generateSecretKey, verifyEvent } from 'nostr-effect/pure'
import { describe, expect, test, vi } from 'vitest'

import {
  ACCEPTANCE_ROLES,
  type MintedCredential,
  acceptanceEnvNames,
  acceptanceSecretId,
  assertDistinctOwners,
  assertPublicSafeSummary,
  mintCredential,
  nip98Authorization,
  parseArguments,
  parseSessionResponse,
  renderEnvFile,
  resolveRepositoryRoot,
  summarize,
  writeCredentialFileExclusive,
} from './mint-livekit-acceptance-bearer'

// A syntactically valid, entirely fake session bearer. Nothing here ever
// reaches Secret Manager, gcloud, the network, or the filesystem.
const FAKE_BEARER = `oa_omega_${'a'.repeat(40)}`
const PRIVATE_USER_ID = `nostr:${'9f'.repeat(32)}`
const COMMUNITY_USER_ID = `nostr:${'3c'.repeat(32)}`

// A repository root that is NOT this worktree, so relative-path resolution in
// the test cannot accidentally land inside the root under test.
const REPOSITORY_ROOT = '/tmp/oa-mint-acceptance-bearer-repo'
const OUTSIDE_FILE = '/tmp/oa-mint-acceptance-bearer-outside/creds.env'

const sessionBody = (
  overrides: Record<string, unknown> = {},
  userOverrides: Record<string, unknown> = {},
): unknown => ({
  accessToken: FAKE_BEARER,
  expiresIn: 900,
  user: { userId: PRIVATE_USER_ID, provider: 'nostr', ...userOverrides },
  ...overrides,
})

const credential = (
  role: MintedCredential['role'],
  userId: string,
): MintedCredential => ({
  role,
  userId,
  ownerRef: userId,
  bearer: FAKE_BEARER,
  expiresInSeconds: 900,
})

describe('mint-livekit-acceptance-bearer parseArguments', () => {
  test('expands --role both into exactly the two acceptance roles', () => {
    const parsed = parseArguments(
      ['--role', 'both', '--credential-file', OUTSIDE_FILE],
      REPOSITORY_ROOT,
    )

    expect(parsed.roles).toEqual(['private', 'community'])
    expect(parsed.roles).toEqual(ACCEPTANCE_ROLES)
    expect(parsed.credentialFile).toBe(OUTSIDE_FILE)
    expect(parsed.baseUrl).toBe('https://openagents.com')
  })

  test.each(['private', 'community'])('accepts a single --role %s', role => {
    expect(
      parseArguments(
        ['--role', role, '--credential-file', OUTSIDE_FILE],
        REPOSITORY_ROOT,
      ).roles,
    ).toEqual([role])
  })

  test('rejects an unknown flag', () => {
    expect(() =>
      parseArguments(
        ['--role', 'both', '--credential-file', OUTSIDE_FILE, '--apply'],
        REPOSITORY_ROOT,
      ),
    ).toThrow(/unexpected argument: --apply/u)
  })

  test('rejects a missing or invalid --role', () => {
    expect(() =>
      parseArguments(['--credential-file', OUTSIDE_FILE], REPOSITORY_ROOT),
    ).toThrow(/--role is required/u)
    expect(() =>
      parseArguments(
        ['--role', 'owner', '--credential-file', OUTSIDE_FILE],
        REPOSITORY_ROOT,
      ),
    ).toThrow(/--role is required/u)
    expect(() => parseArguments(['--role'], REPOSITORY_ROOT)).toThrow(
      /--role is required/u,
    )
  })

  test('rejects a missing or empty --credential-file', () => {
    expect(() => parseArguments(['--role', 'both'], REPOSITORY_ROOT)).toThrow(
      /--credential-file is required/u,
    )
    expect(() =>
      parseArguments(
        ['--role', 'both', '--credential-file', ''],
        REPOSITORY_ROOT,
      ),
    ).toThrow(/--credential-file is required/u)
  })

  // A bearer written into the working tree is one `git add -A` away from being
  // published, so the guard is a path-boundary check, not a substring check.
  test('rejects a --credential-file that resolves inside the repository', () => {
    expect(() =>
      parseArguments(
        ['--role', 'both', '--credential-file', `${REPOSITORY_ROOT}/creds.env`],
        REPOSITORY_ROOT,
      ),
    ).toThrow(/must live outside the repository/u)
    expect(() =>
      parseArguments(
        [
          '--role',
          'both',
          '--credential-file',
          `${REPOSITORY_ROOT}/tmp/../nested/creds.env`,
        ],
        REPOSITORY_ROOT,
      ),
    ).toThrow(/must live outside the repository/u)
  })

  test('accepts a --credential-file that resolves outside the repository', () => {
    expect(
      parseArguments(
        ['--role', 'both', '--credential-file', OUTSIDE_FILE],
        REPOSITORY_ROOT,
      ).credentialFile,
    ).toBe(OUTSIDE_FILE)

    // Escaping the root with `..` is outside, and a sibling directory whose
    // name merely starts with the root is outside too.
    expect(
      parseArguments(
        [
          '--role',
          'both',
          '--credential-file',
          `${REPOSITORY_ROOT}/../oa-creds.env`,
        ],
        REPOSITORY_ROOT,
      ).credentialFile,
    ).toBe('/tmp/oa-creds.env')
    expect(
      parseArguments(
        [
          '--role',
          'both',
          '--credential-file',
          `${REPOSITORY_ROOT}-sibling/creds.env`,
        ],
        REPOSITORY_ROOT,
      ).credentialFile,
    ).toBe(`${REPOSITORY_ROOT}-sibling/creds.env`)
  })

  test('absolutizes the credential file before checking or writing it', () => {
    const parsed = parseArguments(
      ['--role', 'private', '--credential-file', 'oa-relative-creds.env'],
      REPOSITORY_ROOT,
    )

    expect(parsed.credentialFile.startsWith('/')).toBe(true)
    expect(parsed.credentialFile).toBe(resolve('oa-relative-creds.env'))
  })

  test('rejects a non-https or trailing-slash --base-url', () => {
    expect(() =>
      parseArguments(
        [
          '--role',
          'both',
          '--credential-file',
          OUTSIDE_FILE,
          '--base-url',
          'http://openagents.com',
        ],
        REPOSITORY_ROOT,
      ),
    ).toThrow(/--base-url must be an https origin/u)
    expect(() =>
      parseArguments(
        [
          '--role',
          'both',
          '--credential-file',
          OUTSIDE_FILE,
          '--base-url',
          'https://openagents.com/',
        ],
        REPOSITORY_ROOT,
      ),
    ).toThrow(/--base-url must be an https origin/u)
  })

  test('accepts an explicit https --base-url without a trailing slash', () => {
    expect(
      parseArguments(
        [
          '--role',
          'both',
          '--credential-file',
          OUTSIDE_FILE,
          '--base-url',
          'https://staging.openagents.com',
        ],
        REPOSITORY_ROOT,
      ).baseUrl,
    ).toBe('https://staging.openagents.com')
  })
})

describe('mint-livekit-acceptance-bearer names', () => {
  test('pins the acceptance Secret Manager ids', () => {
    expect(acceptanceSecretId('private')).toBe(
      'oa-livekit-acceptance-private-nostr-key',
    )
    expect(acceptanceSecretId('community')).toBe(
      'oa-livekit-acceptance-community-nostr-key',
    )
  })

  test('pins the exact env var names the acceptance CLI reads', () => {
    expect(acceptanceEnvNames('private')).toEqual({
      bearer: 'OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_BEARER',
      ownerRef: 'OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_OWNER_REF',
    })
    expect(acceptanceEnvNames('community')).toEqual({
      bearer: 'OA_SARAH_LIVEKIT_ACCEPTANCE_COMMUNITY_BEARER',
      ownerRef: 'OA_SARAH_LIVEKIT_ACCEPTANCE_COMMUNITY_OWNER_REF',
    })
  })

  // The producer and the consumer are in different packages, so a rename on
  // either side must break here rather than deep inside a live acceptance run.
  test('matches the names apps/sarah-livekit-agent/src/acceptance-cli.ts reads', () => {
    const acceptanceCli = readFileSync(
      fileURLToPath(
        new URL(
          '../../../../sarah-livekit-agent/src/acceptance-cli.ts',
          import.meta.url,
        ),
      ),
      'utf8',
    )

    for (const role of ACCEPTANCE_ROLES) {
      const names = acceptanceEnvNames(role)
      expect(acceptanceCli).toContain(`"${names.bearer}"`)
      expect(acceptanceCli).toContain(`"${names.ownerRef}"`)
    }
  })
})

describe('mint-livekit-acceptance-bearer parseSessionResponse', () => {
  test('accepts a well-formed response and derives the owner ref', () => {
    expect(parseSessionResponse('private', sessionBody())).toEqual({
      role: 'private',
      userId: PRIVATE_USER_ID,
      ownerRef: PRIVATE_USER_ID,
      bearer: FAKE_BEARER,
      expiresInSeconds: 900,
    })
  })

  /**
   * Pins the owner ref to the production authorization contract rather than to
   * whatever this tool happens to emit.
   *
   * `POST /api/omega/sarah/voice/admission` compares `identity.ownerRef`
   * against `userIdFromSession(session)` by strict equality and answers 403
   * `sarah_voice_identity_mismatch` otherwise. This tool once emitted
   * `agent:${userId}`, and the tests asserted that same decorated form, so the
   * suite was green while every minted bearer was refused at admission before a
   * room existed. Assert the undecorated identity, and assert the absence of a
   * prefix explicitly, so a future decoration cannot pass by agreeing with
   * itself again.
   */
  test('derives an owner ref the admission route will accept, undecorated', () => {
    const minted = parseSessionResponse('private', sessionBody())
    expect(minted.ownerRef).toBe(minted.userId)
    expect(minted.ownerRef.startsWith('agent:')).toBe(false)
    expect(minted.ownerRef).toMatch(/^nostr:/u)
  })

  test('rejects a response that is not an object', () => {
    expect(() => parseSessionResponse('private', null)).toThrow(
      /was not an object/u,
    )
    expect(() => parseSessionResponse('private', 'ok')).toThrow(
      /was not an object/u,
    )
  })

  test('rejects a response that carries no user', () => {
    expect(() =>
      parseSessionResponse('community', {
        accessToken: FAKE_BEARER,
        expiresIn: 900,
      }),
    ).toThrow(/carried no user/u)
  })

  test('rejects a bearer that is not an oa_omega_ session token', () => {
    expect(() =>
      parseSessionResponse(
        'private',
        sessionBody({ accessToken: 'oa_omega_' }),
      ),
    ).toThrow(/carried no oa_omega_ bearer/u)
    expect(() =>
      parseSessionResponse(
        'private',
        sessionBody({ accessToken: `oa_omega_${'a'.repeat(31)}` }),
      ),
    ).toThrow(/carried no oa_omega_ bearer/u)
    expect(() =>
      parseSessionResponse(
        'private',
        sessionBody({ accessToken: `nsec1${'a'.repeat(40)}` }),
      ),
    ).toThrow(/carried no oa_omega_ bearer/u)
    expect(() =>
      parseSessionResponse('private', sessionBody({ accessToken: 12 })),
    ).toThrow(/carried no oa_omega_ bearer/u)
  })

  test('rejects a provider that is not nostr', () => {
    expect(() =>
      parseSessionResponse('private', sessionBody({}, { provider: 'github' })),
    ).toThrow(/session provider was github, expected nostr/u)
    expect(() =>
      parseSessionResponse('private', sessionBody({}, { provider: undefined })),
    ).toThrow(/session provider was undefined, expected nostr/u)
  })

  test('rejects an empty or non-string userId', () => {
    expect(() =>
      parseSessionResponse('private', sessionBody({}, { userId: '' })),
    ).toThrow(/carried no userId/u)
    expect(() =>
      parseSessionResponse('private', sessionBody({}, { userId: 7 })),
    ).toThrow(/carried no userId/u)
  })

  test('rejects an absent or non-positive expiresIn', () => {
    expect(() =>
      parseSessionResponse('private', sessionBody({ expiresIn: undefined })),
    ).toThrow(/carried no positive expiresIn/u)
    expect(() =>
      parseSessionResponse('private', sessionBody({ expiresIn: 0 })),
    ).toThrow(/carried no positive expiresIn/u)
    expect(() =>
      parseSessionResponse('private', sessionBody({ expiresIn: -900 })),
    ).toThrow(/carried no positive expiresIn/u)
    expect(() =>
      parseSessionResponse('private', sessionBody({ expiresIn: '900' })),
    ).toThrow(/carried no positive expiresIn/u)
    expect(() =>
      parseSessionResponse(
        'private',
        sessionBody({ expiresIn: Number.POSITIVE_INFINITY }),
      ),
    ).toThrow(/carried no positive expiresIn/u)
  })
})

describe('mint-livekit-acceptance-bearer assertDistinctOwners', () => {
  test('passes for two distinct authenticated owners', () => {
    expect(() =>
      assertDistinctOwners([
        credential('private', PRIVATE_USER_ID),
        credential('community', COMMUNITY_USER_ID),
      ]),
    ).not.toThrow()
  })

  // The concurrent two-room matrix needs two owners. One owner used twice is a
  // preflight error, not a live-run discovery.
  test('throws when both identities resolve to the same owner', () => {
    expect(() =>
      assertDistinctOwners([
        credential('private', PRIVATE_USER_ID),
        credential('community', PRIVATE_USER_ID),
      ]),
    ).toThrow(/two distinct owners/u)
  })
})

describe('mint-livekit-acceptance-bearer renderEnvFile', () => {
  test('emits exactly the comment header and four unquoted KEY=value lines', () => {
    const rendered = renderEnvFile([
      credential('private', PRIVATE_USER_ID),
      credential('community', COMMUNITY_USER_ID),
    ])

    expect(rendered.endsWith('\n')).toBe(true)
    expect(rendered.split('\n')).toEqual([
      '# Short-lived Sarah LiveKit acceptance bearers. Mode 0600, outside the',
      '# repository, expires in minutes. Do not commit, paste, or forward.',
      `OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_BEARER=${FAKE_BEARER}`,
      `OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_OWNER_REF=${PRIVATE_USER_ID}`,
      `OA_SARAH_LIVEKIT_ACCEPTANCE_COMMUNITY_BEARER=${FAKE_BEARER}`,
      `OA_SARAH_LIVEKIT_ACCEPTANCE_COMMUNITY_OWNER_REF=${COMMUNITY_USER_ID}`,
      '',
    ])
    // `set -a; . file` must see the raw values, so no quoting of any kind.
    expect(rendered).not.toContain('"')
    expect(rendered).not.toContain("'")
    expect(rendered).not.toContain('export ')
  })

  test('emits only the requested role', () => {
    const rendered = renderEnvFile([credential('private', PRIVATE_USER_ID)])

    expect(rendered).toContain('OA_SARAH_LIVEKIT_ACCEPTANCE_PRIVATE_BEARER=')
    expect(rendered).not.toContain('COMMUNITY')
  })
})

describe('mint-livekit-acceptance-bearer side effects', () => {
  test('creates one owner-only file and refuses overwrite or symlink replacement', () => {
    const directory = mkdtempSync(join(tmpdir(), 'oa-livekit-credential-'))
    const credentialFile = join(directory, 'acceptance.env')
    const symlinkTarget = join(directory, 'target.env')
    const symlinkFile = join(directory, 'credential-link.env')
    try {
      writeCredentialFileExclusive(credentialFile, 'FIRST=value\n')
      expect(readFileSync(credentialFile, 'utf8')).toBe('FIRST=value\n')
      expect(statSync(credentialFile).mode & 0o777).toBe(0o600)

      expect(() =>
        writeCredentialFileExclusive(credentialFile, 'SECOND=value\n'),
      ).toThrow()
      expect(readFileSync(credentialFile, 'utf8')).toBe('FIRST=value\n')

      writeFileSync(symlinkTarget, 'TARGET=value\n', { mode: 0o644 })
      // Use the platform API here: the production `wx` open must reject this
      // final-component symlink rather than following it.
      symlinkSync(symlinkTarget, symlinkFile)
      expect(() =>
        writeCredentialFileExclusive(symlinkFile, 'SECRET=value\n'),
      ).toThrow()
      expect(readFileSync(symlinkTarget, 'utf8')).toBe('TARGET=value\n')
      expect(statSync(symlinkTarget).mode & 0o777).toBe(0o644)
    } finally {
      rmSync(directory, { recursive: true, force: true })
    }
  })

  test('decodes URL-escaped repository paths before applying path guards', () => {
    const encodedScriptUrl = pathToFileURL(
      '/tmp/openagents path/apps/openagents.com/workers/api/scripts/tool.ts',
    ).href

    expect(encodedScriptUrl).toContain('%20')
    expect(resolveRepositoryRoot(encodedScriptUrl)).toBe(
      '/tmp/openagents path/',
    )
  })

  test('signs exactly one POST session request with a verifiable NIP-98 proof', async () => {
    const secret = generateSecretKey()
    const createdAtSeconds = 1_785_513_600
    const requestInputs: Array<Readonly<{ url: string; init?: RequestInit }>> =
      []
    const fetchMock = vi.fn(
      async (input: string | URL | Request, init?: RequestInit) => {
        requestInputs.push({ url: String(input), init })
        return new Response(JSON.stringify(sessionBody()), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      },
    )
    const readSecret = vi.fn(() => Buffer.from(secret).toString('hex'))

    await expect(
      mintCredential('private', 'https://openagents.com', {
        readSecret,
        fetch: fetchMock as typeof fetch,
        nowSeconds: () => createdAtSeconds,
      }),
    ).resolves.toMatchObject({
      role: 'private',
      ownerRef: PRIVATE_USER_ID,
      bearer: FAKE_BEARER,
    })

    expect(readSecret).toHaveBeenCalledOnce()
    expect(fetchMock).toHaveBeenCalledOnce()
    expect(requestInputs).toHaveLength(1)
    const request = requestInputs[0]
    expect(request?.url).toBe('https://openagents.com/api/omega/auth/session')
    expect(request?.init?.method).toBe('POST')
    expect(request?.init?.body).toBeInstanceOf(Uint8Array)
    const headers = new Headers(request?.init?.headers)
    const authorization = headers.get('authorization')
    expect(authorization).toMatch(/^Nostr /u)
    const event = JSON.parse(
      Buffer.from(
        authorization?.slice('Nostr '.length) ?? '',
        'base64',
      ).toString('utf8'),
    ) as Parameters<typeof verifyEvent>[0]
    expect(verifyEvent(event)).toBe(true)
    expect(event.kind).toBe(27_235)
    expect(event.created_at).toBe(createdAtSeconds)
    expect(event.tags).toEqual([
      ['u', 'https://openagents.com/api/omega/auth/session'],
      ['method', 'POST'],
      ['payload', hashPayloadBytes(new Uint8Array())],
    ])
  })

  test('pins the pure NIP-98 helper to its supplied instant', () => {
    const secret = generateSecretKey()
    const authorization = nip98Authorization(
      secret,
      'https://openagents.com/api/omega/auth/session',
      new Uint8Array(),
      1_785_513_600,
    )
    const event = JSON.parse(
      Buffer.from(authorization.slice('Nostr '.length), 'base64').toString(
        'utf8',
      ),
    ) as Parameters<typeof verifyEvent>[0]

    expect(verifyEvent(event)).toBe(true)
    expect(event.created_at).toBe(1_785_513_600)
  })
})

describe('mint-livekit-acceptance-bearer public-safe summaries', () => {
  const now = Date.parse('2026-07-31T12:00:00.000Z')

  test('computes expiresAt from the supplied clock and never carries the bearer', () => {
    const minted = credential('private', PRIVATE_USER_ID)
    const summary = summarize(minted, now)

    expect(summary).toEqual({
      role: 'private',
      userId: PRIVATE_USER_ID,
      ownerRef: PRIVATE_USER_ID,
      bearerDigest: expect.stringMatching(/^[0-9a-f]{64}$/u),
      expiresInSeconds: 900,
      expiresAt: '2026-07-31T12:15:00.000Z',
    })
    expect(JSON.stringify(summary)).not.toContain(minted.bearer)
  })

  test('throws when a summary somehow carries the bearer or secret material', () => {
    const minted = credential('private', PRIVATE_USER_ID)

    expect(() =>
      assertPublicSafeSummary(
        { ...summarize(minted, now), ownerRef: minted.bearer },
        minted,
      ),
    ).toThrow(/containing the bearer/u)
    expect(() =>
      assertPublicSafeSummary(
        { ...summarize(minted, now), userId: `oa_omega_${'b'.repeat(40)}` },
        minted,
      ),
    ).toThrow(/containing secret material/u)
    expect(() =>
      assertPublicSafeSummary(
        { ...summarize(minted, now), userId: `nsec1${'q'.repeat(58)}` },
        minted,
      ),
    ).toThrow(/containing secret material/u)
  })

  // Regression pin, live run 2026-07-31: an over-broad "any 64-hex string is
  // secret" check rejected every normal summary. A Nostr public key and the
  // bearer's SHA-256 digest are public by construction — reporting the digest
  // is the whole point — so a clean summary must pass.
  test('does not reject a normal summary whose userId is a nostr public key', () => {
    for (const [role, userId] of [
      ['private', PRIVATE_USER_ID],
      ['community', COMMUNITY_USER_ID],
    ] as const) {
      const minted = credential(role, userId)
      const summary = summarize(minted, now)

      expect(summary.userId).toMatch(/^nostr:[0-9a-f]{64}$/u)
      expect(summary.bearerDigest).toMatch(/^[0-9a-f]{64}$/u)
      expect(() => assertPublicSafeSummary(summary, minted)).not.toThrow()
    }
  })
})

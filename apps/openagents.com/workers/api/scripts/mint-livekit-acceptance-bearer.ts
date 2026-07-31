#!/usr/bin/env -S pnpm exec tsx
// Mint the short-lived acceptance bearers the two-room Sarah LiveKit
// acceptance needs (EP263-LK H1, #9282).
//
// Every passing acceptance run so far required hand-building a throwaway
// minting helper, running it, and deleting it. That is unreviewable, it makes
// each run a slightly different procedure, and a deleted helper is not
// evidence. This is the reviewed, repeatable, checked-in path.
//
// What it does, and nothing else:
//   1. reads a Nostr acceptance key from Secret Manager, in memory only;
//   2. signs one NIP-98 kind-27235 event over POST /api/omega/auth/session;
//   3. exchanges it for an `oa_omega_…` session bearer with a 15 minute TTL;
//   4. writes the bearer to a mode-0600 file OUTSIDE the repository.
//
// It creates NO durable identity rows. Session bearers live in the AuthKV
// store under a TTL and expire on their own, so there is nothing to delete
// afterwards — an identity the tool had to create and then remove would be a
// worse design than one it never creates. The durable acceptance identities
// (users, auth_identities, agent_balances, sarah_voice_alpha_memberships) are
// admitted separately and deliberately by admit-sarah-voice-npub.ts, and must
// persist between runs; this tool never writes to Postgres at all.
//
// It never prints the Nostr secret and never prints the bearer. Standard
// output is public-safe metadata: the owner ref, the expiry, and a SHA-256
// digest of the bearer so a receipt can name the credential without carrying
// it.
//
// Usage:
//   OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
//   pnpm exec tsx apps/openagents.com/workers/api/scripts/mint-livekit-acceptance-bearer.ts \
//     --role both --credential-file /tmp/oa-livekit-acceptance.env
//
//   set -a; . /tmp/oa-livekit-acceptance.env; set +a
//   pnpm --dir apps/sarah-livekit-agent acceptance -- --apply …
//
// Bearers expire in 15 minutes. Run this immediately before the acceptance.

import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { hashPayloadBytes } from 'nostr-effect/nip98'
import { finalizeEvent } from 'nostr-effect/pure'

import { parseSecretMaterial } from '@openagentsinc/sarah/nostr-identity'

/** The same cost/mutation gate every other EP263-LK operator tool requires. */
export const OWNER_GATE = 'I_ACCEPT_EP263_LIVEKIT_GCP_COST'

export const GCP_PROJECT = 'openagentsgemini'

export const ACCEPTANCE_ROLES = ['private', 'community'] as const

export type AcceptanceRole = (typeof ACCEPTANCE_ROLES)[number]

/** Secret Manager ids. These are acceptance-only identities, never Sarah's. */
export const acceptanceSecretId = (role: AcceptanceRole): string =>
  `oa-livekit-acceptance-${role}-nostr-key`

/** The env var names apps/sarah-livekit-agent/src/acceptance-cli.ts reads. */
export const acceptanceEnvNames = (
  role: AcceptanceRole,
): Readonly<{ bearer: string; ownerRef: string }> => ({
  bearer: `OA_SARAH_LIVEKIT_ACCEPTANCE_${role.toUpperCase()}_BEARER`,
  ownerRef: `OA_SARAH_LIVEKIT_ACCEPTANCE_${role.toUpperCase()}_OWNER_REF`,
})

/** packages/audio-contract pins this shape; refuse anything else. */
export const OMEGA_SESSION_TOKEN_PATTERN = /^oa_omega_[A-Za-z0-9_-]{32,256}$/u

export type MintedCredential = Readonly<{
  role: AcceptanceRole
  userId: string
  ownerRef: string
  bearer: string
  expiresInSeconds: number
}>

export type CredentialSummary = Readonly<{
  role: AcceptanceRole
  userId: string
  ownerRef: string
  bearerDigest: string
  expiresInSeconds: number
  expiresAt: string
}>

export type ParsedArguments = Readonly<{
  roles: ReadonlyArray<AcceptanceRole>
  credentialFile: string
  baseUrl: string
}>

const USAGE =
  'usage: mint-livekit-acceptance-bearer.ts --role private|community|both ' +
  '--credential-file <absolute path outside the repository> [--base-url URL]'

export const parseArguments = (
  argv: ReadonlyArray<string>,
  repositoryRoot: string,
): ParsedArguments => {
  let role: string | undefined
  let credentialFile: string | undefined
  let baseUrl = 'https://openagents.com'

  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index]
    switch (token) {
      case '--role':
        index += 1
        role = argv[index]
        break
      case '--credential-file':
        index += 1
        credentialFile = argv[index]
        break
      case '--base-url':
        index += 1
        baseUrl = argv[index] ?? ''
        break
      default:
        throw new Error(`${USAGE}\nunexpected argument: ${token}`)
    }
  }

  if (role === undefined || !['private', 'community', 'both'].includes(role)) {
    throw new Error(`${USAGE}\n--role is required`)
  }
  if (credentialFile === undefined || credentialFile.length === 0) {
    throw new Error(`${USAGE}\n--credential-file is required`)
  }
  const resolvedEnvFile = resolve(credentialFile)
  // A bearer written into the working tree is one `git add -A` away from being
  // published. The acceptance CLI applies the same rule to its PCM prompts.
  if (resolvedEnvFile.startsWith(`${resolve(repositoryRoot)}/`)) {
    throw new Error(
      `${USAGE}\n--credential-file must live outside the repository: ${resolvedEnvFile}`,
    )
  }
  if (!/^https:\/\/[^\s]+$/u.test(baseUrl) || baseUrl.endsWith('/')) {
    throw new Error(`${USAGE}\n--base-url must be an https origin: ${baseUrl}`)
  }

  return {
    roles: role === 'both' ? ACCEPTANCE_ROLES : [role as AcceptanceRole],
    credentialFile: resolvedEnvFile,
    baseUrl,
  }
}

export const sha256Hex = (value: Uint8Array | string): string =>
  createHash('sha256').update(value).digest('hex')

export const nip98Authorization = (
  secret: Uint8Array,
  url: string,
  payload: Uint8Array,
): string => {
  const event = finalizeEvent(
    {
      content: '',
      created_at: Math.floor(Date.now() / 1_000),
      kind: 27_235,
      tags: [
        ['u', url],
        ['method', 'POST'],
        ['payload', hashPayloadBytes(payload)],
      ],
    },
    secret,
  )
  return `Nostr ${Buffer.from(JSON.stringify(event), 'utf8').toString('base64')}`
}

/**
 * Reject a session response that is not exactly what the acceptance needs. A
 * bearer that is subtly wrong fails deep inside a live run, where the failure
 * is expensive and reads like an infrastructure fault.
 */
export const parseSessionResponse = (
  role: AcceptanceRole,
  body: unknown,
): MintedCredential => {
  if (typeof body !== 'object' || body === null) {
    throw new Error(`${role}: session response was not an object`)
  }
  const record = body as Record<string, unknown>
  const user = record.user
  if (typeof user !== 'object' || user === null) {
    throw new Error(`${role}: session response carried no user`)
  }
  const userRecord = user as Record<string, unknown>
  const bearer = record.accessToken
  const userId = userRecord.userId
  const provider = userRecord.provider
  const expiresIn = record.expiresIn

  if (typeof bearer !== 'string' || !OMEGA_SESSION_TOKEN_PATTERN.test(bearer)) {
    throw new Error(`${role}: session response carried no oa_omega_ bearer`)
  }
  if (provider !== 'nostr') {
    throw new Error(
      `${role}: session provider was ${String(provider)}, expected nostr`,
    )
  }
  if (typeof userId !== 'string' || userId.length === 0) {
    throw new Error(`${role}: session response carried no userId`)
  }
  if (
    typeof expiresIn !== 'number' ||
    !Number.isFinite(expiresIn) ||
    expiresIn <= 0
  ) {
    throw new Error(`${role}: session response carried no positive expiresIn`)
  }

  return {
    role,
    userId,
    ownerRef: `agent:${userId}`,
    bearer,
    expiresInSeconds: expiresIn,
  }
}

/**
 * The concurrent two-room matrix needs two distinct authenticated owners; one
 * owner used twice is a preflight error in the harness. Catch it here, before
 * a live run spends provider time discovering it.
 */
export const assertDistinctOwners = (
  credentials: ReadonlyArray<MintedCredential>,
): void => {
  const owners = new Set(credentials.map(credential => credential.ownerRef))
  if (owners.size !== credentials.length) {
    throw new Error(
      'the private and community acceptance identities resolved to the same ' +
        'owner; the concurrent matrix requires two distinct owners',
    )
  }
}

export const renderEnvFile = (
  credentials: ReadonlyArray<MintedCredential>,
): string => {
  const lines = [
    '# Short-lived Sarah LiveKit acceptance bearers. Mode 0600, outside the',
    '# repository, expires in minutes. Do not commit, paste, or forward.',
  ]
  for (const credential of credentials) {
    const names = acceptanceEnvNames(credential.role)
    lines.push(`${names.bearer}=${credential.bearer}`)
    lines.push(`${names.ownerRef}=${credential.ownerRef}`)
  }
  return `${lines.join('\n')}\n`
}

export const summarize = (
  credential: MintedCredential,
  now: number,
): CredentialSummary => ({
  role: credential.role,
  userId: credential.userId,
  ownerRef: credential.ownerRef,
  bearerDigest: sha256Hex(credential.bearer),
  expiresInSeconds: credential.expiresInSeconds,
  expiresAt: new Date(
    now + credential.expiresInSeconds * 1_000,
  ).toISOString(),
})

/**
 * Nothing this tool prints may carry credential material. The acceptance
 * harness applies the same rule to its receipts.
 */
export const assertPublicSafeSummary = (
  summary: CredentialSummary,
  credential: MintedCredential,
): void => {
  const serialized = JSON.stringify(summary)
  if (serialized.includes(credential.bearer)) {
    throw new Error('refusing to print a summary containing the bearer')
  }
  // `oa_omega_` is the session-token prefix and `nsec1` is Nostr secret
  // material; neither may appear. The 64-hex strings that DO appear — the
  // user's public key inside `nostr:<pubkey>` and the bearer digest — are
  // public by construction, which is the point of reporting a digest.
  if (/oa_omega_|nsec1/u.test(serialized)) {
    throw new Error('refusing to print a summary containing secret material')
  }
}

const readAcceptanceSecret = (role: AcceptanceRole): string =>
  execFileSync(
    'gcloud',
    [
      'secrets',
      'versions',
      'access',
      'latest',
      '--secret',
      acceptanceSecretId(role),
      '--project',
      GCP_PROJECT,
    ],
    { encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] },
  )

const mint = async (
  role: AcceptanceRole,
  baseUrl: string,
): Promise<MintedCredential> => {
  const url = `${baseUrl}/api/omega/auth/session`
  const payload = new Uint8Array()
  let secret: Uint8Array | undefined
  try {
    secret = parseSecretMaterial(readAcceptanceSecret(role))
    const authorization = nip98Authorization(secret, url, payload)
    const response = await fetch(url, {
      method: 'POST',
      headers: { authorization },
      body: payload,
    })
    if (!response.ok) {
      // The body may echo request detail; report status only.
      throw new Error(
        `${role}: ${url} refused the NIP-98 proof with status ${response.status}`,
      )
    }
    return parseSessionResponse(role, await response.json())
  } finally {
    secret?.fill(0)
  }
}

const main = async (): Promise<void> => {
  if (process.env.OA_LIVEKIT_OWNER_GATE !== OWNER_GATE) {
    throw new Error(
      `set OA_LIVEKIT_OWNER_GATE=${OWNER_GATE} to mint acceptance credentials`,
    )
  }

  const repositoryRoot = resolve(
    new URL('../../../../..', import.meta.url).pathname,
  )
  const parsed = parseArguments(process.argv.slice(2), repositoryRoot)

  const credentials: Array<MintedCredential> = []
  for (const role of parsed.roles) {
    credentials.push(await mint(role, parsed.baseUrl))
  }
  if (credentials.length > 1) {
    assertDistinctOwners(credentials)
  }

  writeFileSync(parsed.credentialFile, renderEnvFile(credentials), {
    encoding: 'utf8',
    mode: 0o600,
  })

  const now = Date.now()
  const summaries = credentials.map(credential => {
    const summary = summarize(credential, now)
    assertPublicSafeSummary(summary, credential)
    return summary
  })

  console.log(
    JSON.stringify(
      {
        schema: 'openagents.livekit_acceptance_credentials.v1',
        baseUrl: parsed.baseUrl,
        credentialFile: parsed.credentialFile,
        credentials: summaries,
      },
      undefined,
      2,
    ),
  )
}

// Only run when invoked as the process entrypoint, so the pure helpers above
// stay importable (and testable) without minting anything.
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  await main()
}

#!/usr/bin/env -S pnpm exec tsx
// Admit (or remove) one Nostr identity in the Sarah LiveKit community group,
// by publishing the NIP-29 admin record the community access resolver folds.
//
// Why this exists. `SARAH_LIVEKIT_COMMUNITY_AUTHORITY_JSON` names the group,
// its relay, and the admin pubkeys whose kind-9000/9001 records are allowed to
// move the membership ledger (see `sarah-livekit-community-access.ts`). Nothing
// in the repository could actually WRITE one of those records, so community
// admission was an undocumented manual step, and one release-gate lane
// concluded the admin key was lost. It was not: the configured admin pubkey is
// the one derived from Secret Manager's `openagents-nostr-relay-private-key`.
// A capability that only exists in someone's shell history is indistinguishable
// from a capability that does not exist, which is why it is checked in here.
//
// What it does, and nothing else:
//   1. reads a group admin key from Secret Manager, in memory only;
//   2. verifies that key's pubkey is actually listed in the deployed
//      community authority config, refusing early otherwise;
//   3. opens the configured relay, completes the NIP-42 AUTH challenge, and
//      publishes ONE kind-9000 (`put-user`) or kind-9001 (`remove-user`)
//      record naming the subject pubkey;
//   4. prints public-safe metadata: the event id, the signer pubkey, the
//      subject, and the relay's OK frame.
//
// It never prints a secret key. It writes nothing to Postgres — the alpha
// cohort row is a separate, deliberately separate admission and belongs to
// `admit-sarah-voice-npub.ts`. Community membership and voice-cohort
// membership are two different authorities and must not collapse into one
// script.
//
// Usage:
//   OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST \
//   pnpm exec tsx apps/openagents.com/workers/api/scripts/admit-sarah-livekit-community-npub.ts \
//     --community openagents-public --npub npub1… --role member --apply
//
//   … --action remove --npub npub1… --apply     # kind 9001, immediate
//
// Without `--apply` it resolves everything, prints the record it WOULD sign,
// and connects to nothing.

import * as nip19 from 'nostr-effect/nip19'
import { finalizeEvent } from 'nostr-effect/pure'
import { execFileSync } from 'node:child_process'

/** The same cost/mutation gate every other EP263-LK operator tool requires. */
export const OWNER_GATE = 'I_ACCEPT_EP263_LIVEKIT_GCP_COST'

export const GCP_PROJECT = 'openagentsgemini'

/**
 * The group admin key. This is the relay's own key, which is also the pubkey
 * the deployed authority config lists as a group admin. Passing a different
 * secret id is supported so a purpose-built admin key can take over without
 * editing this file.
 */
export const DEFAULT_ADMIN_SECRET_ID = 'openagents-nostr-relay-private-key'

export const NIP_29_PUT_USER_KIND = 9_000
export const NIP_29_REMOVE_USER_KIND = 9_001
export const NIP_42_AUTH_KIND = 22_242

const RELAY_TIMEOUT_MS = 20_000

export type CommunityAdmissionArguments = Readonly<{
  communityRef: string
  npub: string
  role: string
  action: 'admit' | 'remove'
  adminSecretId: string
  apply: boolean
}>

const USAGE =
  'usage: admit-sarah-livekit-community-npub.ts --npub NPUB ' +
  '[--community REF] [--role member|admin] [--action admit|remove] ' +
  '[--admin-secret SECRET_ID] [--apply]'

export const parseArguments = (
  argv: ReadonlyArray<string>,
): CommunityAdmissionArguments => {
  let communityRef = 'openagents-public'
  let npub: string | undefined
  let role = 'member'
  let action: 'admit' | 'remove' = 'admit'
  let adminSecretId = DEFAULT_ADMIN_SECRET_ID
  let apply = false

  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index]
    if (token === '--') continue
    switch (token) {
      case '--community':
        index += 1
        communityRef = argv[index] ?? ''
        break
      case '--npub':
        index += 1
        npub = argv[index]
        break
      case '--role':
        index += 1
        role = argv[index] ?? ''
        break
      case '--action': {
        index += 1
        const value = argv[index]
        if (value !== 'admit' && value !== 'remove') {
          throw new Error(USAGE)
        }
        action = value
        break
      }
      case '--admin-secret':
        index += 1
        adminSecretId = argv[index] ?? ''
        break
      case '--apply':
        apply = true
        break
      default:
        throw new Error(`${USAGE}\nunsupported argument ${String(token)}`)
    }
  }

  if (
    npub === undefined ||
    communityRef.trim() === '' ||
    role.trim() === '' ||
    adminSecretId.trim() === ''
  ) {
    throw new Error(USAGE)
  }
  return {
    communityRef: communityRef.trim(),
    npub,
    role: role.trim(),
    action,
    adminSecretId: adminSecretId.trim(),
    apply,
  }
}

/** One canonical npub, decoded to a lowercase 32-byte hex pubkey. */
export const canonicalPubkey = (npub: string): string => {
  const value = npub.trim()
  if (/^[0-9a-f]{64}$/u.test(value)) return value
  try {
    const decoded = nip19.decode(value)
    if (
      decoded.type !== 'npub' ||
      typeof decoded.data !== 'string' ||
      nip19.npubEncode(decoded.data) !== value
    ) {
      throw new Error('not canonical')
    }
    return decoded.data.toLowerCase()
  } catch {
    throw new Error('the Nostr identity must be one canonical npub or 64-hex pubkey')
  }
}

export type CommunityAuthorityEntry = Readonly<{
  communityRef: string
  relayUrl: string
  adminPubkeys: ReadonlyArray<string>
}>

/**
 * Resolve the deployed community authority so this tool cannot sign a record
 * the API would then refuse to fold. An admission that the relay stores and
 * the resolver ignores is the worst of both worlds: it looks like it worked.
 */
export const resolveCommunityAuthority = (
  authorityJson: string,
  communityRef: string,
): CommunityAuthorityEntry => {
  const parsed: unknown = JSON.parse(authorityJson)
  const communities =
    typeof parsed === 'object' && parsed !== null && 'communities' in parsed
      ? (parsed as { communities: unknown }).communities
      : undefined
  if (!Array.isArray(communities)) {
    throw new Error('the community authority config has no communities')
  }
  const entry = communities.find(
    (candidate): candidate is CommunityAuthorityEntry =>
      typeof candidate === 'object' &&
      candidate !== null &&
      (candidate as { communityRef?: unknown }).communityRef === communityRef,
  )
  if (entry === undefined) {
    throw new Error(`the deployed authority config does not name ${communityRef}`)
  }
  return entry
}

const readSecret = (secretId: string): string =>
  execFileSync(
    'gcloud',
    [
      'secrets',
      'versions',
      'access',
      'latest',
      '--secret',
      secretId,
      '--project',
      GCP_PROJECT,
    ],
    { env: process.env, encoding: 'utf8' },
  ).trim()

/**
 * Publish one signed record, completing NIP-42 AUTH first.
 *
 * The relay enforces NIP-29 permissions itself and answers
 * `auth-required: NIP-29 group write` until the connection is authenticated,
 * so a tool that only sends EVENT sees a refusal that reads like a missing
 * permission. Authenticate, then publish, then report the relay's own verdict
 * rather than assuming success.
 */
export const publishSignedRecord = async (
  relayUrl: string,
  secretKey: Uint8Array,
  record: Readonly<{
    kind: number
    tags: ReadonlyArray<ReadonlyArray<string>>
  }>,
): Promise<Readonly<{ eventId: string; accepted: boolean; message: string }>> => {
  const event = finalizeEvent(
    {
      kind: record.kind,
      created_at: Math.floor(Date.now() / 1_000),
      tags: record.tags.map(tag => [...tag]),
      content: '',
    },
    secretKey,
  )
  return await new Promise((resolve, reject) => {
    const socket = new WebSocket(relayUrl)
    let settled = false
    let published = false
    const timer = setTimeout(() => finish(new Error('the community relay timed out')), RELAY_TIMEOUT_MS)
    const finish = (error?: Error, value?: Readonly<{ eventId: string; accepted: boolean; message: string }>): void => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      try {
        socket.close(1_000, 'community-admission-complete')
      } catch {
        // A close failure cannot invalidate a verdict the relay already gave.
      }
      if (error !== undefined) reject(error)
      else if (value !== undefined) resolve(value)
    }
    const publish = (): void => {
      if (published) return
      published = true
      socket.send(JSON.stringify(['EVENT', event]))
    }
    socket.addEventListener('open', () => {
      // Give the relay a moment to send its AUTH challenge; publish anyway if
      // it does not, because not every relay demands one.
      setTimeout(publish, 1_500)
    })
    socket.addEventListener('message', messageEvent => {
      let frame: unknown
      try {
        frame = JSON.parse(String((messageEvent as MessageEvent).data))
      } catch {
        return
      }
      if (!Array.isArray(frame) || typeof frame[0] !== 'string') return
      if (frame[0] === 'AUTH' && typeof frame[1] === 'string') {
        const auth = finalizeEvent(
          {
            kind: NIP_42_AUTH_KIND,
            created_at: Math.floor(Date.now() / 1_000),
            tags: [
              ['relay', relayUrl],
              ['challenge', frame[1]],
            ],
            content: '',
          },
          secretKey,
        )
        socket.send(JSON.stringify(['AUTH', auth]))
        return
      }
      if (frame[0] === 'OK' && frame[1] === event.id) {
        finish(undefined, {
          eventId: event.id,
          accepted: frame[2] === true,
          message: typeof frame[3] === 'string' ? frame[3] : '',
        })
        return
      }
      if (frame[0] === 'OK') {
        // The AUTH event was accepted; the group write can proceed.
        publish()
      }
    })
    socket.addEventListener('error', () => finish(new Error('the community relay is unavailable')))
    socket.addEventListener('close', () => finish(new Error('the community relay disconnected before answering')))
  })
}

const main = async (): Promise<void> => {
  const input = parseArguments(process.argv.slice(2))
  const pubkey = canonicalPubkey(input.npub)

  const authorityJson = process.env['SARAH_LIVEKIT_COMMUNITY_AUTHORITY_JSON']?.trim()
  if (authorityJson === undefined || authorityJson === '') {
    throw new Error(
      'set SARAH_LIVEKIT_COMMUNITY_AUTHORITY_JSON to the deployed community authority config',
    )
  }
  const authority = resolveCommunityAuthority(authorityJson, input.communityRef)

  const secretHex = readSecret(input.adminSecretId)
  if (!/^[0-9a-f]{64}$/u.test(secretHex)) {
    throw new Error(`${input.adminSecretId} is not a 32-byte hex Nostr secret`)
  }
  const secretKey = Uint8Array.from(Buffer.from(secretHex, 'hex'))
  const signerPubkey = finalizeEvent(
    { kind: 1, created_at: 0, tags: [], content: '' },
    secretKey,
  ).pubkey

  if (!authority.adminPubkeys.includes(signerPubkey)) {
    throw new Error(
      `${input.adminSecretId} derives to ${signerPubkey}, which the deployed authority ` +
        `config does not list as an admin of ${input.communityRef}. The relay might store ` +
        'the record, but the API would ignore it.',
    )
  }

  const kind = input.action === 'admit' ? NIP_29_PUT_USER_KIND : NIP_29_REMOVE_USER_KIND
  const tags =
    input.action === 'admit'
      ? [
          ['h', input.communityRef],
          ['p', pubkey, input.role],
        ]
      : [
          ['h', input.communityRef],
          ['p', pubkey],
        ]

  if (!input.apply) {
    process.stdout.write(
      `${JSON.stringify(
        {
          applied: false,
          gate: OWNER_GATE,
          communityRef: input.communityRef,
          relayUrl: authority.relayUrl,
          signerPubkey,
          kind,
          tags,
        },
        null,
        2,
      )}\n`,
    )
    return
  }

  if (process.env['OA_LIVEKIT_OWNER_GATE']?.trim() !== OWNER_GATE) {
    throw new Error(`set OA_LIVEKIT_OWNER_GATE=${OWNER_GATE} to publish a membership record`)
  }

  const result = await publishSignedRecord(authority.relayUrl, secretKey, { kind, tags })
  process.stdout.write(
    `${JSON.stringify(
      {
        applied: true,
        communityRef: input.communityRef,
        relayUrl: authority.relayUrl,
        signerPubkey,
        subjectPubkey: pubkey,
        kind,
        role: input.action === 'admit' ? input.role : undefined,
        ...result,
      },
      null,
      2,
    )}\n`,
  )
  if (!result.accepted) {
    process.exitCode = 1
  }
}

if (process.argv[1]?.endsWith('admit-sarah-livekit-community-npub.ts') === true) {
  await main()
}

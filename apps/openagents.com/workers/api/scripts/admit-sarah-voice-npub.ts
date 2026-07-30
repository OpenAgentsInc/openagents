#!/usr/bin/env -S pnpm exec tsx

import * as nip19 from 'nostr-effect/nip19'

import { defaultMakeKhalaSyncSqlClient } from '../src/khala-sync-push-routes'

const ADMISSION_GATE = 'I_APPROVE_BOUNDED_SARAH_VOICE_CREDIT'
const MAX_INITIAL_BALANCE_MSAT = 1_000_000_000
const ALPHA_COHORT_REF = 'sarah_voice_cohort:alpha_v1'

const requiredEnv = (name: string): string => {
  const value = process.env[name]?.trim()
  if (value === undefined || value === '') {
    throw new Error(`missing required environment variable ${name}`)
  }
  return value
}

const canonicalPubkey = (npub: string): string => {
  const value = npub.trim()
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
    throw new Error('the Nostr identity must be one canonical npub')
  }
}

const positiveBalance = (value: string): number => {
  const balance = Number(value)
  if (
    !Number.isSafeInteger(balance) ||
    balance <= 0 ||
    balance > MAX_INITIAL_BALANCE_MSAT
  ) {
    throw new Error(
      `balance must be a whole number from 1 through ${MAX_INITIAL_BALANCE_MSAT} msat`,
    )
  }
  return balance
}

const parseArgs = (): Readonly<{ npub: string; balanceMsat: number }> => {
  const args = process.argv.slice(2)
  let apply = false
  let npub: string | undefined
  let balance: string | undefined
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index]
    if (argument === '--apply') {
      apply = true
    } else if (argument === '--npub' && args[index + 1] !== undefined) {
      npub = args[index + 1]
      index += 1
    } else if (argument === '--balance-msat' && args[index + 1] !== undefined) {
      balance = args[index + 1]
      index += 1
    } else {
      throw new Error(
        'usage: admit-sarah-voice-npub.ts --apply --npub NPUB --balance-msat INTEGER',
      )
    }
  }
  if (!apply || npub === undefined || balance === undefined) {
    throw new Error(
      'usage: admit-sarah-voice-npub.ts --apply --npub NPUB --balance-msat INTEGER',
    )
  }
  return { npub, balanceMsat: positiveBalance(balance) }
}

const input = parseArgs()
if (requiredEnv('SARAH_VOICE_NOSTR_ADMISSION_GATE') !== ADMISSION_GATE) {
  throw new Error(
    `set SARAH_VOICE_NOSTR_ADMISSION_GATE=${ADMISSION_GATE} to admit the bounded balance`,
  )
}

const pubkey = canonicalPubkey(input.npub)
const canonicalUserId = `nostr:${pubkey}`
const nowIso = new Date().toISOString()
const databaseUrl = requiredEnv('KHALA_SYNC_DATABASE_URL')
const client = await defaultMakeKhalaSyncSqlClient(databaseUrl)

try {
  const result = await client.sql.begin(async transaction => {
    let identities = await transaction<ReadonlyArray<{ user_id: string }>>`
      SELECT user_id
      FROM auth_identities
      WHERE provider = 'nostr'
        AND provider_subject = ${pubkey}
        AND deleted_at IS NULL
      FOR UPDATE
    `
    let admittedUserId = identities[0]?.user_id
    if (admittedUserId === undefined) {
      const insertedUsers = await transaction<ReadonlyArray<{ id: string }>>`
        INSERT INTO users (
          id, kind, display_name, primary_email, status, created_at, updated_at
        ) VALUES (
          ${canonicalUserId}, 'human', ${`nostr-${pubkey.slice(0, 12)}`},
          ${`${pubkey}@nostr.invalid`}, 'active', ${nowIso}, ${nowIso}
        )
        ON CONFLICT (id) DO NOTHING
        RETURNING id
      `
      await transaction`
        INSERT INTO auth_identities (
          id, user_id, provider, provider_subject, provider_username, email,
          created_at, updated_at
        ) VALUES (
          ${`auth_identity_nostr_${pubkey}`}, ${canonicalUserId}, 'nostr', ${pubkey},
          ${`nostr-${pubkey.slice(0, 12)}`}, ${`${pubkey}@nostr.invalid`},
          ${nowIso}, ${nowIso}
        )
        ON CONFLICT (provider, provider_subject) DO NOTHING
      `
      identities = await transaction<ReadonlyArray<{ user_id: string }>>`
        SELECT user_id
        FROM auth_identities
        WHERE provider = 'nostr'
          AND provider_subject = ${pubkey}
          AND deleted_at IS NULL
        FOR UPDATE
      `
      admittedUserId = identities[0]?.user_id
      if (admittedUserId === undefined) {
        throw new Error('the npub has a deleted or invalid identity binding')
      }
      if (
        admittedUserId !== canonicalUserId &&
        insertedUsers[0]?.id === canonicalUserId
      ) {
        await transaction`
          DELETE FROM users
          WHERE id = ${canonicalUserId}
        `
      }
    }

    const users = await transaction<
      ReadonlyArray<{ kind: string; status: string; deleted_at: string | null }>
    >`
      SELECT kind, status, deleted_at
      FROM users
      WHERE id = ${admittedUserId}
      FOR UPDATE
    `
    const user = users[0]
    if (
      user === undefined ||
      user.kind !== 'human' ||
      user.status !== 'active' ||
      user.deleted_at !== null
    ) {
      throw new Error(
        'the Nostr identity is not linked to an active human user',
      )
    }

    const actorRef = `agent:${admittedUserId}`
    const existingBalance = await transaction<
      ReadonlyArray<{ actor_ref: string; balance_msat: number | string }>
    >`
      SELECT actor_ref, balance_msat
      FROM agent_balances
      WHERE actor_ref = ${actorRef}
      FOR UPDATE
    `
    if (existingBalance[0] === undefined) {
      await transaction`
        INSERT INTO agent_balances (
          actor_ref, balance_msat, held_msat, usd_credit_msat, created_at,
          updated_at
        ) VALUES (
          ${actorRef}, ${input.balanceMsat}, 0, ${input.balanceMsat},
          ${nowIso}, ${nowIso}
        )
      `
    }
    const balanceMsat =
      existingBalance[0] === undefined
        ? input.balanceMsat
        : Number(existingBalance[0].balance_msat)
    if (!Number.isSafeInteger(balanceMsat) || balanceMsat < 0) {
      throw new Error('the existing balance is outside the safe integer range')
    }

    const membershipRef = `sarah_voice_alpha:${admittedUserId}`
    const admitted = await transaction<
      ReadonlyArray<{ membership_ref: string }>
    >`
      INSERT INTO sarah_voice_alpha_memberships (
        membership_ref, cohort_ref, owner_user_id, state, admitted_at,
        admission_actor_ref, admission_reason, updated_at
      ) VALUES (
        ${membershipRef}, ${ALPHA_COHORT_REF}, ${admittedUserId}, 'active',
        ${nowIso}, 'operator:admit-sarah-voice-npub',
        'Approved finite-credit Sarah voice alpha admission', ${nowIso}
      )
      ON CONFLICT (owner_user_id) DO NOTHING
      RETURNING membership_ref
    `
    if (admitted[0] !== undefined) {
      await transaction`
        INSERT INTO sarah_voice_alpha_membership_audit (
          event_ref, membership_ref, cohort_ref, action, actor_ref, reason,
          source, occurred_at
        ) VALUES (
          ${`${membershipRef}:admitted`}, ${membershipRef}, ${ALPHA_COHORT_REF},
          'admitted', 'operator:admit-sarah-voice-npub',
          'Approved finite-credit Sarah voice alpha admission',
          'operator:script', ${nowIso}
        )
        ON CONFLICT (event_ref) DO NOTHING
      `
    }
    const memberships = await transaction<
      ReadonlyArray<{ cohort_ref: string; state: string }>
    >`
      SELECT cohort_ref, state
      FROM sarah_voice_alpha_memberships
      WHERE owner_user_id = ${admittedUserId}
      FOR UPDATE
    `
    if (
      memberships[0]?.cohort_ref !== ALPHA_COHORT_REF ||
      memberships[0]?.state !== 'active'
    ) {
      throw new Error('the npub has a revoked or conflicting alpha membership')
    }
    return {
      userId: admittedUserId,
      balanceMsat,
      balanceCreated: existingBalance[0] === undefined,
    }
  })
  process.stdout.write(`${JSON.stringify({ npub: input.npub, ...result })}\n`)
} finally {
  await client.end()
}

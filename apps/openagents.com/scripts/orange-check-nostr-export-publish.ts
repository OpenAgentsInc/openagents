#!/usr/bin/env bun

/**
 * Publish the orange-check forum identity signal to the OWNED Nostr relay.
 *
 * Promise: `identity.orange_check_forum_signal.v1`.
 * Blocker cleared: `orange_check_nostr_export_missing` — the orange-check signal
 * had a build-only NIP-58 export (`buildOrangeCheckNostrExport`) but was never
 * published anywhere dereferenceable. The owned relay (#5537,
 * `wss://relay.openagents.com`) now accepts the general coordination kinds
 * (NIP-01/02/17/28/38/65) gated by a provisioned-pubkey allowlist or NIP-42
 * AUTH. The NIP-58 badge kinds (8 award, 30009 definition) are NOT in that
 * allowlist, so the dereferenceable on-relay form of the signal is a NIP-01
 * kind-1 attestation note that references the badge definition address, claim,
 * public receipt ref, recipient pubkey, and paid amount as tags.
 *
 * Flow (mirrors scripts/nostr-fallback-drill.ts):
 *   1. Build the public-safe kind-1 attestation via buildOrangeCheckNostrAttestation.
 *   2. Sign it with the issuer key.
 *   3. Open the relay WS; complete NIP-42 AUTH for the issuer key (or rely on the
 *      provisioned allowlist) so the write is authorized.
 *   4. Publish the EVENT; require an OK true.
 *   5. Read the event back by id (REQ) -> that id is the dereferenceable receipt.
 *
 * Keys: by default an EPHEMERAL issuer key is generated per run. To publish under
 * a stable OpenAgents issuer key, set NOSTR_SECRET_KEY (64-hex) — ONLY via env,
 * never on argv. The secret is used to sign locally and is never sent or logged.
 *
 * Usage:
 *   bun apps/openagents.com/scripts/orange-check-nostr-export-publish.ts publish \
 *     [--relay wss://relay.openagents.com] [--recipient <64-hex>]
 *   bun apps/openagents.com/scripts/orange-check-nostr-export-publish.ts plan
 *     (offline: builds + prints the attestation, no relay)
 */

import {
  finalizeEvent,
  generateSecretKey,
  getPublicKey,
  type EventTemplate,
  type VerifiedEvent,
} from '../../../../nostr-effect/src/wrappers/pure.ts'
import { makeAuthEvent } from '../../../../nostr-effect/src/wrappers/nip42.ts'
import {
  buildOrangeCheckNostrAttestation,
  type OrangeCheckNostrAttestation,
} from '../workers/api/src/orange-check-nostr-export.ts'
import type { OrangeCheckEntitlement } from '../workers/api/src/orange-check-entitlements.ts'

type Flags = Record<string, string | true>
type RelayMessage = ReadonlyArray<unknown>

const defaultRelay = 'wss://relay.openagents.com'

const usage = () => `Usage:
  bun apps/openagents.com/scripts/orange-check-nostr-export-publish.ts publish [--relay URL] [--recipient 64-hex]
  bun apps/openagents.com/scripts/orange-check-nostr-export-publish.ts plan

Options:
  --relay <url>        Relay to publish/read against. Defaults to ${defaultRelay}.
  --recipient <hex>    64-hex recipient (orange-checked agent) pubkey. Defaults to the issuer pubkey.
Env:
  NOSTR_SECRET_KEY     Optional 64-hex issuer secret key. If unset an ephemeral key is generated.
`

const parseFlags = (argv: ReadonlyArray<string>): { command: string; flags: Flags } => {
  const [command = 'help', ...rest] = argv
  const flags: Flags = {}
  for (let index = 0; index < rest.length; index++) {
    const arg = rest[index]
    if (!arg.startsWith('--')) continue
    const name = arg.slice(2)
    const next = rest[index + 1]
    if (next === undefined || next.startsWith('--')) {
      flags[name] = true
      continue
    }
    flags[name] = next
    index++
  }
  return { command, flags }
}

const optionalString = (flags: Flags, name: string, fallback: string): string => {
  const value = flags[name]
  return typeof value === 'string' && value.length > 0 ? value : fallback
}

const relayWebSocketUrl = (input: string): string => {
  const url = new URL(/^[a-z]+:\/\//i.test(input) ? input : `wss://${input}`)
  if (url.protocol === 'http:') url.protocol = 'ws:'
  if (url.protocol === 'https:') url.protocol = 'wss:'
  if (url.protocol !== 'ws:' && url.protocol !== 'wss:') {
    throw new Error(`Expected ws/wss/http/https relay URL, got ${input}`)
  }
  return url.toString()
}

const hexToBytes = (hex: string): Uint8Array => {
  if (!/^[0-9a-f]{64}$/i.test(hex)) {
    throw new Error('NOSTR_SECRET_KEY must be 64 hex chars')
  }
  const out = new Uint8Array(32)
  for (let i = 0; i < 32; i++) {
    out[i] = Number.parseInt(hex.slice(i * 2, i * 2 + 2), 16)
  }
  return out
}

const issuerSecretKey = (): Uint8Array => {
  const raw = process.env.NOSTR_SECRET_KEY
  return typeof raw === 'string' && raw.length > 0
    ? hexToBytes(raw.trim())
    : generateSecretKey()
}

const waitForOpen = (socket: WebSocket): Promise<void> =>
  new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error('WebSocket open timed out')), 10_000)
    socket.addEventListener('open', () => {
      clearTimeout(timeout)
      resolve()
    })
    socket.addEventListener('error', () => {
      clearTimeout(timeout)
      reject(new Error('WebSocket open failed'))
    })
  })

const waitForMessage = (
  socket: WebSocket,
  label: string,
  predicate: (message: RelayMessage) => boolean,
): Promise<RelayMessage> =>
  new Promise((resolve, reject) => {
    const seen: Array<RelayMessage> = []
    const timeout = setTimeout(
      () => reject(new Error(`Timed out waiting for ${label}; saw ${JSON.stringify(seen)}`)),
      15_000,
    )
    socket.addEventListener('message', event => {
      const decoded: unknown = JSON.parse(String(event.data))
      const parsed: RelayMessage = Array.isArray(decoded) ? decoded : []
      seen.push(parsed)
      if (predicate(parsed)) {
        clearTimeout(timeout)
        resolve(parsed)
      }
    })
    socket.addEventListener('error', () => {
      clearTimeout(timeout)
      reject(new Error(`WebSocket error while waiting for ${label}`))
    })
  })

const waitForAuthChallenge = (socket: WebSocket, timeoutMs: number): Promise<string | null> =>
  new Promise(resolve => {
    const timeout = setTimeout(() => resolve(null), timeoutMs)
    const onMessage = (event: MessageEvent) => {
      const decoded: unknown = JSON.parse(String(event.data))
      if (Array.isArray(decoded) && decoded[0] === 'AUTH' && typeof decoded[1] === 'string') {
        clearTimeout(timeout)
        socket.removeEventListener('message', onMessage)
        resolve(decoded[1])
      }
    }
    socket.addEventListener('message', onMessage)
  })

const authenticate = async (
  socket: WebSocket,
  relayUrl: string,
  challenge: string,
  authSecretKey: Uint8Array,
): Promise<void> => {
  const template = makeAuthEvent(relayUrl, challenge)
  const authEvent = finalizeEvent(template, authSecretKey)
  socket.send(JSON.stringify(['AUTH', authEvent]))
  const ok = await waitForMessage(
    socket,
    `AUTH OK for ${authEvent.id}`,
    message => message[0] === 'OK' && message[1] === authEvent.id,
  )
  if (ok[2] !== true) {
    throw new Error(`Relay rejected NIP-42 AUTH: ${JSON.stringify(ok)}`)
  }
}

const eventIdOf = (value: unknown): string | undefined => {
  if (typeof value !== 'object' || value === null) return undefined
  const id = Reflect.get(value, 'id')
  return typeof id === 'string' ? id : undefined
}

const publishAndReadBack = async (
  relayUrl: string,
  event: VerifiedEvent,
  authSecretKey: Uint8Array,
): Promise<string> => {
  const socket = new WebSocket(relayUrl)
  await waitForOpen(socket)
  const challenge = await waitForAuthChallenge(socket, 2_000)
  if (challenge !== null) {
    await authenticate(socket, relayUrl, challenge, authSecretKey)
  }
  socket.send(JSON.stringify(['EVENT', event]))
  const ok = await waitForMessage(
    socket,
    `OK for ${event.id}`,
    message => message[0] === 'OK' && message[1] === event.id,
  )
  if (ok[2] !== true) {
    socket.close(1000, 'publish failed')
    throw new Error(`Relay rejected ${event.kind}/${event.id}: ${JSON.stringify(ok)}`)
  }

  const subscriptionId = `orange-check-${event.kind}-${Date.now()}`
  socket.send(JSON.stringify(['REQ', subscriptionId, { ids: [event.id], limit: 1 }]))
  const message = await waitForMessage(
    socket,
    `EVENT ${event.id}`,
    candidate =>
      candidate[0] === 'EVENT' &&
      candidate[1] === subscriptionId &&
      eventIdOf(candidate[2]) === event.id,
  )
  socket.send(JSON.stringify(['CLOSE', subscriptionId]))
  socket.close(1000, 'read complete')
  const id = eventIdOf(message[2])
  if (id === undefined) throw new Error(`Relay returned a malformed event for ${event.id}`)
  return id
}

const sign = (template: EventTemplate, secretKey: Uint8Array): VerifiedEvent =>
  finalizeEvent(template, secretKey)

// The orange-check entitlement projected for the attestation. This mirrors the
// production entitlement shape; refs are public-safe (no payment material).
const attestationEntitlement = (nowIso: string): OrangeCheckEntitlement => ({
  actionRef: 'forum_paid_action.orange_check.challenge_one',
  actorRef: 'agent:openagents-orange-check',
  agentUserId: 'openagents-orange-check',
  createdAt: nowIso,
  id: 'orange_check_openagents-orange-check',
  paidAmountCents: 500,
  receiptRef: 'orange_check_receipt.challenge_one',
  state: 'active',
  updatedAt: nowIso,
})

const buildAttestation = async (
  issuerPubkey: string,
  recipientPubkey: string,
  relayUrl: string,
): Promise<OrangeCheckNostrAttestation> => {
  const nowIso = new Date().toISOString()
  return buildOrangeCheckNostrAttestation({
    entitlement: attestationEntitlement(nowIso),
    issuerPubkey,
    nowIso,
    recipientPubkey,
    relayUrls: [relayUrl],
  })
}

const summarise = (
  relayUrl: string,
  issuerPubkey: string,
  recipientPubkey: string,
  attestation: OrangeCheckNostrAttestation,
  eventId: string | null,
) =>
  JSON.stringify(
    {
      ok: true,
      promise: 'identity.orange_check_forum_signal.v1',
      blocker: 'orange_check_nostr_export_missing',
      relay: relayUrl,
      issuerPubkey,
      recipientPubkey,
      exportKind: attestation.exportKind,
      badgeDefinitionAddress: attestation.badgeDefinitionAddress,
      receiptRef: attestation.receiptRef,
      noteKind: attestation.note.kind,
      eventId,
      dereference: eventId
        ? `wss://relay.openagents.com REQ ["REQ","oc",{"ids":["${eventId}"]}]`
        : '(plan mode: not published)',
    },
    null,
    2,
  )

const runPlan = async () => {
  const sk = issuerSecretKey()
  const issuerPubkey = getPublicKey(sk)
  const relayUrl = defaultRelay
  const attestation = await buildAttestation(issuerPubkey, issuerPubkey, relayUrl)
  // Sign so the event id (the would-be receipt) is shown even offline.
  const signed = sign(
    {
      kind: attestation.note.kind,
      created_at: attestation.note.created_at,
      tags: attestation.note.tags.map(t => [...t]),
      content: attestation.note.content,
    },
    sk,
  )
  console.log(summarise(relayUrl, issuerPubkey, issuerPubkey, attestation, null))
  console.error(`plan: would publish event id ${signed.id}`)
}

const runPublish = async (flags: Flags) => {
  const sk = issuerSecretKey()
  const issuerPubkey = getPublicKey(sk)
  const recipientPubkey = optionalString(flags, 'recipient', issuerPubkey).toLowerCase()
  const relayUrl = relayWebSocketUrl(optionalString(flags, 'relay', defaultRelay))

  const attestation = await buildAttestation(issuerPubkey, recipientPubkey, relayUrl)
  const signed = sign(
    {
      kind: attestation.note.kind,
      created_at: attestation.note.created_at,
      tags: attestation.note.tags.map(t => [...t]),
      content: attestation.note.content,
    },
    sk,
  )

  const gotId = await publishAndReadBack(relayUrl, signed, sk)
  if (gotId !== signed.id) {
    throw new Error(`Read-back id mismatch: ${gotId} != ${signed.id}`)
  }

  console.log(summarise(relayUrl, issuerPubkey, recipientPubkey, attestation, gotId))
  console.error(`read-back verified orange-check attestation ${gotId} on ${relayUrl}`)
}

const { command, flags } = parseFlags(process.argv.slice(2))

if (command === 'publish') {
  await runPublish(flags)
} else if (command === 'plan') {
  await runPlan()
} else {
  console.log(usage())
  process.exit(command === 'help' || command === '--help' ? 0 : 1)
}

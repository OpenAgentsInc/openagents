#!/usr/bin/env bun

/**
 * Nostr fallback-coordination outage drill (smoke).
 *
 * Promise: `agents.nostr_fallback_coordination.v1` (DE-8, EPIC #5531).
 * Receipt: an outage-coordination drill whose every coordination step is a real,
 * fetchable event on a public relay, with zero secrets in any event/log line.
 *
 * Scenario, end to end:
 *   1. OpenAgents HTTP is unreachable -> agents fall back to Nostr.
 *   2. Each agent publishes a NIP-38 user-status (kind 30315) so peers see liveness.
 *   3. Peers are discovered via NIP-02 contacts (kind 3) + NIP-65 relay lists
 *      (kind 10002) -> the fallback relay set is itself advertised over Nostr.
 *   4. The two agents exchange a NIP-17 gift-wrapped DM (kind 1059) to agree to
 *      keep working while HTTP is down.
 *   5. A NIP-90 labor job keeps moving over Nostr: LBR request -> quote ->
 *      acceptance -> result (kinds 5934 / 7000 / 6934), all ref-only and
 *      public-safe (the LBR codec rejects secrets/preimages/invoices/paths).
 *   6. On recovery each agent publishes a NIP-38 "online" status to reconcile.
 *
 * It models on apps/openagents.com/scripts/nip-ds.ts: build protocol objects
 * through @openagentsinc/nip90 (which reuses the sibling nostr-effect impl),
 * publish each signed event to a relay, and read it back by id as the receipt.
 * It does NOT move sats, grant settlement authority, or publish anything private.
 *
 * Keys: this drill uses EPHEMERAL demo keys generated per run (generateSecretKey),
 * never a real agent key. If you ever wire a real key in, pass it ONLY through the
 * NOSTR_SECRET_KEY env var -- never on argv. No secret is ever emitted or logged.
 *
 * Usage:
 *   bun apps/openagents.com/scripts/nostr-fallback-drill.ts smoke [--relay URL]
 *   bun apps/openagents.com/scripts/nostr-fallback-drill.ts plan   (offline; builds + prints events, no relay)
 */

import {
  LBR_AGENTIC_CODING_REQUEST_KIND,
  LBR_AGENTIC_CODING_RESULT_KIND,
  LBR_FEEDBACK_KIND,
  lbrAcceptanceToDraft,
  lbrAgenticCodingRequestToDraft,
  lbrQuoteToDraft,
  lbrResultToDraft,
  makeLbrAcceptance,
  makeLbrAgenticCodingRequest,
  makeLbrQuote,
  makeLbrResult,
} from "@openagentsinc/nip90"
import {
  finalizeEvent,
  generateSecretKey,
  getPublicKey,
  type EventTemplate,
  type VerifiedEvent,
} from "../../../../nostr-effect/src/wrappers/pure.ts"
import { wrapEvent } from "../../../../nostr-effect/src/wrappers/nip17.ts"
import { makeAuthEvent } from "../../../../nostr-effect/src/wrappers/nip42.ts"

type Flags = Record<string, string | true>
type RelayMessage = ReadonlyArray<unknown>

// The publishable shape shared by a finalized event and a NIP-17 gift wrap:
// both carry a signed id and are valid relay EVENT payloads.
type PublishableEvent = {
  readonly id: string
  readonly kind: number
  readonly content: string
  readonly tags: ReadonlyArray<ReadonlyArray<string>>
}

const defaultRelay = "wss://relay.damus.io"
const fallbackRelays = ["wss://relay.damus.io", "wss://nos.lol"] as const

// NIP-38 user statuses / NIP-02 contacts / NIP-65 relay list.
const KIND_USER_STATUS = 30315
const KIND_CONTACTS = 3
const KIND_RELAY_LIST = 10002

const usage = () => `Usage:
  bun apps/openagents.com/scripts/nostr-fallback-drill.ts smoke [--relay URL]
  bun apps/openagents.com/scripts/nostr-fallback-drill.ts plan

Options:
  --relay <url>   Public relay to publish/read against. Defaults to ${defaultRelay}.
`

const parseFlags = (argv: ReadonlyArray<string>): { command: string; flags: Flags } => {
  const [command = "help", ...rest] = argv
  const flags: Flags = {}
  for (let index = 0; index < rest.length; index++) {
    const arg = rest[index]
    if (!arg.startsWith("--")) continue
    const name = arg.slice(2)
    const next = rest[index + 1]
    if (next === undefined || next.startsWith("--")) {
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
  return typeof value === "string" && value.length > 0 ? value : fallback
}

const relayWebSocketUrl = (input: string): string => {
  const url = new URL(/^[a-z]+:\/\//i.test(input) ? input : `wss://${input}`)
  if (url.protocol === "http:") url.protocol = "ws:"
  if (url.protocol === "https:") url.protocol = "wss:"
  if (url.protocol !== "ws:" && url.protocol !== "wss:") {
    throw new Error(`Expected ws/wss/http/https relay URL, got ${input}`)
  }
  return url.toString()
}

const waitForOpen = (socket: WebSocket): Promise<void> =>
  new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("WebSocket open timed out")), 10_000)
    socket.addEventListener("open", () => {
      clearTimeout(timeout)
      resolve()
    })
    socket.addEventListener("error", () => {
      clearTimeout(timeout)
      reject(new Error("WebSocket open failed"))
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
    socket.addEventListener("message", event => {
      const decoded: unknown = JSON.parse(String(event.data))
      const parsed: RelayMessage = Array.isArray(decoded) ? decoded : []
      seen.push(parsed)
      if (predicate(parsed)) {
        clearTimeout(timeout)
        resolve(parsed)
      }
    })
    socket.addEventListener("error", () => {
      clearTimeout(timeout)
      reject(new Error(`WebSocket error while waiting for ${label}`))
    })
  })

// Wait briefly for an optional NIP-42 ["AUTH", <challenge>] frame the relay may
// send on open. Public relays do not send one; the owned relay (#5537) does, to
// gate the general coordination kinds. Returns the challenge string or null.
const waitForAuthChallenge = (socket: WebSocket, timeoutMs: number): Promise<string | null> =>
  new Promise(resolve => {
    const timeout = setTimeout(() => resolve(null), timeoutMs)
    const onMessage = (event: MessageEvent) => {
      const decoded: unknown = JSON.parse(String(event.data))
      if (Array.isArray(decoded) && decoded[0] === "AUTH" && typeof decoded[1] === "string") {
        clearTimeout(timeout)
        socket.removeEventListener("message", onMessage)
        resolve(decoded[1])
      }
    }
    socket.addEventListener("message", onMessage)
  })

// Complete the relay's NIP-42 AUTH handshake for `authSecretKey`. The auth event
// (kind 22242) references the relay URL and the relay-issued challenge. The
// secret key is used only to sign locally; it is never sent or logged.
const authenticate = async (
  socket: WebSocket,
  relayUrl: string,
  challenge: string,
  authSecretKey: Uint8Array,
): Promise<void> => {
  const template = makeAuthEvent(relayUrl, challenge)
  const authEvent = finalizeEvent(template, authSecretKey)
  socket.send(JSON.stringify(["AUTH", authEvent]))
  const ok = await waitForMessage(
    socket,
    `AUTH OK for ${authEvent.id}`,
    message => message[0] === "OK" && message[1] === authEvent.id,
  )
  if (ok[2] !== true) {
    throw new Error(`Relay rejected NIP-42 AUTH: ${JSON.stringify(ok)}`)
  }
}

const publishEvent = async (
  relayUrl: string,
  event: PublishableEvent,
  authSecretKey: Uint8Array,
): Promise<RelayMessage> => {
  const socket = new WebSocket(relayUrl)
  await waitForOpen(socket)
  // If the relay issues a NIP-42 challenge, authenticate this connection before
  // publishing so the owned relay accepts general coordination kinds.
  const challenge = await waitForAuthChallenge(socket, 1_000)
  if (challenge !== null) {
    await authenticate(socket, relayUrl, challenge, authSecretKey)
  }
  socket.send(JSON.stringify(["EVENT", event]))
  const ok = await waitForMessage(
    socket,
    `OK for ${event.id}`,
    message => message[0] === "OK" && message[1] === event.id,
  )
  socket.close(1000, "publish complete")
  if (ok[2] !== true) {
    throw new Error(`Relay rejected ${event.kind}/${event.id}: ${JSON.stringify(ok)}`)
  }
  return ok
}

// Pull the `id` field out of an unknown relay payload without a type assertion.
const eventIdOf = (value: unknown): string | undefined => {
  if (typeof value !== "object" || value === null) return undefined
  const id = Reflect.get(value, "id")
  return typeof id === "string" ? id : undefined
}

// Read an event back by id and return its relay-confirmed id (the receipt).
const readEventId = async (relayUrl: string, event: PublishableEvent): Promise<string> => {
  const socket = new WebSocket(relayUrl)
  await waitForOpen(socket)
  const subscriptionId = `nostr-fallback-${event.kind}-${Date.now()}`
  socket.send(JSON.stringify(["REQ", subscriptionId, { ids: [event.id], limit: 1 }]))
  const message = await waitForMessage(
    socket,
    `EVENT ${event.id}`,
    candidate =>
      candidate[0] === "EVENT" &&
      candidate[1] === subscriptionId &&
      eventIdOf(candidate[2]) === event.id,
  )
  socket.send(JSON.stringify(["CLOSE", subscriptionId]))
  socket.close(1000, "read complete")
  const id = eventIdOf(message[2])
  if (id === undefined) throw new Error(`Relay returned a malformed event for ${event.id}`)
  return id
}

const sign = (template: EventTemplate, secretKey: Uint8Array): VerifiedEvent =>
  finalizeEvent(template, secretKey)

const sleep = (ms: number): Promise<void> => new Promise(resolve => setTimeout(resolve, ms))

type Phase = {
  readonly phase: string
  readonly nip: string
  readonly note: string
  readonly event: PublishableEvent
  /**
   * Secret key used to satisfy a relay's NIP-42 AUTH challenge when publishing
   * this event (#5537). The OWNED relay write-gates general coordination kinds
   * behind AUTH; public relays ignore it. For gift wraps (ephemeral wire key)
   * any held key authenticates the connection; the relay allows kind 1059 on any
   * authenticated connection. Never logged.
   */
  readonly authSecretKey: Uint8Array
}

/**
 * Build the full ordered set of coordination events for the drill.
 * Every event is signed and publishable; nothing here touches a relay.
 */
const buildDrill = () => {
  const now = Math.floor(Date.now() / 1000)

  // Two ephemeral demo agents (never real keys).
  const requesterSk = generateSecretKey()
  const providerSk = generateSecretKey()
  const requesterPk = getPublicKey(requesterSk)
  const providerPk = getPublicKey(providerSk)

  const phases: Array<Phase> = []

  // Phase 1+2: HTTP unreachable -> publish liveness over Nostr (NIP-38 kind 30315).
  // The status content is the public, non-secret signal "openagents-http-unreachable".
  // NIP-38 statuses are addressable (kind 30315): a later event with the same
  // `d` tag REPLACES an earlier one, which would make the earlier id un-fetchable.
  // Give every status in the drill a distinct `d` so each stays independently
  // addressable and id-readable as its own receipt.
  const requesterStatus = sign(
    {
      kind: KIND_USER_STATUS,
      created_at: now,
      tags: [
        ["d", "fallback-drill-outage"],
        ["t", "openagents-fallback-drill"],
        ["expiration", String(now + 3600)],
      ],
      content: "openagents-http-unreachable: coordinating over Nostr",
    },
    requesterSk,
  )
  phases.push({
    phase: "1-2 fallback-liveness",
    nip: "NIP-38 (kind 30315)",
    note: "HTTP unreachable; requester advertises liveness over Nostr",
    event: requesterStatus,
    authSecretKey: requesterSk,
  })
  const providerStatus = sign(
    {
      kind: KIND_USER_STATUS,
      created_at: now,
      tags: [
        ["d", "fallback-drill-outage"],
        ["t", "openagents-fallback-drill"],
        ["expiration", String(now + 3600)],
      ],
      content: "openagents-http-unreachable: provider available for labor",
    },
    providerSk,
  )
  phases.push({
    phase: "1-2 fallback-liveness",
    nip: "NIP-38 (kind 30315)",
    note: "provider advertises liveness over Nostr",
    event: providerStatus,
    authSecretKey: providerSk,
  })

  // Phase 3: discovery. Requester advertises its fallback relay set (NIP-65 kind
  // 10002) and a contact list pointing at the provider (NIP-02 kind 3), so peers
  // can find each other and the agreed relays without any central HTTP service.
  const requesterRelayList = sign(
    {
      kind: KIND_RELAY_LIST,
      created_at: now,
      tags: fallbackRelays.map(r => ["r", r]),
      content: "",
    },
    requesterSk,
  )
  phases.push({
    phase: "3 discovery",
    nip: "NIP-65 (kind 10002)",
    note: "requester advertises its fallback relay set",
    event: requesterRelayList,
    authSecretKey: requesterSk,
  })
  const requesterContacts = sign(
    {
      kind: KIND_CONTACTS,
      created_at: now,
      tags: [["p", providerPk, fallbackRelays[0], "provider"]],
      content: "",
    },
    requesterSk,
  )
  phases.push({
    phase: "3 discovery",
    nip: "NIP-02 (kind 3)",
    note: "requester contact list points at the provider",
    event: requesterContacts,
    authSecretKey: requesterSk,
  })

  // Phase 4: NIP-17 gift-wrapped DM (kind 1059). Requester tells the provider:
  // keep working, HTTP is down, settle the receipt over Nostr. The plaintext lives
  // only inside the NIP-44-encrypted seal; the published event content is ciphertext.
  const giftWrap: PublishableEvent = wrapEvent(
    requesterSk,
    { publicKey: providerPk },
    "openagents fallback: HTTP down, keep the labor job moving and we reconcile over Nostr",
  )
  phases.push({
    phase: "4 private-dm",
    nip: "NIP-17 gift wrap (kind 1059)",
    note: "requester -> provider encrypted coordination DM (content is ciphertext)",
    event: giftWrap,
    authSecretKey: requesterSk,
  })

  // Phase 5: NIP-90 labor job keeps moving over Nostr (LBR: request/quote/accept/
  // result). All refs are public-safe; the codec rejects secrets/preimages/invoices.
  const objectiveRef = "github.openagents.issue:5531"
  const repositoryRefs = ["github.openagents.repo:openagents"]
  const verificationCommandRef = "ci.openagents.command:check-deploy"
  const capabilityRefs = ["capability.openagents.skill:typescript"]

  const lbrRequest = makeLbrAgenticCodingRequest({
    objectiveRef,
    repositoryRefs,
    verificationCommandRef,
    requiredCapabilityRefs: capabilityRefs,
    bidMsats: 100_000,
    forumTopicRef: "forum.openagents.topic:product-promises",
    relays: [...fallbackRelays],
  })
  const requestDraft = lbrAgenticCodingRequestToDraft(lbrRequest)
  const requestEvent = sign(
    { kind: requestDraft.kind, created_at: now, tags: requestDraft.tags.map(t => [...t]), content: requestDraft.content },
    requesterSk,
  )
  phases.push({
    phase: "5 labor-job",
    nip: `NIP-90 LBR request (kind ${LBR_AGENTIC_CODING_REQUEST_KIND})`,
    note: "labor request published over Nostr while HTTP is down",
    event: requestEvent,
    authSecretKey: requesterSk,
  })

  const lbrQuote = makeLbrQuote({
    requestId: requestEvent.id,
    requesterPubkey: requesterPk,
    amountMsats: 100_000,
    providerRef: "agent.openagents.provider:lathe",
    capabilityRefs,
    quoteRef: "quote.openagents.lbr:fallback-drill",
    requestRelay: fallbackRelays[0],
  })
  const quoteDraft = lbrQuoteToDraft(lbrQuote)
  const quoteEvent = sign(
    { kind: quoteDraft.kind, created_at: now + 1, tags: quoteDraft.tags.map(t => [...t]), content: quoteDraft.content },
    providerSk,
  )
  phases.push({
    phase: "5 labor-job",
    nip: `NIP-90 LBR quote (kind ${LBR_FEEDBACK_KIND})`,
    note: "provider quotes the job over Nostr",
    event: quoteEvent,
    authSecretKey: providerSk,
  })

  const lbrAcceptance = makeLbrAcceptance({
    requestId: requestEvent.id,
    providerPubkey: providerPk,
    escrowReceiptRef: "escrow.openagents.receipt:fallback-drill",
    acceptanceRef: "acceptance.openagents.lbr:fallback-drill",
    requestRelay: fallbackRelays[0],
  })
  const acceptanceDraft = lbrAcceptanceToDraft(lbrAcceptance)
  const acceptanceEvent = sign(
    {
      kind: acceptanceDraft.kind,
      created_at: now + 2,
      tags: acceptanceDraft.tags.map(t => [...t]),
      content: acceptanceDraft.content,
    },
    requesterSk,
  )
  phases.push({
    phase: "5 labor-job",
    nip: `NIP-90 LBR acceptance (kind ${LBR_FEEDBACK_KIND})`,
    note: "requester accepts the quote over Nostr",
    event: acceptanceEvent,
    authSecretKey: requesterSk,
  })

  const lbrResult = makeLbrResult({
    requestId: requestEvent.id,
    requesterPubkey: requesterPk,
    artifactRefs: ["github.openagents.pr:5531-drill"],
    platformCloseoutRef: "closeout.openagents.lbr:fallback-drill",
    summaryRef: "summary.openagents.lbr:fallback-drill",
    testRef: "ci.openagents.run:fallback-drill",
    requestRelay: fallbackRelays[0],
  })
  const resultDraft = lbrResultToDraft(lbrResult)
  const resultEvent = sign(
    { kind: resultDraft.kind, created_at: now + 3, tags: resultDraft.tags.map(t => [...t]), content: resultDraft.content },
    providerSk,
  )
  phases.push({
    phase: "5 labor-job",
    nip: `NIP-90 LBR result (kind ${LBR_AGENTIC_CODING_RESULT_KIND})`,
    note: "provider delivers the result over Nostr",
    event: resultEvent,
    authSecretKey: providerSk,
  })

  // Phase 6: recovery / reconcile. HTTP is back; each agent publishes an "online"
  // status so the cluster knows the outage is over and state can be reconciled.
  const requesterRecovered = sign(
    {
      kind: KIND_USER_STATUS,
      created_at: now + 4,
      tags: [["d", "fallback-drill-recovery"], ["t", "openagents-fallback-drill"]],
      content: "openagents-http-recovered: reconciled, online",
    },
    requesterSk,
  )
  phases.push({
    phase: "6 recovery",
    nip: "NIP-38 (kind 30315)",
    note: "requester reconciles on recovery",
    event: requesterRecovered,
    authSecretKey: requesterSk,
  })
  const providerRecovered = sign(
    {
      kind: KIND_USER_STATUS,
      created_at: now + 4,
      tags: [["d", "fallback-drill-recovery"], ["t", "openagents-fallback-drill"]],
      content: "openagents-http-recovered: reconciled, online",
    },
    providerSk,
  )
  phases.push({
    phase: "6 recovery",
    nip: "NIP-38 (kind 30315)",
    note: "provider reconciles on recovery",
    event: providerRecovered,
    authSecretKey: providerSk,
  })

  return { requesterPk, providerPk, phases }
}

// Defence in depth: scan every byte we are about to publish for anything that
// looks like a secret, before it leaves the process. The LBR codec already
// rejects unsafe refs; this catches the hand-built NIP-38/02/65 events too.
const SECRET_PATTERN =
  /(nsec1|lnbc|lntb|lno1|-----BEGIN|mnemonic|payment_preimage|preimage|seed phrase|SECRET|TOKEN=|api[_-]?key)/i

const assertNoSecrets = (phases: ReadonlyArray<Phase>) => {
  for (const p of phases) {
    const haystack = `${p.event.content}\n${JSON.stringify(p.event.tags)}`
    if (SECRET_PATTERN.test(haystack)) {
      throw new Error(`Refusing to publish ${p.phase} (${p.nip}): event would carry secret-like material`)
    }
  }
}

const summarise = (relay: string, phases: ReadonlyArray<Phase>, requesterPk: string, providerPk: string) =>
  JSON.stringify(
    {
      ok: true,
      drill: "agents.nostr_fallback_coordination.v1",
      relay,
      requesterPubkey: requesterPk,
      providerPubkey: providerPk,
      events: phases.map(p => ({ phase: p.phase, nip: p.nip, note: p.note, kind: p.event.kind, id: p.event.id })),
    },
    null,
    2,
  )

const runPlan = () => {
  const { requesterPk, providerPk, phases } = buildDrill()
  assertNoSecrets(phases)
  console.log(summarise("(none: plan mode)", phases, requesterPk, providerPk))
}

const runSmoke = async (flags: Flags) => {
  const { requesterPk, providerPk, phases } = buildDrill()
  assertNoSecrets(phases)
  const relayUrl = relayWebSocketUrl(optionalString(flags, "relay", defaultRelay))

  // Pace publishes so we stay well under public-relay anti-spam rate limits.
  const publishDelayMs = Number(optionalString(flags, "publish-delay-ms", "750"))
  let first = true
  for (const p of phases) {
    if (!first) await sleep(publishDelayMs)
    first = false
    await publishEvent(relayUrl, p.event, p.authSecretKey)
  }
  const readBack: Array<string> = []
  for (const p of phases) {
    const gotId = await readEventId(relayUrl, p.event)
    if (gotId !== p.event.id) {
      throw new Error(`Read-back id mismatch for ${p.phase}: ${gotId} != ${p.event.id}`)
    }
    readBack.push(gotId)
  }

  console.log(summarise(relayUrl, phases, requesterPk, providerPk))
  console.error(`read-back verified ${readBack.length}/${phases.length} events on ${relayUrl}`)
}

const { command, flags } = parseFlags(process.argv.slice(2))

if (command === "smoke") {
  await runSmoke(flags)
} else if (command === "plan") {
  runPlan()
} else {
  console.log(usage())
  process.exit(command === "help" || command === "--help" ? 0 : 1)
}

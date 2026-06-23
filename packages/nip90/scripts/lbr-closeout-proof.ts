#!/usr/bin/env bun

/**
 * NIP-LBR labor-market closeout proof.
 *
 * Produces a real, dereferenceable labor-market closeout receipt for the
 * agentic-coding labor lane (DE-6 "open protocol markets" — the labor market's
 * dippable + receipt rung). It signs a complete LBR lifecycle (request `5934`,
 * quote `7000`, acceptance `7000`, result `6934`) with deterministic keys,
 * composes the four accepted events into one content-addressed
 * `LbrLaborCloseout`, and re-derives the digest to prove the receipt
 * dereferences the exact lifecycle that produced it.
 *
 * This is protocol-only and offline by default. It does NOT move sats, create
 * invoices, escrow funds, publish private material, or grant settlement
 * authority — the relay/transport is reference-only here, and a live paid labor
 * run remains the owner gate. The structure mirrors the NIP-DS
 * `apps/openagents.com/scripts/nip-ds.ts` proof: build → sign → bind → verify.
 *
 * Offline (default) — composes and dereferences the receipt with no network:
 *
 *   bun packages/nip90/scripts/lbr-closeout-proof.ts
 *
 * Relay smoke (optional) — also publishes the four events to a scoped market
 * relay and reads them back, proving the lifecycle is dippable on the relay
 * before the closeout binds it:
 *
 *   bun packages/nip90/scripts/lbr-closeout-proof.ts --relay https://relay.openagents.com
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
  makeLbrLaborCloseout,
  makeLbrQuote,
  makeLbrResult,
  verifyLbrLaborCloseoutDigest,
  type LbrLifecycleEvent,
} from "../src/index.js"
import {
  finalizeEvent,
  getPublicKey,
  type EventTemplate,
} from "nostr-effect/pure"

const requesterSecretKey = new Uint8Array(32).fill(7)
const providerSecretKey = new Uint8Array(32).fill(9)
const requesterPubkey = getPublicKey(requesterSecretKey)
const providerPubkey = getPublicKey(providerSecretKey)
const createdAt = 1_781_107_200

type SignedEvent = LbrLifecycleEvent & { tags: ReadonlyArray<ReadonlyArray<string>> }
type RelayMessage = ReadonlyArray<unknown>

const sign = (template: EventTemplate, secretKey: Uint8Array): SignedEvent => {
  const event = finalizeEvent(template, secretKey)
  return {
    id: event.id,
    pubkey: event.pubkey,
    kind: event.kind,
    created_at: event.created_at,
    tags: event.tags,
    content: event.content,
    sig: event.sig,
  }
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
    const timeout = setTimeout(
      () => reject(new Error("WebSocket open timed out")),
      10_000,
    )
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
    const timeout = setTimeout(
      () => reject(new Error(`Timed out waiting for ${label}`)),
      15_000,
    )
    socket.addEventListener("message", (event) => {
      const parsed = JSON.parse(String(event.data)) as RelayMessage
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

const publishAndReadBack = async (
  relayUrl: string,
  events: ReadonlyArray<SignedEvent>,
): Promise<ReadonlyArray<string>> => {
  for (const event of events) {
    const socket = new WebSocket(relayUrl)
    await waitForOpen(socket)
    socket.send(JSON.stringify(["EVENT", event]))
    const ok = await waitForMessage(
      socket,
      `OK for ${event.id}`,
      (message) => message[0] === "OK" && message[1] === event.id,
    )
    socket.close(1000, "publish complete")
    if (ok[2] !== true) {
      throw new Error(`Relay rejected kind ${event.kind}: ${JSON.stringify(ok)}`)
    }
  }

  const readIds: Array<string> = []
  for (const event of events) {
    const socket = new WebSocket(relayUrl)
    await waitForOpen(socket)
    const subId = `lbr-closeout-${event.kind}-${Date.now()}`
    socket.send(
      JSON.stringify(["REQ", subId, { ids: [event.id], kinds: [event.kind], limit: 1 }]),
    )
    const message = await waitForMessage(
      socket,
      `EVENT ${event.id}`,
      (candidate) =>
        candidate[0] === "EVENT" &&
        candidate[1] === subId &&
        typeof candidate[2] === "object" &&
        candidate[2] !== null &&
        (candidate[2] as { id?: unknown }).id === event.id,
    )
    socket.send(JSON.stringify(["CLOSE", subId]))
    socket.close(1000, "read complete")
    readIds.push((message[2] as { id: string }).id)
  }
  return readIds
}

const parseRelayFlag = (argv: ReadonlyArray<string>): string | undefined => {
  const index = argv.indexOf("--relay")
  if (index === -1) return undefined
  const value = argv[index + 1]
  if (value === undefined || value.startsWith("--")) {
    throw new Error("--relay requires a URL")
  }
  return value
}

const main = async () => {
  const relayInput = parseRelayFlag(process.argv.slice(2))
  // 1. Requester publishes a budgeted, output-only agentic-coding request.
  const request = makeLbrAgenticCodingRequest({
    objectiveRef: "objective.public.lbr.de6_proof",
    repositoryRefs: ["repo.public.openagents"],
    verificationCommandRef: "command.public.bun_test",
    requiredCapabilityRefs: ["capability.pylon.local_claude_agent"],
    bidMsats: 2_000_000,
    deadline: "deadline.public.lbr.20260630",
    forumTopicRef: "topic.public.forum.labor_de6_proof",
    relays: ["wss://relay.openagents.com"],
  })
  const requestDraft = lbrAgenticCodingRequestToDraft(request)
  const requestEvent = sign(
    { kind: requestDraft.kind, created_at: createdAt, tags: requestDraft.tags.map((t) => [...t]), content: requestDraft.content },
    requesterSecretKey,
  )
  if (requestEvent.kind !== LBR_AGENTIC_CODING_REQUEST_KIND) {
    throw new Error("request kind drift")
  }

  // 2. Provider quotes within budget (feedback 7000), referencing the request id.
  const quote = makeLbrQuote({
    requestId: requestEvent.id,
    requesterPubkey,
    amountMsats: 1_500_000,
    providerRef: "provider.public.pylon.local_claude",
    capabilityRefs: ["capability.pylon.local_claude_agent"],
    quoteRef: "quote.public.lbr.de6_proof",
    expiresAt: "expiry.public.lbr.20260630",
    requestRelay: "wss://relay.openagents.com",
  })
  const quoteDraft = lbrQuoteToDraft(quote)
  const quoteEvent = sign(
    { kind: quoteDraft.kind, created_at: createdAt, tags: quoteDraft.tags.map((t) => [...t]), content: quoteDraft.content },
    providerSecretKey,
  )
  if (quoteEvent.kind !== LBR_FEEDBACK_KIND) throw new Error("quote kind drift")

  // 3. Requester accepts and references a platform escrow receipt ref.
  const acceptance = makeLbrAcceptance({
    requestId: requestEvent.id,
    providerPubkey,
    escrowReceiptRef: "receipt.public.escrow.de6_proof",
    acceptanceRef: "acceptance.public.lbr.de6_proof",
    requestRelay: "wss://relay.openagents.com",
  })
  const acceptanceDraft = lbrAcceptanceToDraft(acceptance)
  const acceptanceEvent = sign(
    { kind: acceptanceDraft.kind, created_at: createdAt, tags: acceptanceDraft.tags.map((t) => [...t]), content: acceptanceDraft.content },
    requesterSecretKey,
  )

  // 4. Provider publishes an output-only result (6934) with artifact + receipt refs.
  const result = makeLbrResult({
    requestId: requestEvent.id,
    requesterPubkey,
    artifactRefs: ["artifact.public.lbr.patch_de6_proof"],
    platformCloseoutRef: "closeout.public.lbr.de6_proof",
    summaryRef: "summary.public.lbr.de6_proof",
    testRef: "test.public.lbr.bun",
    buildRef: "build.public.lbr.de6_proof",
    requestRelay: "wss://relay.openagents.com",
  })
  const resultDraft = lbrResultToDraft(result)
  const resultEvent = sign(
    { kind: resultDraft.kind, created_at: createdAt, tags: resultDraft.tags.map((t) => [...t]), content: resultDraft.content },
    providerSecretKey,
  )
  if (resultEvent.kind !== LBR_AGENTIC_CODING_RESULT_KIND) {
    throw new Error("result kind drift")
  }

  // Compose the four accepted events into one content-addressed closeout.
  const closeout = makeLbrLaborCloseout({
    requestEvent,
    quoteEvent,
    acceptanceEvent,
    resultEvent,
  })

  // Dereference: re-derive the digest/receiptRef from the receipt's own fields.
  const dereferenced = verifyLbrLaborCloseoutDigest(closeout)
  if (!dereferenced) {
    throw new Error("closeout receipt failed to dereference")
  }

  const orderedEvents = [requestEvent, quoteEvent, acceptanceEvent, resultEvent]

  let relayProof:
    | { relay: string; publishedReadBackIds: ReadonlyArray<string>; allReadBack: boolean }
    | undefined
  if (relayInput !== undefined) {
    const relayUrl = relayWebSocketUrl(relayInput)
    const readIds = await publishAndReadBack(relayUrl, orderedEvents)
    const allReadBack =
      readIds.length === orderedEvents.length &&
      orderedEvents.every((event, index) => readIds[index] === event.id)
    if (!allReadBack) {
      throw new Error("relay read-back did not return the published events")
    }
    relayProof = { relay: relayUrl, publishedReadBackIds: readIds, allReadBack }
  }

  console.log(
    JSON.stringify(
      {
        ok: true,
        note:
          "Protocol-only NIP-LBR labor-market closeout. No sats moved; no escrow; " +
          "settlement authority stays in the platform receipt systems. A live paid " +
          "labor run is the owner gate.",
        requesterPubkey,
        providerPubkey,
        signedEventIds: {
          request: requestEvent.id,
          quote: quoteEvent.id,
          acceptance: acceptanceEvent.id,
          result: resultEvent.id,
        },
        eventKinds: orderedEvents.map((event) => event.kind),
        receiptRef: closeout.receiptRef,
        digest: closeout.digest,
        closeoutDereferenced: dereferenced,
        ...(relayProof === undefined ? {} : { relayProof }),
        closeout,
      },
      null,
      2,
    ),
  )
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exit(1)
})

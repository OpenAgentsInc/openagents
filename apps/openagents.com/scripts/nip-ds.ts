#!/usr/bin/env bun

/**
 * NIP-DS dataset listing/offer helper for agents.
 *
 * Conversational use:
 * - Tell the agent what data to sell.
 * - Save the public-safe, redacted bundle to a local file.
 * - Run:
 *   bun apps/openagents.com/scripts/nip-ds.ts draft \
 *     --file ./bundle.json --title "Redacted conversation bundle" \
 *     --d redacted-conversation-bundle --price-sats 50
 * - For integrated relay proof:
 *   bun apps/openagents.com/scripts/nip-ds.ts smoke \
 *     --relay https://openagents-market-relay.openagents.workers.dev
 *
 * This script builds protocol objects through @openagentsinc/nip90, which reuses
 * the sibling nostr-effect implementation. It does not create invoices, move
 * sats, publish private bundles, or grant settlement authority.
 */

import { readFile } from "node:fs/promises"

import {
  KIND_DATASET_ACCESS_REQUEST,
  KIND_DATASET_ACCESS_RESULT,
  KIND_DATASET_LISTING,
  KIND_DATASET_OFFER,
  datasetAccessRequestToTags,
  datasetAccessResultToTags,
  datasetAddress,
  datasetListingToTags,
  datasetOfferAddress,
  datasetOfferToTags,
  makeDatasetAccessRequest,
  makeDatasetAccessResult,
  makeDatasetListing,
  makeDatasetOffer,
  sha256Hex,
  verifyDatasetDeliveryDescriptorDigest,
} from "@openagentsinc/nip90"
import {
  finalizeEvent,
  getPublicKey,
  type EventTemplate,
  type VerifiedEvent,
} from "../../../../nostr-effect/src/wrappers/pure.ts"

type Flags = Record<string, string | true>
type RelayMessage = ReadonlyArray<unknown>

const defaultRelay = "https://openagents-market-relay.openagents.workers.dev"
const sellerSecretKey = new Uint8Array(32).fill(1)
const buyerSecretKey = new Uint8Array(32).fill(2)

const usage = () => `Usage:
  bun apps/openagents.com/scripts/nip-ds.ts draft --file PATH --title TITLE --d SLUG [--price-sats 50]
  bun apps/openagents.com/scripts/nip-ds.ts smoke [--relay URL] [--file PATH]

Options:
  --access <mode>       open, paid, quote, targeted, subscription, or negotiated. Defaults to paid.
  --d <slug>            Stable dataset slug. Defaults to openagents-nip-ds-smoke.
  --file <path>         Public-safe redacted dataset bundle to digest.
  --price-sats <n>      Offer price in sats. Defaults to 50.
  --relay <url>         Relay URL. Defaults to ${defaultRelay}.
  --summary <text>      Short listing summary.
  --title <text>        Dataset listing title.
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

const requireString = (flags: Flags, name: string): string => {
  const value = flags[name]
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`Missing --${name}`)
  }
  return value
}

const optionalString = (
  flags: Flags,
  name: string,
  fallback: string,
): string => {
  const value = flags[name]
  return typeof value === "string" && value.length > 0 ? value : fallback
}

const readBundle = async (flags: Flags): Promise<string> => {
  const file = flags.file
  if (typeof file !== "string") {
    return JSON.stringify({
      kind: "conversation_bundle",
      records: [
        {
          role: "user",
          text: "public-safe redacted dataset sale smoke request",
        },
      ],
    })
  }

  return await readFile(file, "utf8")
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
    const seen: Array<RelayMessage> = []
    const timeout = setTimeout(
      () => reject(new Error(`Timed out waiting for ${label}; saw ${JSON.stringify(seen)}`)),
      15_000,
    )
    socket.addEventListener("message", event => {
      const parsed = JSON.parse(String(event.data)) as RelayMessage
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

const publishEvent = async (
  relayUrl: string,
  event: VerifiedEvent,
): Promise<RelayMessage> => {
  const socket = new WebSocket(relayUrl)
  await waitForOpen(socket)
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

const readEvent = async (
  relayUrl: string,
  event: VerifiedEvent,
): Promise<VerifiedEvent> => {
  const socket = new WebSocket(relayUrl)
  await waitForOpen(socket)
  const subscriptionId = `nip-ds-${event.kind}-${Date.now()}`
  socket.send(JSON.stringify(["REQ", subscriptionId, { ids: [event.id], kinds: [event.kind], limit: 1 }]))
  const message = await waitForMessage(
    socket,
    `EVENT ${event.id}`,
    candidate =>
      candidate[0] === "EVENT" &&
      candidate[1] === subscriptionId &&
      typeof candidate[2] === "object" &&
      candidate[2] !== null &&
      (candidate[2] as { id?: unknown }).id === event.id,
  )
  socket.send(JSON.stringify(["CLOSE", subscriptionId]))
  socket.close(1000, "read complete")
  return message[2] as VerifiedEvent
}

const sign = (
  template: EventTemplate,
  secretKey: Uint8Array,
): VerifiedEvent => finalizeEvent(template, secretKey)

const createDatasetFlow = async (flags: Flags) => {
  const payload = await readBundle(flags)
  const now = Math.floor(Date.now() / 1000)
  const sellerPubkey = getPublicKey(sellerSecretKey)
  const buyerPubkey = getPublicKey(buyerSecretKey)
  const d = optionalString(flags, "d", `openagents-nip-ds-smoke-${now}`)
  const title = optionalString(flags, "title", "OpenAgents NIP-DS smoke bundle")
  const priceSats = optionalString(flags, "price-sats", "50")
  const listingAddress = datasetAddress(sellerPubkey, d)
  const offerAddress = datasetOfferAddress(sellerPubkey, `${d}-offer`)
  const digest = sha256Hex(payload)
  const listing = makeDatasetListing({
    d,
    title,
    x: digest,
    publishedAt: now,
    content: optionalString(
      flags,
      "summary",
      "Public-safe redacted dataset bundle prepared for NIP-DS sale.",
    ),
    summary: optionalString(flags, "summary", "Public-safe redacted bundle."),
    datasetKind: "conversation_bundle",
    mime: "application/json",
    size: new TextEncoder().encode(payload).byteLength,
    access: optionalString(flags, "access", "paid") as never,
    delivery: ["nip90", "download"],
    topics: ["dataset", "openagents", "nip-ds"],
  })
  const offer = makeDatasetOffer({
    d: `${d}-offer`,
    listing: listingAddress,
    status: "active",
    delivery: ["nip90", "download"],
    content: `Access to ${title}. Delivery is verified by the listing digest.`,
    policy: "open_offer",
    price: [priceSats, "SAT"],
    payments: [["ln"], ["manual"]],
    license: "seller-license-v1",
    topics: ["dataset", "openagents", "nip-ds"],
  })
  const request = makeDatasetAccessRequest({
    listing: listingAddress,
    offer: offerAddress,
    sellerPubkey,
    bid: Number(priceSats) * 1000,
    delivery: "download",
    preview: "metadata_only",
    licenseAck: "seller-license-v1",
  })
  const descriptor = {
    dataset: listingAddress,
    offer: offerAddress,
    delivery: "download" as const,
    ref: `local-public-safe-bundle:${d}`,
    mime: "application/json",
    x: digest,
    license: "seller-license-v1",
  }
  const result = makeDatasetAccessResult({
    requestId: "00".repeat(32),
    customerPubkey: buyerPubkey,
    listing: listingAddress,
    offer: offerAddress,
    descriptor,
  })

  const listingEvent = sign({
    kind: KIND_DATASET_LISTING,
    created_at: now,
    tags: datasetListingToTags(listing).map(tag => [...tag]),
    content: listing.content,
  }, sellerSecretKey)
  const offerEvent = sign({
    kind: KIND_DATASET_OFFER,
    created_at: now,
    tags: datasetOfferToTags(offer).map(tag => [...tag]),
    content: offer.content,
  }, sellerSecretKey)
  const requestEvent = sign({
    kind: KIND_DATASET_ACCESS_REQUEST,
    created_at: now,
    tags: datasetAccessRequestToTags(request, {
      listing: listingAddress,
      offer: offerAddress,
    }).map(tag => [...tag]),
    content: request.content,
  }, buyerSecretKey)
  const resultEvent = sign({
    kind: KIND_DATASET_ACCESS_RESULT,
    created_at: now,
    tags: datasetAccessResultToTags(result, {
      listing: listingAddress,
      offer: offerAddress,
      descriptor,
    }).map(tag => [...tag]),
    content: result.content,
  }, sellerSecretKey)

  return {
    buyerPubkey,
    descriptor,
    digest,
    listingAddress,
    offerAddress,
    payload,
    sellerPubkey,
    events: [listingEvent, offerEvent, requestEvent, resultEvent],
  }
}

const runDraft = async (flags: Flags) => {
  requireString(flags, "file")
  requireString(flags, "title")
  requireString(flags, "d")
  const flow = await createDatasetFlow(flags)
  console.log(JSON.stringify({
    ok: true,
    listingAddress: flow.listingAddress,
    offerAddress: flow.offerAddress,
    digest: flow.digest,
    sellerPubkey: flow.sellerPubkey,
    buyerPubkey: flow.buyerPubkey,
    eventKinds: flow.events.map(event => event.kind),
    deliveryDigestVerified: verifyDatasetDeliveryDescriptorDigest(
      flow.descriptor,
      flow.payload,
    ),
    events: flow.events,
  }, null, 2))
}

const runSmoke = async (flags: Flags) => {
  const flow = await createDatasetFlow(flags)
  const relayUrl = relayWebSocketUrl(optionalString(flags, "relay", defaultRelay))

  for (const event of flow.events) {
    await publishEvent(relayUrl, event)
  }

  const readBack = []
  for (const event of flow.events) {
    readBack.push(await readEvent(relayUrl, event))
  }

  const deliveryDigestVerified = verifyDatasetDeliveryDescriptorDigest(
    flow.descriptor,
    flow.payload,
  )
  if (!deliveryDigestVerified) {
    throw new Error("Delivered bundle digest does not match listing digest")
  }

  console.log(JSON.stringify({
    ok: true,
    relay: relayUrl,
    listingAddress: flow.listingAddress,
    offerAddress: flow.offerAddress,
    digest: flow.digest,
    publishedEventIds: flow.events.map(event => event.id),
    readEventIds: readBack.map(event => event.id),
    deliveryDigestVerified,
  }, null, 2))
}

const { command, flags } = parseFlags(process.argv.slice(2))

if (command === "draft") {
  await runDraft(flags)
} else if (command === "smoke") {
  await runSmoke(flags)
} else {
  console.log(usage())
  process.exit(command === "help" || command === "--help" ? 0 : 1)
}

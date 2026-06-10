import { describe, expect, test } from "bun:test"
import {
  KIND_DATASET_ACCESS_REQUEST,
  KIND_DATASET_LISTING,
  KIND_DATASET_OFFER,
  KIND_JOB_TEXT_GENERATION,
  Nip90ProtocolError,
  datasetAccessRequestToTags,
  datasetAddress,
  datasetListingToTags,
  datasetOfferAddress,
  datasetOfferToTags,
  getResultKind,
  jobInput,
  jobParam,
  makeDatasetAccessRequest,
  makeDatasetListing,
  makeDatasetOffer,
  jobRequestToTags,
  makeJobRequest,
  parseDatasetListingEvent,
  parseDatasetOfferEvent,
  parseJobRequestEvent,
  sha256Hex,
  verifyDatasetDigest,
} from "./index.js"

const pubkey = "11".repeat(32)
const sig = "22".repeat(64)

describe("@openagents/nip90", () => {
  test("re-exports shared nostr-effect NIP-90 request helpers", () => {
    const request = makeJobRequest({
      kind: KIND_JOB_TEXT_GENERATION,
      inputs: [jobInput.text("Summarize this")],
      params: [jobParam("model", "openagents-text")],
      output: "text/plain",
      bid: 2500,
      relays: ["wss://relay.openagents.example"],
    })

    expect(getResultKind(request.kind)).toBe(6050)
    expect(jobRequestToTags(request).map((tag: readonly string[]) => [...tag])).toEqual([
      ["i", "Summarize this", "text"],
      ["output", "text/plain"],
      ["param", "model", "openagents-text"],
      ["bid", "2500"],
      ["relays", "wss://relay.openagents.example"],
    ])
  })

  test("validates event shape through Effect Schema and typed errors", () => {
    const parsed = parseJobRequestEvent({
      id: "aa".repeat(32),
      pubkey,
      created_at: 1_762_000_000,
      kind: 5050,
      tags: [
        ["i", "hello", "prompt"],
        ["bid", "1000"],
      ],
      content: "",
      sig,
    })

    expect(parsed.inputs[0]?.inputType).toBe("text")
    expect(parsed.bid).toBe(1000)
    expect(() => makeJobRequest({ kind: 6000 })).toThrow(Nip90ProtocolError)
  })

  test("re-exports shared NIP-DS listing, offer, and digest helpers", () => {
    const digest = sha256Hex("public-safe redacted bundle")
    const listingAddress = datasetAddress(pubkey, "redacted-conversation-bundle")
    const offerAddress = datasetOfferAddress(pubkey, "redacted-conversation-bundle-offer")
    const listing = makeDatasetListing({
      d: "redacted-conversation-bundle",
      title: "Redacted Conversation Bundle",
      x: digest,
      publishedAt: 1_781_000_000,
      content: "Redacted bundle for a small-sats NIP-DS sale.",
      datasetKind: "conversation_bundle",
      access: "paid",
      delivery: ["nip90", "download"],
    })
    const offer = makeDatasetOffer({
      d: "redacted-conversation-bundle-offer",
      listing: listingAddress,
      status: "active",
      delivery: ["nip90"],
      price: ["50", "SAT"],
      payments: [["ln"]],
    })
    const request = makeDatasetAccessRequest({
      listing: listingAddress,
      offer: offerAddress,
      sellerPubkey: pubkey,
      delivery: "download",
      bid: 50_000,
    })

    expect(KIND_DATASET_LISTING).toBe(30404)
    expect(KIND_DATASET_OFFER).toBe(30406)
    expect(KIND_DATASET_ACCESS_REQUEST).toBe(5960)
    expect(verifyDatasetDigest("public-safe redacted bundle", digest)).toBe(true)
    expect(datasetListingToTags(listing).map((tag: readonly string[]) => [...tag])).toContainEqual([
      "x",
      digest,
    ])
    expect(datasetOfferToTags(offer).map((tag: readonly string[]) => [...tag])).toContainEqual([
      "a",
      listingAddress,
    ])
    expect(datasetAccessRequestToTags(request, {
      listing: listingAddress,
      offer: offerAddress,
    }).map((tag: readonly string[]) => [...tag])).toContainEqual(["a", offerAddress])
    expect(parseDatasetListingEvent({
      id: "aa".repeat(32),
      pubkey,
      created_at: 1_781_000_000,
      kind: 30404,
      tags: datasetListingToTags(listing),
      content: listing.content,
      sig,
    }).x).toBe(digest)
    expect(parseDatasetOfferEvent({
      id: "bb".repeat(32),
      pubkey,
      created_at: 1_781_000_000,
      kind: 30406,
      tags: datasetOfferToTags(offer),
      content: offer.content,
      sig,
    }).listing).toBe(listingAddress)
    expect(() => parseDatasetOfferEvent({
      id: "cc".repeat(32),
      pubkey,
      created_at: 1_781_000_000,
      kind: 30406,
      tags: [
        ["d", "malformed"],
        ["a", listingAddress],
        ["status", "active"],
      ],
      content: "",
      sig,
    })).toThrow(Nip90ProtocolError)
  })
})

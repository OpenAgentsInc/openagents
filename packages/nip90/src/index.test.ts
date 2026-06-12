import { describe, expect, test } from "bun:test"
import {
  KIND_DATASET_ACCESS_REQUEST,
  KIND_DATASET_LISTING,
  KIND_DATASET_OFFER,
  KIND_JOB_LABOR_CODE_TASK,
  KIND_JOB_LABOR_REVIEW,
  KIND_JOB_TEXT_GENERATION,
  Nip90ProtocolError,
  PROVIDER_COMPLIANT_USAGE_LABOR_POLICY_REF,
  datasetAccessRequestToTags,
  datasetAddress,
  datasetListingToTags,
  datasetOfferAddress,
  datasetOfferToTags,
  getResultKind,
  jobInput,
  jobParam,
  laborJobRequestToTags,
  laborJobResultToTags,
  laborJobTypeForKind,
  makeDatasetAccessRequest,
  makeDatasetListing,
  makeDatasetOffer,
  makeLaborJobRequest,
  makeLaborJobResult,
  jobRequestToTags,
  makeJobRequest,
  parseDatasetListingEvent,
  parseDatasetOfferEvent,
  parseJobRequestEvent,
  parseLaborJobRequestEvent,
  parseLaborJobResultEvent,
  sha256Hex,
  verifyDatasetDigest,
} from "./index.js"

const pubkey = "11".repeat(32)
const sig = "22".repeat(64)

describe("@openagentsinc/nip90", () => {
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

  test("re-exports shared labor job request and result helpers", () => {
    const request = makeLaborJobRequest({
      jobType: "review",
      inputRefs: ["work-order.public.review-1"],
      acceptanceCriteria: ["review artifact is public-safe"],
      bid: 10_000,
      relays: ["wss://relay.openagents.example"],
    })
    const result = makeLaborJobResult({
      jobType: "review",
      requestId: "dd".repeat(32),
      customerPubkey: pubkey,
      artifactRefs: ["artifact.public.review-1"],
      content: '{"status":"accepted"}',
      amount: 10_000,
    })

    expect(KIND_JOB_LABOR_REVIEW).toBe(5935)
    expect(KIND_JOB_LABOR_CODE_TASK).toBe(5934)
    expect(laborJobTypeForKind(KIND_JOB_LABOR_REVIEW)).toBe("review")
    expect(request.policyRef).toBe(PROVIDER_COMPLIANT_USAGE_LABOR_POLICY_REF)
    expect(laborJobRequestToTags(request).map((tag: readonly string[]) => [...tag])).toContainEqual([
      "param",
      "acceptance",
      "review artifact is public-safe",
    ])
    expect(parseLaborJobRequestEvent({
      id: "aa".repeat(32),
      pubkey,
      created_at: 1_762_000_000,
      kind: KIND_JOB_LABOR_REVIEW,
      tags: laborJobRequestToTags(request),
      content: request.request.content,
      sig,
    }).jobType).toBe("review")
    expect(laborJobResultToTags(result).map((tag: readonly string[]) => [...tag])).toContainEqual([
      "artifact",
      "artifact.public.review-1",
    ])
    expect(parseLaborJobResultEvent({
      id: "bb".repeat(32),
      pubkey,
      created_at: 1_762_000_000,
      kind: 6935,
      tags: laborJobResultToTags(result),
      content: result.result.content,
      sig,
    }).artifactRefs).toEqual(["artifact.public.review-1"])
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

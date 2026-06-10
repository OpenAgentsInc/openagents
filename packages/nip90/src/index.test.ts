import { describe, expect, test } from "bun:test"
import {
  KIND_JOB_TEXT_GENERATION,
  Nip90ProtocolError,
  getResultKind,
  jobInput,
  jobParam,
  jobRequestToTags,
  makeJobRequest,
  parseJobRequestEvent,
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
})

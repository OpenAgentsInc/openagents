import { Schema as S } from "effect"
import { finalizeEvent, generateSecretKey } from "nostr-effect/pure"
import { describe, expect, it, vi } from "vitest"

import {
  NostrEvent,
  PUBLIC_CHAT_GROUP_ID,
  makePublicChatRelayClient,
} from "./index.js"

class TestSocket {
  readonly sent: string[] = []
  private readonly listeners = new Map<string, Array<(event: { data?: unknown }) => void>>()

  addEventListener(
    type: "open" | "message" | "close" | "error",
    listener: (event: { data?: unknown }) => void,
  ): void {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener])
  }
  close(): void {
    this.fire("close")
  }
  send(data: string): void {
    this.sent.push(data)
  }
  fire(type: string, data?: unknown): void {
    for (const listener of this.listeners.get(type) ?? []) listener({ data })
  }
}

describe("public chat relay client", () => {
  it("replays with an overlap, deduplicates, reaches current at EOSE, and maps OK", async () => {
    vi.useFakeTimers()
    const socket = new TestSocket()
    const secret = generateSecretKey()
    const event = S.decodeUnknownSync(NostrEvent)(
      finalizeEvent(
        {
          content: "hello",
          created_at: 1_000,
          kind: 9,
          tags: [["h", PUBLIC_CHAT_GROUP_ID]],
        },
        secret,
      ),
    )
    const client = makePublicChatRelayClient({
      now: () => 1_000_000,
      relayUrl: "wss://relay.example",
      webSocket: () => socket,
    })
    client.connect()
    socket.fire("open")
    const request = JSON.parse(socket.sent[0]!) as unknown[]
    expect(request[0]).toBe("REQ")
    const subscription = String(request[1])
    socket.fire("message", JSON.stringify(["EVENT", subscription, event]))
    socket.fire("message", JSON.stringify(["EVENT", subscription, event]))
    socket.fire("message", JSON.stringify(["EOSE", subscription]))
    expect(client.snapshot()).toMatchObject({
      events: [event],
      gapReason: null,
      lastCurrentAt: 1_000_000,
      state: "current",
    })

    const result = client.publish(event)
    socket.fire(
      "message",
      JSON.stringify(["OK", event.id, false, "rate-limited: slow down"]),
    )
    await expect(result).resolves.toEqual({
      reason: "rate-limited",
      state: "rejected",
    })
    client.close()
    vi.useRealTimers()
  })

  it("answers a NIP-42 challenge with the selected signer", async () => {
    const socket = new TestSocket()
    const secret = generateSecretKey()
    const client = makePublicChatRelayClient({
      relayUrl: "wss://relay.example",
      signer: {
        getPublicKey: async () => "unused",
        signEvent: async (template) =>
          S.decodeUnknownSync(NostrEvent)(finalizeEvent(template, secret)),
      },
      webSocket: () => socket,
    })
    client.connect()
    socket.fire("open")
    socket.fire("message", JSON.stringify(["AUTH", "challenge-1"]))
    await vi.waitFor(() => {
      expect(
        socket.sent.some((frame) => JSON.parse(frame)[0] === "AUTH"),
      ).toBe(true)
    })
    client.close()
  })
})

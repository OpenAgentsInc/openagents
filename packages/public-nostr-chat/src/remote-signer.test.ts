import { decrypt, encrypt, getConversationKey } from "nostr-effect/nip44"
import { NIP46_KIND } from "nostr-effect/nip46"
import {
  finalizeEvent,
  generateSecretKey,
  getPublicKey,
  verifyEvent,
} from "nostr-effect/pure"
import { describe, expect, it } from "vitest"

import {
  PUBLIC_CHAT_NIP46_PERMISSIONS,
  connectPublicChatRemoteSigner,
} from "./remote-signer.js"

class BunkerSocket {
  readonly remoteSecret = generateSecretKey()
  readonly remotePubkey = getPublicKey(this.remoteSecret)
  readonly sentMethods: string[] = []
  closed = false
  private readonly listeners = new Map<
    string,
    Array<(event: { data?: unknown }) => void>
  >()

  addEventListener(
    type: "open" | "message" | "close" | "error",
    listener: (event: { data?: unknown }) => void,
  ): void {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener])
  }
  close(): void {
    this.closed = true
    this.fire("close")
  }
  fire(type: string, data?: unknown): void {
    for (const listener of this.listeners.get(type) ?? []) listener({ data })
  }
  send(data: string): void {
    const frame = JSON.parse(data) as unknown[]
    if (frame[0] !== "EVENT") return
    const request = frame[1] as {
      content: string
      pubkey: string
    }
    const key = getConversationKey(this.remoteSecret, request.pubkey)
    const rpc = JSON.parse(decrypt(request.content, key)) as {
      id: string
      method: string
      params: string[]
    }
    this.sentMethods.push(rpc.method)
    let result: string
    if (rpc.method === "connect") {
      expect(rpc.params.at(-1)).toBe(PUBLIC_CHAT_NIP46_PERMISSIONS)
      result = "one-time"
    } else if (rpc.method === "get_public_key") {
      result = this.remotePubkey
    } else if (rpc.method === "sign_event") {
      result = JSON.stringify(
        finalizeEvent(JSON.parse(rpc.params[0]!), this.remoteSecret),
      )
    } else {
      result = "ack"
    }
    const response = finalizeEvent(
      {
        content: encrypt(JSON.stringify({ id: rpc.id, result }), key),
        created_at: Math.floor(Date.now() / 1_000),
        kind: NIP46_KIND as number,
        tags: [["p", request.pubkey]],
      },
      this.remoteSecret,
    )
    queueMicrotask(() =>
      this.fire(
        "message",
        JSON.stringify(["EVENT", "bunker-response", response]),
      ),
    )
  }
}

describe("generic NIP-46 remote signer", () => {
  it("connects with a one-time secret, requests the exact permissions, and verifies signatures", async () => {
    const socket = new BunkerSocket()
    const connection = connectPublicChatRemoteSigner({
      bunkerUrl: `bunker://${socket.remotePubkey}?relay=${encodeURIComponent(
        "wss://signer.example",
      )}&secret=one-time`,
      timeoutMs: 1_000,
      webSocket: () => socket,
    })
    setTimeout(() => socket.fire("open"), 0)
    const signer = await connection
    const event = await signer.signEvent({
      content: "portable",
      created_at: 1_000,
      kind: 9,
      tags: [["h", "another-public-group"]],
    })

    expect(signer.userPubkey).toBe(socket.remotePubkey)
    expect(verifyEvent({ ...event, tags: event.tags.map(tag => [...tag]) })).toBe(
      true,
    )
    expect(socket.sentMethods).toEqual([
      "connect",
      "get_public_key",
      "sign_event",
    ])
    await signer.switchRelays(["wss://another.example"])
    signer.disconnect()
    expect(socket.closed).toBe(true)
  })
})

import { Effect, Schema as S } from "effect"
import { decrypt, encrypt, getConversationKey } from "nostr-effect/nip44"
import {
  NIP46_KIND,
  decodeResponse,
  generateRequestId,
  parseBunkerUrl,
  type Nip46Response,
} from "nostr-effect/nip46"
import { makeAuthEvent } from "nostr-effect/nip42"
import {
  finalizeEvent,
  generateSecretKey,
  getPublicKey,
  verifyEvent,
} from "nostr-effect/pure"

import { NostrEvent, PUBLIC_CHAT_SIGNER_KINDS } from "./profile.js"
import type { PublicChatSigner } from "./client.js"

type RemoteSocket = Readonly<{
  addEventListener: (
    type: "open" | "message" | "close" | "error",
    listener: (event: { data?: unknown }) => void,
  ) => void
  close: (code?: number, reason?: string) => void
  send: (data: string) => void
}>

export type PublicChatRemoteSigner = PublicChatSigner &
  Readonly<{
    clientPubkey: string
    disconnect: () => void
    switchRelays: (relays: ReadonlyArray<string>) => Promise<void>
    userPubkey: string
  }>

export const PUBLIC_CHAT_NIP46_PERMISSIONS = PUBLIC_CHAT_SIGNER_KINDS.map(
  (kind) => kind,
)
  .filter((kind) => ![9002, 9005, 9010].includes(kind))
  .map((kind) => `sign_event:${kind}`)
  .join(",")

export const connectPublicChatRemoteSigner = async (input: Readonly<{
  bunkerUrl: string
  permissions?: string
  timeoutMs?: number
  webSocket?: (url: string) => RemoteSocket
}>): Promise<PublicChatRemoteSigner> => {
  const parsed = await Effect.runPromise(parseBunkerUrl(input.bunkerUrl))
  const relayUrl = parsed.relays.find((relay) => relay.startsWith("wss://"))
  if (relayUrl === undefined) throw new Error("nip46-no-secure-relay")

  const clientSecret = generateSecretKey()
  const clientPubkey = getPublicKey(clientSecret)
  const conversationKey = getConversationKey(
    clientSecret,
    parsed.remoteSignerPubkey,
  )
  const makeSocket =
    input.webSocket ??
    ((url: string) => new WebSocket(url) as unknown as RemoteSocket)
  const socket = makeSocket(relayUrl)
  const timeoutMs = input.timeoutMs ?? 30_000
  const pending = new Map<
    string,
    {
      reject: (error: Error) => void
      resolve: (response: Nip46Response) => void
      timer: ReturnType<typeof setTimeout>
    }
  >()
  let disconnected = false

  const rpc = (
    method: string,
    params: ReadonlyArray<string>,
  ): Promise<Nip46Response> => {
    if (disconnected) return Promise.reject(new Error("nip46-disconnected"))
    const id = generateRequestId()
    const content = encrypt(
      JSON.stringify({ id, method, params }),
      conversationKey,
    )
    const event = finalizeEvent(
      {
        content,
        created_at: Math.floor(Date.now() / 1_000),
        kind: NIP46_KIND as number,
        tags: [["p", parsed.remoteSignerPubkey]],
      },
      clientSecret,
    )
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        pending.delete(id)
        reject(new Error("nip46-timeout"))
      }, timeoutMs)
      pending.set(id, { reject, resolve, timer })
      socket.send(JSON.stringify(["EVENT", event]))
    })
  }

  const ready = new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("nip46-relay-timeout")), timeoutMs)
    socket.addEventListener("open", () => {
      clearTimeout(timer)
      socket.send(
        JSON.stringify([
          "REQ",
          `nip46-${clientPubkey.slice(0, 12)}`,
          { "#p": [clientPubkey], kinds: [NIP46_KIND] },
        ]),
      )
      resolve()
    })
    socket.addEventListener("error", () => {
      clearTimeout(timer)
      reject(new Error("nip46-relay-unavailable"))
    })
  })

  socket.addEventListener("message", (message) => {
    void (async () => {
      let frame: unknown
      try {
        frame = JSON.parse(String(message.data))
      } catch {
        return
      }
      if (!Array.isArray(frame)) return
      if (frame[0] === "AUTH" && typeof frame[1] === "string") {
        socket.send(
          JSON.stringify([
            "AUTH",
            finalizeEvent(makeAuthEvent(relayUrl, frame[1]), clientSecret),
          ]),
        )
        return
      }
      if (frame[0] !== "EVENT") return
      let event: NostrEvent
      try {
        event = S.decodeUnknownSync(NostrEvent)(frame[2])
      } catch {
        return
      }
      if (
        event.kind !== NIP46_KIND ||
        event.pubkey !== parsed.remoteSignerPubkey ||
        !event.tags.some((tag) => tag[0] === "p" && tag[1] === clientPubkey) ||
        !verifyEvent({ ...event, tags: event.tags.map((tag) => [...tag]) })
      ) {
        return
      }
      try {
        const response = await Effect.runPromise(
          decodeResponse(decrypt(event.content, conversationKey)),
        )
        const request = pending.get(response.id)
        if (request === undefined) return
        clearTimeout(request.timer)
        pending.delete(response.id)
        request.resolve(response)
      } catch {
        // A malformed or undecryptable response is not correlated to a request.
      }
    })()
  })
  socket.addEventListener("close", () => {
    disconnected = true
    clientSecret.fill(0)
    for (const request of pending.values()) {
      clearTimeout(request.timer)
      request.reject(new Error("nip46-disconnected"))
    }
    pending.clear()
  })

  let userPubkey: string
  try {
    await ready
    const connectParams: string[] = [parsed.remoteSignerPubkey]
    if (parsed.secret !== undefined) connectParams.push(parsed.secret)
    connectParams.push(input.permissions ?? PUBLIC_CHAT_NIP46_PERMISSIONS)
    const connection = await rpc("connect", connectParams)
    if (
      connection.error !== undefined ||
      (parsed.secret !== undefined &&
        connection.result !== "ack" &&
        connection.result !== parsed.secret)
    ) {
      throw new Error(connection.error ?? "nip46-secret-mismatch")
    }
    const identity = await rpc("get_public_key", [])
    if (
      identity.error !== undefined ||
      identity.result === undefined ||
      !/^[0-9a-f]{64}$/.test(identity.result)
    ) {
      throw new Error(identity.error ?? "nip46-invalid-user-pubkey")
    }
    userPubkey = identity.result
  } catch (error) {
    disconnected = true
    clientSecret.fill(0)
    socket.close(1_000, "nip46-handshake-failed")
    throw error
  }

  return {
    clientPubkey,
    disconnect: () => {
      disconnected = true
      for (const request of pending.values()) {
        clearTimeout(request.timer)
        request.reject(new Error("nip46-disconnected"))
      }
      pending.clear()
      clientSecret.fill(0)
      socket.close(1_000, "nip46-client-disconnect")
    },
    getPublicKey: async () => userPubkey,
    signEvent: async (template) => {
      const response = await rpc("sign_event", [JSON.stringify(template)])
      if (response.error !== undefined || response.result === undefined) {
        throw new Error(response.error ?? "nip46-empty-signature")
      }
      const signed = S.decodeUnknownSync(NostrEvent)(JSON.parse(response.result))
      if (
        signed.pubkey !== userPubkey ||
        signed.kind !== template.kind ||
        signed.content !== template.content ||
        signed.created_at !== template.created_at ||
        JSON.stringify(signed.tags) !== JSON.stringify(template.tags) ||
        !verifyEvent({ ...signed, tags: signed.tags.map((tag) => [...tag]) })
      ) {
        throw new Error("nip46-invalid-signature")
      }
      return signed
    },
    switchRelays: async (relays) => {
      const response = await rpc("switch_relays", [JSON.stringify(relays)])
      if (response.error !== undefined) throw new Error(response.error)
    },
    userPubkey,
  }
}

import { Schema as S } from "effect"
import { makeAuthEvent } from "nostr-effect/nip42"

import {
  NostrEvent,
  PUBLIC_CHAT_ACCEPTED_KINDS,
  PUBLIC_CHAT_GROUP_ID,
  PUBLIC_CHAT_GROUP_STATE_KINDS,
  PUBLIC_CHAT_LIMITS,
  PUBLIC_CHAT_MODERATION_KINDS,
  PublicChatRejectionReason,
  relayResultPrefix,
  stableChronological,
  hasGroupTag,
  validatePublicChatEvent,
  validateRelayGroupState,
} from "./profile.js"
import { verifyEvent } from "nostr-effect/pure"

export type PublicChatRelayState =
  | "disconnected"
  | "connecting"
  | "replaying"
  | "current"
  | "reconnecting"
  | "stale"

export type PublicChatPublishState =
  | "signing"
  | "publishing"
  | "accepted"
  | "rejected"

export type PublicChatSigner = Readonly<{
  getPublicKey: () => Promise<string>
  signEvent: (
    event: Readonly<{
      content: string
      created_at: number
      kind: number
      tags: string[][]
    }>,
  ) => Promise<NostrEvent>
}>

export type PublicChatRelaySnapshot = Readonly<{
  events: ReadonlyArray<NostrEvent>
  gapReason: string | null
  lastCurrentAt: number | null
  state: PublicChatRelayState
}>

type SocketLike = Readonly<{
  close: (code?: number, reason?: string) => void
  send: (data: string) => void
  addEventListener: (
    type: "open" | "message" | "close" | "error",
    listener: (event: { data?: unknown }) => void,
  ) => void
}>

export type PublicChatRelayClient = Readonly<{
  close: () => void
  connect: () => void
  loadOlder: () => void
  publish: (event: NostrEvent) => Promise<{
    reason?: PublicChatRejectionReason
    state: "accepted" | "rejected"
  }>
  snapshot: () => PublicChatRelaySnapshot
  subscribe: (listener: (snapshot: PublicChatRelaySnapshot) => void) => () => void
}>

export const makePublicChatRelayClient = (input: Readonly<{
  relayUrl: string
  groupId?: string
  acceptedKinds?: ReadonlyArray<number>
  groupStateKinds?: ReadonlyArray<number>
  signer?: PublicChatSigner
  webSocket?: (url: string) => SocketLike
  now?: () => number
  reconnectMs?: number
  relaySelfPubkey?: string
}>): PublicChatRelayClient => {
  const makeSocket =
    input.webSocket ??
    ((url: string) => new WebSocket(url) as unknown as SocketLike)
  const now = input.now ?? Date.now
  const reconnectMs = input.reconnectMs ?? 1_000
  const groupId = input.groupId ?? PUBLIC_CHAT_GROUP_ID
  const acceptedKinds: ReadonlyArray<number> =
    input.acceptedKinds ?? PUBLIC_CHAT_ACCEPTED_KINDS
  const groupStateKinds: ReadonlyArray<number> =
    input.groupStateKinds ?? PUBLIC_CHAT_GROUP_STATE_KINDS
  const events = new Map<string, NostrEvent>()
  const listeners = new Set<(snapshot: PublicChatRelaySnapshot) => void>()
  const pending = new Map<
    string,
    (result: {
      reason?: PublicChatRejectionReason
      state: "accepted" | "rejected"
    }) => void
  >()
  let socket: SocketLike | null = null
  let closed = false
  let reconnectTimer: ReturnType<typeof setTimeout> | undefined
  let state: PublicChatRelayState = "disconnected"
  let gapReason: string | null = null
  let lastCurrentAt: number | null = null
  let subscriptionId = "agentchat-0"
  let stateSubscriptionId: string | null = null
  const awaitingEose = new Set<string>()
  const pageSubscriptions = new Set<string>()
  const profileSubscriptions = new Set<string>()
  const profileAuthors = new Set<string>()

  const snapshot = (): PublicChatRelaySnapshot => ({
    events: stableChronological([...events.values()]),
    gapReason,
    lastCurrentAt,
    state,
  })
  const emit = (): void => {
    const value = snapshot()
    for (const listener of listeners) listener(value)
  }
  const setState = (
    next: PublicChatRelayState,
    nextGap: string | null = gapReason,
  ): void => {
    state = next
    gapReason = nextGap
    if (next === "current") lastCurrentAt = now()
    emit()
  }

  const subscribeFrames = (): void => {
    const latest = [...events.values()].reduce(
      (maximum, event) => Math.max(maximum, event.created_at),
      0,
    )
    subscriptionId = `agentchat-${now()}`
    awaitingEose.clear()
    awaitingEose.add(subscriptionId)
    socket?.send(
      JSON.stringify([
        "REQ",
        subscriptionId,
        {
          "#h": [groupId],
          kinds: [...acceptedKinds, ...PUBLIC_CHAT_MODERATION_KINDS],
          limit: PUBLIC_CHAT_LIMITS.historyPageSize,
          ...(latest === 0 ? {} : { since: Math.max(0, latest - 1) }),
        },
      ]),
    )
    if (input.relaySelfPubkey !== undefined) {
      stateSubscriptionId = `agentchat-state-${now()}`
      awaitingEose.add(stateSubscriptionId)
      socket?.send(
        JSON.stringify([
          "REQ",
          stateSubscriptionId,
          {
            "#d": [groupId],
            authors: [input.relaySelfPubkey],
            kinds: [...groupStateKinds],
          },
        ]),
      )
    }
  }

  const scheduleReconnect = (): void => {
    if (closed) return
    setState("stale", "relay-unavailable")
    reconnectTimer = setTimeout(() => {
      setState("reconnecting", "awaiting-eose")
      connect()
    }, reconnectMs)
  }

  const handleFrame = async (raw: unknown): Promise<void> => {
    let frame: unknown
    try {
      frame = JSON.parse(String(raw))
    } catch {
      gapReason = "invalid-relay-frame"
      emit()
      return
    }
    if (!Array.isArray(frame) || typeof frame[0] !== "string") return
    if (frame[0] === "AUTH" && typeof frame[1] === "string") {
      if (input.signer === undefined) return
      try {
        const event = await input.signer.signEvent(
          makeAuthEvent(input.relayUrl, frame[1]),
        )
        socket?.send(JSON.stringify(["AUTH", event]))
      } catch {
        gapReason = "signer-refused"
        emit()
      }
      return
    }
    if (
      frame[0] === "EOSE" &&
      typeof frame[1] === "string" &&
      awaitingEose.has(frame[1])
    ) {
      awaitingEose.delete(frame[1])
      if (awaitingEose.size === 0) setState("current", null)
      return
    }
    if (
      frame[0] === "EVENT" &&
      (frame[1] === subscriptionId ||
        frame[1] === stateSubscriptionId ||
        pageSubscriptions.has(String(frame[1])) ||
        profileSubscriptions.has(String(frame[1])))
    ) {
      try {
        const event = S.decodeUnknownSync(NostrEvent)(frame[2])
        const isProfile = profileSubscriptions.has(String(frame[1]))
        const validation = isProfile
          ? event.kind === 0 &&
            verifyEvent({
              ...event,
              tags: event.tags.map((tag) => [...tag]),
            })
            ? { ok: true as const }
            : { ok: false as const, reason: "signature-invalid" as const }
          : frame[1] === stateSubscriptionId &&
          input.relaySelfPubkey !== undefined
            ? validateRelayGroupState(
                event,
                input.relaySelfPubkey,
                groupId,
                groupStateKinds,
              )
            : PUBLIC_CHAT_MODERATION_KINDS.includes(event.kind as never)
              ? verifyEvent({
                    ...event,
                    tags: event.tags.map((tag) => [...tag]),
                  }) && hasGroupTag(event, groupId)
                ? { ok: true as const }
                : { ok: false as const, reason: "signature-invalid" as const }
              : validatePublicChatEvent(event, {
                acceptedKinds,
                groupId,
                nowSeconds: Math.floor(now() / 1_000),
                })
        if (!validation.ok) {
          gapReason = validation.reason
          emit()
          return
        }
        events.set(event.id, event)
        if (
          !isProfile &&
          acceptedKinds.includes(event.kind) &&
          !profileAuthors.has(event.pubkey)
        ) {
          profileAuthors.add(event.pubkey)
          const profileId = `agentchat-profile-${event.pubkey.slice(0, 12)}`
          profileSubscriptions.add(profileId)
          socket?.send(
            JSON.stringify([
              "REQ",
              profileId,
              { authors: [event.pubkey], kinds: [0], limit: 1 },
            ]),
          )
        }
        emit()
      } catch {
        gapReason = "invalid-event"
        emit()
      }
      return
    }
    if (
      frame[0] === "OK" &&
      typeof frame[1] === "string" &&
      typeof frame[2] === "boolean"
    ) {
      const resolve = pending.get(frame[1])
      if (resolve === undefined) return
      pending.delete(frame[1])
      resolve(
        frame[2]
          ? { state: "accepted" }
          : {
              reason: relayResultPrefix(String(frame[3] ?? "error")),
              state: "rejected",
            },
      )
      return
    }
    if (frame[0] === "CLOSED" || frame[0] === "NOTICE") {
      gapReason = String(frame.at(-1) ?? "relay-notice").slice(0, 256)
      emit()
    }
  }

  const connect = (): void => {
    if (closed || socket !== null) return
    setState("connecting", "awaiting-eose")
    try {
      const next = makeSocket(input.relayUrl)
      socket = next
      next.addEventListener("open", () => {
        setState("replaying", "awaiting-eose")
        subscribeFrames()
      })
      next.addEventListener("message", (event) => {
        void handleFrame(event.data)
      })
      next.addEventListener("error", () => {
        gapReason = "relay-unavailable"
        emit()
      })
      next.addEventListener("close", () => {
        socket = null
        if (state === "replaying") gapReason = "disconnect-before-eose"
        scheduleReconnect()
      })
    } catch {
      socket = null
      scheduleReconnect()
    }
  }

  const publish = (
    event: NostrEvent,
  ): Promise<{
    reason?: PublicChatRejectionReason
    state: "accepted" | "rejected"
  }> => {
    if (socket === null) {
      return Promise.resolve({
        reason: "relay-unavailable",
        state: "rejected",
      })
    }
    return new Promise((resolve) => {
      pending.set(event.id, resolve)
      socket?.send(JSON.stringify(["EVENT", event]))
      setTimeout(() => {
        if (!pending.has(event.id)) return
        pending.delete(event.id)
        resolve({ reason: "relay-unavailable", state: "rejected" })
      }, 10_000)
    })
  }

  return {
    close: () => {
      closed = true
      if (reconnectTimer !== undefined) clearTimeout(reconnectTimer)
      socket?.send(JSON.stringify(["CLOSE", subscriptionId]))
      if (stateSubscriptionId !== null) {
        socket?.send(JSON.stringify(["CLOSE", stateSubscriptionId]))
      }
      socket?.close(1_000, "agentchat-client-close")
      socket = null
      setState("disconnected", null)
    },
    connect,
    loadOlder: () => {
      if (socket === null) return
      const oldest = [...events.values()]
        .filter((event) =>
          acceptedKinds.includes(event.kind),
        )
        .reduce<number | undefined>(
          (minimum, event) =>
            minimum === undefined
              ? event.created_at
              : Math.min(minimum, event.created_at),
          undefined,
        )
      if (oldest === undefined) return
      const pageId = `agentchat-page-${now()}-${oldest}`
      pageSubscriptions.add(pageId)
      socket.send(
        JSON.stringify([
          "REQ",
          pageId,
          {
            "#h": [groupId],
            kinds: [...acceptedKinds, ...PUBLIC_CHAT_MODERATION_KINDS],
            limit: PUBLIC_CHAT_LIMITS.historyPageSize,
            until: oldest,
          },
        ]),
      )
    },
    publish,
    snapshot,
    subscribe: (listener) => {
      listeners.add(listener)
      listener(snapshot())
      return () => listeners.delete(listener)
    },
  }
}

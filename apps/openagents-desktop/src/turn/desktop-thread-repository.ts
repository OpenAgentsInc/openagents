import { randomUUID } from "node:crypto"

import { Effect, Layer } from "effect"

import {
  ThreadRepository,
  type ThreadRepositoryInterface,
  type ThreadTurnMessage,
} from "@openagentsinc/agent-turn-runtime"

import type { DesktopMessage } from "../chat-contract.ts"
import type { makeThreadStore } from "../thread-store.ts"

/**
 * AFS-01 Desktop transition adapter: `thread-store.ts` -> kernel
 * `ThreadRepository`.
 *
 * The kernel persists canonical user and assistant turn state through the
 * current thread authority. This adapter wraps the existing bounded composer
 * thread store; it copies none of the store's LRU/protection/acceptance-verdict
 * concerns into the core. The store remains the sole owner of thread identity,
 * bounds, and file layout.
 */
type ThreadStore = ReturnType<typeof makeThreadStore>

const timestamp = (): string => new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })

const toDesktopMessage = (role: ThreadTurnMessage["role"], message: ThreadTurnMessage): DesktopMessage => ({
  key: `${randomUUID()}-${role}`,
  role,
  text: message.text,
  timestamp: timestamp(),
})

/** Wrap a Desktop thread store as the kernel thread repository. */
export const makeDesktopThreadRepository = (store: ThreadStore): ThreadRepositoryInterface => ({
  exists: (threadRef) => Effect.sync(() => store.open(threadRef) !== null),
  appendUser: (threadRef, message) =>
    Effect.sync(() => {
      store.append(threadRef, toDesktopMessage("user", message))
    }),
  appendAssistant: (threadRef, message) =>
    Effect.sync(() => {
      store.append(threadRef, toDesktopMessage("assistant", message))
    }),
})

/** Layer form for host composition. */
export const desktopThreadRepositoryLayer = (store: ThreadStore): Layer.Layer<ThreadRepository> =>
  Layer.succeed(ThreadRepository, ThreadRepository.of(makeDesktopThreadRepository(store)))

export { ThreadRepository }

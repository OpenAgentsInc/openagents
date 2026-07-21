import { randomUUID } from "node:crypto"

import { Effect, Layer } from "effect"

import { decodeAppleFmRouteOutput } from "@openagentsinc/apple-fm-runtime"
import {
  ThreadRepository,
  type ThreadRepositoryInterface,
  type ThreadTurnMessage,
} from "@openagentsinc/agent-turn-runtime"

import { DELEGATE_CANDIDATES } from "./desktop-delegation.ts"

import type { DesktopMessage, DesktopMessageMeta } from "../chat-contract.ts"
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
 *
 * #9127 persistence hygiene: the canonical thread store carries CONVERSATION
 * content only.
 *
 * - An assistant answer's bounded provenance projects into the persisted note
 *   `meta` (provider/model/dataDestination/usageTruth), so a delegated
 *   subagent answer stays attributed after reload — the same metadata surface
 *   the message inspector already reads.
 * - The Apple FM router's guided ROUTE RECOMMENDATION frame (the JSON control
 *   output that selects a delegate) is not a conversation message. Persisting
 *   it would show raw route JSON as an assistant note on reload and feed it
 *   back into delegate/router history prompts. It is skipped fail-closed: only
 *   an exact, well-formed recommendation for an admitted delegate candidate,
 *   produced by the `apple_fm` router itself, is skipped.
 */
type ThreadStore = ReturnType<typeof makeThreadStore>

const timestamp = (): string => new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })

/** Bounded note metadata from the kernel's answer provenance (never a secret or path). */
const metaFromProvenance = (
  provenance: NonNullable<ThreadTurnMessage["provenance"]>,
): DesktopMessageMeta => ({
  provider: provenance.candidate.slice(0, 60),
  model: provenance.model.slice(0, 120),
  dataDestination: provenance.dataDestination.slice(0, 60),
  usageTruth: provenance.usageTruth.slice(0, 60),
})

/** True when an apple_fm assistant text is a guided route-recommendation control frame. */
const isRouteControlFrame = (message: ThreadTurnMessage): boolean =>
  message.provenance?.candidate === "apple_fm" &&
  decodeAppleFmRouteOutput({ raw: message.text, admittedCandidates: DELEGATE_CANDIDATES })._tag ===
    "Recommendation"

const toDesktopMessage = (role: ThreadTurnMessage["role"], message: ThreadTurnMessage): DesktopMessage => ({
  key: `${randomUUID()}-${role}`,
  role,
  text: message.text,
  timestamp: timestamp(),
  ...(message.provenance === undefined ? {} : { meta: metaFromProvenance(message.provenance) }),
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
      if (isRouteControlFrame(message)) return
      store.append(threadRef, toDesktopMessage("assistant", message))
    }),
})

/** Layer form for host composition. */
export const desktopThreadRepositoryLayer = (store: ThreadStore): Layer.Layer<ThreadRepository> =>
  Layer.succeed(ThreadRepository, ThreadRepository.of(makeDesktopThreadRepository(store)))

export { ThreadRepository }

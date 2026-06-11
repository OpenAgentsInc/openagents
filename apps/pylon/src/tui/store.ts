// Solid-side view state for the Pylon dashboard (issue #4737).
//
// This module holds the signals/stores the components render from. It must
// not import Effect — the bridge (`bridge.ts`) is the only module that
// touches both worlds. Setters here are called by the bridge (runtime
// events/state) and by view-local interactions (composer chat items).

import { createSignal } from "solid-js"
import { createStore, produce } from "solid-js/store"
import {
  formatLogTimestamp,
  initialOperatorPaneState,
  initialTelemetryPaneState,
  initialWalletPaneState,
  isLogEntryVisible,
  type PylonLogEntry,
  type TelemetryPaneState,
  type WalletPaneState,
} from "../node/state"

// Runtime-toggleable verbosity (issue #4738's verbose toggle command; the
// initial value comes from --verbose / PYLON_VERBOSE at startup). Applies to
// entries appended after a change.
export const [verboseMode, setVerboseMode] = createSignal(false)

export const [walletState, setWalletState] = createSignal<WalletPaneState>(initialWalletPaneState)
export const [telemetryState, setTelemetryState] = createSignal<TelemetryPaneState>(initialTelemetryPaneState)
export const [operatorText, setOperatorText] = createSignal<string>(initialOperatorPaneState.text)

// The feed renders heterogeneous markdown items: runtime log lines and
// composer/diagnostic chat items (which stream content updates in place).
export type FeedItem = {
  id: number
  markdown: string
  tone: "log" | "logError" | "chat"
  streaming: boolean
}

export const maxFeedItems = 1000

const [feedItemsStore, setFeedItems] = createStore<FeedItem[]>([])
export const feedItems = feedItemsStore

let nextFeedId = 1

function pushFeedItem(item: Omit<FeedItem, "id">): number {
  const id = nextFeedId
  nextFeedId += 1
  setFeedItems(
    produce((items) => {
      items.push({ ...item, id })
      if (items.length > maxFeedItems) {
        items.splice(0, items.length - maxFeedItems)
      }
    }),
  )
  return id
}

// Runtime log entries: filtered by verbosity, rendered as dim one-liners.
export function appendRuntimeLogEntry(entry: PylonLogEntry, verbose: boolean = verboseMode()): void {
  if (!isLogEntryVisible(entry, verbose)) return
  pushFeedItem({
    markdown: `[${formatLogTimestamp(entry.at)}] ${entry.message}`,
    tone: entry.level === "error" ? "logError" : "log",
    streaming: false,
  })
}

// Chat items: composer prompts/responses and verbose boot diagnostics.
// Returned handle lets the producer stream content updates in place.
export interface ChatFeedItemHandle {
  update: (markdown: string) => void
  finish: () => void
}

export function appendChatFeedItem(markdown: string, options?: { streaming?: boolean }): ChatFeedItemHandle {
  const id = pushFeedItem({ markdown, tone: "chat", streaming: options?.streaming ?? false })
  const apply = (patch: Partial<Omit<FeedItem, "id">>) => {
    setFeedItems(
      produce((items) => {
        const item = items.find((candidate) => candidate.id === id)
        if (item) Object.assign(item, patch)
      }),
    )
  }
  return {
    update: (next) => apply({ markdown: next }),
    finish: () => apply({ streaming: false }),
  }
}

// Test/maintenance helper: reset all view state.
export function resetViewState(): void {
  setVerboseMode(false)
  setFeedItems([])
  nextFeedId = 1
  setWalletState(initialWalletPaneState)
  setTelemetryState(initialTelemetryPaneState)
  setOperatorText(initialOperatorPaneState.text)
}

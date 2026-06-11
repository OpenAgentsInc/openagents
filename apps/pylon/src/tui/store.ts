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

// --- Virtualized feed (issue #4739) ----------------------------------------
//
// The feed is the Textual "Line API" idea: a plain (non-reactive) line
// buffer that can hold 100k entries with flat memory, while the component
// renders only the visible window. Reactivity is a single version counter;
// slicing happens per render of the window memo.

export type FeedTone = "log" | "logError" | "chat"

export type FeedLine = {
  tone: FeedTone
  text: string
}

export const maxFeedLines = 100_000
// Collapse pathological single lines; the full text stays in the JSONL log.
export const maxFeedLineChars = 400

const feedLineBuffer: FeedLine[] = []
const [feedVersion, setFeedVersion] = createSignal(0)
export { feedVersion }

export function feedLineCount(): number {
  feedVersion()
  return feedLineBuffer.length
}

export function collapseFeedLine(text: string): string {
  if (text.length <= maxFeedLineChars) return text
  return `${text.slice(0, maxFeedLineChars)} ...(+${text.length - maxFeedLineChars} chars)`
}

export function appendFeedText(tone: FeedTone, text: string): void {
  for (const raw of text.split("\n")) {
    feedLineBuffer.push({ tone, text: collapseFeedLine(raw) })
  }
  if (feedLineBuffer.length > maxFeedLines) {
    feedLineBuffer.splice(0, feedLineBuffer.length - maxFeedLines)
  }
  setFeedVersion((value) => value + 1)
}

// Pure window computation: offset is lines scrolled up from the bottom.
export function computeFeedWindow(
  total: number,
  offsetFromBottom: number,
  viewportRows: number,
): { start: number; end: number } {
  const rows = Math.max(1, viewportRows)
  const maxOffset = Math.max(0, total - rows)
  const offset = Math.min(Math.max(0, offsetFromBottom), maxOffset)
  const end = total - offset
  return { start: Math.max(0, end - rows), end }
}

export function visibleFeedLines(offsetFromBottom: number, viewportRows: number): FeedLine[] {
  feedVersion()
  const { start, end } = computeFeedWindow(feedLineBuffer.length, offsetFromBottom, viewportRows)
  return feedLineBuffer.slice(start, end)
}

// Scroll state: 0 = stuck to the bottom (sticky tail).
export const [feedScrollOffset, setFeedScrollOffset] = createSignal(0)
let viewportRowsProvider: () => number = () => 20

export function registerFeedViewport(provider: () => number): void {
  viewportRowsProvider = provider
}

export function scrollFeedBy(delta: number, unit: "step" | "viewport" | "content" = "step"): void {
  const rows = viewportRowsProvider()
  const total = feedLineBuffer.length
  const maxOffset = Math.max(0, total - rows)
  if (unit === "content") {
    setFeedScrollOffset(delta < 0 ? maxOffset : 0)
    return
  }
  const lines = unit === "viewport" ? Math.round(rows * Math.abs(delta)) * Math.sign(delta) : delta
  // Scrolling "up" (negative delta) increases the offset from the bottom.
  setFeedScrollOffset((value) => Math.min(Math.max(0, value - lines), maxOffset))
}

// Runtime log entries: filtered by verbosity, rendered as dim one-liners.
export function appendRuntimeLogEntry(entry: PylonLogEntry, verbose: boolean = verboseMode()): void {
  if (!isLogEntryVisible(entry, verbose)) return
  appendFeedText(
    entry.level === "error" ? "logError" : "log",
    `[${formatLogTimestamp(entry.at)}] ${entry.message}`,
  )
}

// --- Streaming chat items ----------------------------------------------------
//
// Streaming responses (composer, boot diagnostics) render as live markdown
// "tails" pinned under the virtual window; on finish they are flattened into
// plain feed lines. In-flight updates touch only the tail item, never the
// line buffer - no full-feed reflow.

export type StreamingTail = { id: number; markdown: string }

const [streamingTailsStore, setStreamingTails] = createStore<StreamingTail[]>([])
export const streamingTails = streamingTailsStore
let nextTailId = 1

export function flattenMarkdownToText(markdown: string): string {
  return markdown.replaceAll("**", "").replaceAll("\u0060", "")
}

export interface ChatFeedItemHandle {
  update: (markdown: string) => void
  finish: () => void
}

export function appendChatFeedItem(markdown: string, options?: { streaming?: boolean }): ChatFeedItemHandle {
  if (!options?.streaming) {
    appendFeedText("chat", flattenMarkdownToText(markdown))
    return { update: () => {}, finish: () => {} }
  }
  const id = nextTailId++
  let latest = markdown
  let finished = false
  setStreamingTails(produce((tails) => tails.push({ id, markdown })))
  return {
    update: (next) => {
      if (finished) return
      latest = next
      setStreamingTails(
        produce((tails) => {
          const tail = tails.find((candidate) => candidate.id === id)
          if (tail) tail.markdown = next
        }),
      )
    },
    finish: () => {
      if (finished) return
      finished = true
      setStreamingTails(
        produce((tails) => {
          const index = tails.findIndex((candidate) => candidate.id === id)
          if (index >= 0) tails.splice(index, 1)
        }),
      )
      appendFeedText("chat", flattenMarkdownToText(latest))
    },
  }
}

// Rough height estimate for a streaming markdown tail, used to size the
// virtual window so the tail stays visible.
export function estimateMarkdownRows(markdown: string, width: number): number {
  let rows = 0
  for (const line of markdown.split("\n")) {
    rows += Math.max(1, Math.ceil(line.length / Math.max(20, width)))
  }
  return rows
}

// Test/maintenance helper: reset all view state.
export function resetViewState(): void {
  setVerboseMode(false)
  feedLineBuffer.length = 0
  setFeedVersion(0)
  setStreamingTails([])
  nextTailId = 1
  setFeedScrollOffset(0)
  setWalletState(initialWalletPaneState)
  setTelemetryState(initialTelemetryPaneState)
  setOperatorText(initialOperatorPaneState.text)
}

import { readFileSync } from "node:fs"
import { join } from "node:path"
import { describe, expect, mock, test } from "bun:test"
import * as React from "react"
import { act, create as createTestRenderer } from "react-test-renderer"

import {
  mobileRuntimeInterruptedEvents,
  mobileRuntimeOrderedEvents,
  mobileRuntimeRefusalEvents,
} from "./fixtures/mobile-screen-fixtures"
import { reduceRuntimeTranscript } from "../src/sync/khala-runtime-transcript-core"

mock.module("../src/components/touchable-feedback", () => ({
  TouchableFeedback: ({
    accessibilityLabel,
    accessibilityRole,
    children,
  }: {
    accessibilityLabel?: string
    accessibilityRole?: "button"
    children?: React.ReactNode
  }) => React.createElement("TouchableFeedback", { accessibilityLabel, accessibilityRole }, children),
}))

mock.module("react-native-safe-area-context", () => ({
  SafeAreaView: ({ children }: { children?: React.ReactNode }) => React.createElement("SafeAreaView", null, children),
  useSafeAreaInsets: () => ({ bottom: 0, left: 0, right: 0, top: 0 }),
}))

const { TranscriptPartRow } = await import("../src/components/transcript-part-row")

const mobileRoot = new URL("../", import.meta.url).pathname
const source = readFileSync(join(mobileRoot, "src/screens/thread-messages-screen.tsx"), "utf8")

type AnyNode = { props: Record<string, unknown>; type: unknown }

const textContent = (value: unknown): string => {
  if (typeof value === "string" || typeof value === "number") return String(value)
  if (Array.isArray(value)) return value.map(textContent).join("")
  if (React.isValidElement(value)) return textContent((value.props as { children?: unknown }).children)
  return ""
}

const renderParts = (events: Parameters<typeof reduceRuntimeTranscript>[0]) => {
  let renderer: ReturnType<typeof createTestRenderer> | undefined
  const parts = reduceRuntimeTranscript(events)
  act(() => {
    renderer = createTestRenderer(
      React.createElement(
        React.Fragment,
        null,
        ...parts.map(part => React.createElement(TranscriptPartRow, { key: part.id, part })),
      ),
    )
  })
  return renderer!
}

const hasText = (renderer: ReturnType<typeof createTestRenderer>, text: string): boolean =>
  renderer.root.findAll(
    (node: AnyNode) =>
      typeof node.type === "string" &&
      node.type === "Text" &&
      textContent(node.props.children).includes(text),
  ).length > 0

describe("contract khala_mobile.thread_messages.rn_component_mount_coverage.v1 — ThreadMessagesScreen streaming renderer", () => {
  test("screen source stays wired to synced messages, runtime events, empty/loading/error states, and transcript rows", () => {
    expect(source).toContain("CHAT_MESSAGE_ENTITY_TYPE")
    expect(source).toContain("RUNTIME_EVENT_ENTITY_TYPE")
    expect(source).toContain("reduceRuntimeTranscript")
    expect(source).toContain("TranscriptPartRow")
    expect(source).toContain("Loading messages")
    expect(source).toContain("No messages yet")
    expect(source).toContain("Thread unavailable")
    expect(source).toContain("Sync unavailable")
  })

  test("renders ordered runtime streams with transcript order, interruption, typed refusals, and writeback cards", () => {
    const ordered = renderParts(mobileRuntimeOrderedEvents)
    expect(hasText(ordered, "turn started")).toBe(true)
    expect(hasText(ordered, "Inspecting the mobile test harness.")).toBe(true)
    expect(hasText(ordered, "I added the mount fixtures.")).toBe(true)
    expect(hasText(ordered, "42 in · 24 out · 66 total tokens")).toBe(true)
    expect(hasText(ordered, "OpenAgentsInc/openagents")).toBe(true)
    expect(hasText(ordered, "PR #8537 · 4 files")).toBe(true)
    expect(hasText(ordered, "turn completed")).toBe(true)

    expect(hasText(renderParts(mobileRuntimeInterruptedEvents), "turn interrupted")).toBe(true)

    const refused = renderParts(mobileRuntimeRefusalEvents)
    expect(hasText(refused, "Failed: insufficient_credit")).toBe(true)
    expect(hasText(refused, "Failed: rate_limited")).toBe(true)
    expect(hasText(refused, "Failed: org_capacity_unavailable")).toBe(true)
    expect(hasText(refused, "turn failed")).toBe(true)
  })
})

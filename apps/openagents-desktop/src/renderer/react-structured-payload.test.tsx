/**
 * Oracle for openagents_desktop.chat.structured_payload_card.v1
 *
 * Presentation layer: the owner-facing rendering. A message body that is (or
 * embeds) a JSON payload renders as a collapsible card — the Full Auto mission
 * packet as a purpose-built mission card with the objective/done condition
 * readable — never a raw inline JSON blob with a "Show full message" dump, and
 * "copy raw" is preserved.
 */
import { afterEach, describe, expect, test } from "vite-plus/test"
import { Effect } from "@effect-native/core/effect"
import type { IntentReporter } from "@effect-native/core"
import { Window } from "happy-dom"
import { createRoot } from "react-dom/client"

import { StructuredPayloadCard } from "./react-structured-payload.tsx"
import { detectStructuredPayload, FULL_AUTO_MISSION_SCHEMA_ID } from "./structured-payload.ts"
import { TimelineItem, type ReactTimelineRecord } from "./react-timeline.tsx"

const restores: Array<() => void> = []
const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0))
const report: IntentReporter = () => Effect.void

const installDom = (): { container: HTMLElement } => {
  const window = new Window({ url: "http://localhost/" })
  const values: Record<string, unknown> = {
    window,
    document: window.document,
    navigator: window.navigator,
    Node: window.Node,
    Element: window.Element,
    HTMLElement: window.HTMLElement,
    Event: window.Event,
    MouseEvent: window.MouseEvent,
  }
  for (const [name, value] of Object.entries(values)) {
    const previous = Object.getOwnPropertyDescriptor(globalThis, name)
    Object.defineProperty(globalThis, name, { configurable: true, writable: true, value })
    restores.push(() => {
      if (previous === undefined) delete (globalThis as Record<string, unknown>)[name]
      else Object.defineProperty(globalThis, name, previous)
    })
  }
  const container = window.document.createElement("div")
  window.document.body.appendChild(container)
  return { container: container as unknown as HTMLElement }
}

afterEach(async () => {
  // Flush any pending react-dom scheduler macrotask while the DOM globals still
  // exist, then restore — otherwise a late scheduler tick sees `window` removed.
  await new Promise((resolve) => setTimeout(resolve, 0))
  while (restores.length > 0) restores.pop()?.()
})

const missionPacket = {
  schema: FULL_AUTO_MISSION_SCHEMA_ID,
  runRef: "run-42",
  threadRef: "thread-42",
  objective: "Render the Full Auto mission packet as a mission card, not raw JSON",
  doneCondition: "The first run message reads as a mission card and the objective is fully visible",
  objectiveSource: "user",
  workspaceRef: "/workspace",
  currentLane: "codex-local",
  accountRef: null,
  continuationOrdinal: 1,
  turnCap: 40,
  remainingTurnsIncludingThisOne: 40,
}

describe("openagents_desktop.chat.structured_payload_card.v1 — rendering", () => {
  test("the mission packet renders as a mission card with the verbatim objective and done condition", async () => {
    const { container } = installDom()
    const detection = detectStructuredPayload(JSON.stringify(missionPacket, null, 2))
    expect(detection?.kind).toBe("mission")
    if (detection === null) throw new Error("expected detection")
    const root = createRoot(container)
    root.render(<StructuredPayloadCard detection={detection} itemKey="mission-1" />)
    await settle()

    const card = container.querySelector('[data-kind="full_auto_mission"]')
    expect(card).not.toBeNull()
    // A small schema-name chip, not a raw blob.
    expect(container.querySelector(".oa-react-payload-chip")?.textContent).toContain("MISSION")
    // The objective and done condition are fully present (never truncated away).
    expect(container.querySelector(".oa-react-mission-objective")?.textContent).toBe(
      missionPacket.objective,
    )
    expect(container.textContent).toContain(missionPacket.doneCondition)
    // The lane and turn budget are surfaced as quiet labels.
    expect(container.textContent).toContain("codex-local")
    expect(container.textContent).toContain("1 of 40")
    // No raw "Show full message" truncation control anywhere in the card.
    expect(container.textContent).not.toContain("Show full message")
    // "copy raw" is preserved, and the raw JSON packet is available.
    expect(container.querySelector(".oa-react-payload-copy")).not.toBeNull()
    expect(container.querySelector(".oa-react-payload-raw code")?.textContent).toContain(
      FULL_AUTO_MISSION_SCHEMA_ID,
    )
    root.unmount()
  })

  test("a generic JSON payload renders as a structured key/value tree card", async () => {
    const { container } = installDom()
    const detection = detectStructuredPayload('{"model":"gpt-5","tokens":128,"streamed":true}')
    expect(detection?.kind).toBe("json")
    if (detection === null) throw new Error("expected detection")
    const root = createRoot(container)
    root.render(<StructuredPayloadCard detection={detection} itemKey="json-1" />)
    await settle()

    expect(container.querySelector('[data-kind="structured_payload"]')).not.toBeNull()
    expect(container.querySelector(".oa-react-payload-chip")?.textContent).toContain(
      "STRUCTURED PAYLOAD",
    )
    const keys = [...container.querySelectorAll(".oa-react-payload-key")].map(
      (node) => node.textContent,
    )
    expect(keys).toContain('"model"')
    expect(keys).toContain('"tokens"')
    expect(container.querySelector(".oa-react-payload-string")?.textContent).toContain("gpt-5")
    expect(container.querySelector(".oa-react-payload-copy")).not.toBeNull()
    root.unmount()
  })

  test("TimelineItem routes a mission-packet user message to the card instead of a raw bubble", async () => {
    const { container } = installDom()
    const record: ReactTimelineRecord = {
      key: "m1",
      itemRef: "m1",
      sequence: 0,
      kind: "local_message",
      label: "You",
      body: JSON.stringify(missionPacket, null, 2),
      timestamp: "18:00",
      status: null,
      redacted: false,
      fields: [],
      resultRef: null,
      resultBody: null,
      resultStatus: null,
    }
    const root = createRoot(container)
    root.render(<TimelineItem record={record} report={report} />)
    await settle()

    // The mission card is rendered; the raw user chat bubble and its
    // "Show full message" toggle are not.
    expect(container.querySelector('[data-kind="full_auto_mission"]')).not.toBeNull()
    expect(container.querySelector(".oa-react-user-message-bubble")).toBeNull()
    expect(container.textContent).not.toContain("Show full message")
    expect(container.querySelector(".oa-react-mission-objective")?.textContent).toBe(
      missionPacket.objective,
    )
    root.unmount()
  })

  test("an ordinary assistant message is unaffected and stays a normal bubble", async () => {
    const { container } = installDom()
    const record: ReactTimelineRecord = {
      key: "a1",
      itemRef: "a1",
      sequence: 1,
      kind: "local_message",
      label: "Assistant",
      body: "Here is a normal answer with no JSON payload.",
      timestamp: "18:01",
      status: null,
      redacted: false,
      fields: [],
      resultRef: null,
      resultBody: null,
      resultStatus: null,
    }
    const root = createRoot(container)
    root.render(<TimelineItem record={record} report={report} />)
    await settle()
    expect(container.querySelector('[data-kind="full_auto_mission"]')).toBeNull()
    expect(container.querySelector('[data-kind="structured_payload"]')).toBeNull()
    expect(container.textContent).toContain("Here is a normal answer")
    root.unmount()
  })
})

import { afterEach, describe, expect, test } from "vite-plus/test"
import { resolveIntentRef, type IntentReporter } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"
import { Window } from "happy-dom"
import { createRoot } from "react-dom/client"

import type { CodexHistoryItem } from "../codex-history-contract.ts"
import {
  ConversationTimeline,
  ReactTimeline,
  SafeReactMarkdown,
  formatReactTimelineTimestamp,
  deriveReactTimelineTurns,
  deriveAssistantMetaKeys,
  projectLocalTimelineRecords,
  projectReactTimelineRecords,
  shouldCollapseUserMessage,
  type ReactTimelineRecord,
} from "./react-timeline.tsx"

const restores: Array<() => void> = []
const resizeObservers = new Set<{ callback: ResizeObserverCallback }>()
const flushResizeObservers = async (): Promise<void> => {
  for (const observer of resizeObservers) observer.callback([], observer as unknown as ResizeObserver)
  await new Promise(resolve => setTimeout(resolve, 20))
}
const installDom = () => {
  const window = new Window({ url: "http://localhost/" })
  class TestResizeObserver {
    readonly callback: ResizeObserverCallback
    constructor(callback: ResizeObserverCallback) { this.callback = callback; resizeObservers.add(this) }
    observe(): void {}
    unobserve(): void {}
    disconnect(): void { resizeObservers.delete(this) }
  }
  const values = {
    window,
    document: window.document,
    navigator: window.navigator,
    Node: window.Node,
    Element: window.Element,
    HTMLElement: window.HTMLElement,
    Event: window.Event,
    MouseEvent: window.MouseEvent,
    ResizeObserver: TestResizeObserver,
  }
  const previous = new Map<string, PropertyDescriptor | undefined>()
  for (const [name, value] of Object.entries(values)) {
    previous.set(name, Object.getOwnPropertyDescriptor(globalThis, name))
    Object.defineProperty(globalThis, name, { configurable: true, writable: true, value })
  }
  const originalRect = window.HTMLElement.prototype.getBoundingClientRect
  const originalScrollTo = window.HTMLElement.prototype.scrollTo
  window.HTMLElement.prototype.scrollTo = function(this: HTMLElement, options?: ScrollToOptions | number, y?: number) {
    this.scrollTop = typeof options === "number" ? (y ?? 0) : (options?.top ?? this.scrollTop)
  }
  window.HTMLElement.prototype.getBoundingClientRect = function(this: unknown) {
    const element = this as unknown as HTMLElement
    const key = element.dataset.timelineKey ?? element.dataset.messageId
    if (!key) return { x: 0, y: 0, top: 0, left: 0, right: 720, bottom: 100, width: 720, height: 100, toJSON: () => ({}) } as unknown as DOMRect
    const scroll = element.closest<HTMLElement>(".oa-react-timeline-scroll")
    const rows = scroll === null ? [] : [...scroll.querySelectorAll<HTMLElement>("[data-message-id]")]
    const rowKey = (row: HTMLElement): string | undefined => row.dataset.messageId ?? row.dataset.timelineKey
    const rowHeight = (row: HTMLElement): number => rowKey(row) === "a" ? 60 : rowKey(row) === "c" ? 80 : 40
    const measured = element.dataset.messageId ? element : element.closest<HTMLElement>("[data-message-id]") ?? element
    const height = rowHeight(measured)
    const top = rows.slice(0, rows.indexOf(measured)).reduce((total, row) => total + rowHeight(row), 0) - (scroll?.scrollTop ?? 0)
    return { x: 0, y: top, top, left: 0, right: 720, bottom: top + height, width: 720, height, toJSON: () => ({}) } as unknown as DOMRect
  } as typeof window.HTMLElement.prototype.getBoundingClientRect
  restores.push(() => {
    resizeObservers.clear()
    window.HTMLElement.prototype.getBoundingClientRect = originalRect
    window.HTMLElement.prototype.scrollTo = originalScrollTo
    for (const [name, descriptor] of previous) {
      if (descriptor === undefined) delete (globalThis as Record<string, unknown>)[name]
      else Object.defineProperty(globalThis, name, descriptor)
    }
  })
  const container = window.document.createElement("div") as unknown as HTMLDivElement
  window.document.body.appendChild(container as never)
  return { window, container }
}

afterEach(async () => {
  await new Promise(resolve => setTimeout(resolve, 0))
  restores.splice(0).reverse().forEach(restore => restore())
})

const historyItem = (
  sequence: number,
  kind: CodexHistoryItem["kind"],
  summary: string,
  extra: Partial<CodexHistoryItem> = {},
): CodexHistoryItem => ({
  itemRef: `item-${sequence}`,
  threadRef: "thread-1",
  sequence,
  timestamp: `2026-07-14T12:00:${String(sequence).padStart(2, "0")}.000Z`,
  kind,
  label: kind.replaceAll("_", " "),
  summary,
  status: null,
  fields: [],
  redacted: false,
  sourceType: `fixture/${kind}`,
  ...extra,
})

const record = (key: string, sequence: number): ReactTimelineRecord => ({
  key,
  itemRef: key,
  sequence,
  kind: "assistant_message",
  label: "Assistant",
  body: key,
  timestamp: "12:00",
  status: null,
  redacted: false,
  fields: [],
  resultRef: null,
  resultBody: null,
  resultStatus: null,
})

const report: IntentReporter = () => Effect.void

describe("conversation empty state", () => {
  test("uses the selected agent name and a compact icon-only directory action", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    root.render(<ConversationTimeline
      page={null}
      notes={[]}
      loadingEdge={null}
      workingDirectory="/Users/test/work"
      agentName="Claude"
      report={report}
    />)
    await new Promise(resolve => setTimeout(resolve, 0))

    expect(container.querySelector("h2")?.textContent).toBe("Start a conversation with Claude")
    const change = container.querySelector<HTMLButtonElement>('[aria-label="Change working directory"]')
    expect(change?.textContent).toBe("")
    expect(change?.querySelector('[data-icon-name="FolderPen"]')).not.toBeNull()
    root.unmount()
  })
})
const settle = (): Promise<void> => new Promise(resolve => setTimeout(resolve, 0))

describe("React typed timeline projection", () => {
  test("formats valid ISO timestamps for display and hides Unix epoch sentinels", () => {
    const valid = formatReactTimelineTimestamp("2026-07-17T11:32:00.000Z")
    expect(valid.short).toMatch(/\d{1,2}:\d{2}/u)
    expect(valid.short).not.toContain("2026-")
    expect(valid.tooltip).toContain("2026")
    expect(formatReactTimelineTimestamp("1970-01-01T00:00:00.000Z")).toEqual({ short: "", tooltip: "" })
  })

  test("merges local tool lifecycle updates into one stable command row", () => {
    const notes = [
      {
        key: "bash-start",
        role: "system" as const,
        text: 'Bash · started · {"command":"mkdir Misc"}',
        timestamp: "05:41",
      },
      {
        key: "bash-result",
        role: "system" as const,
        text: "Bash · ok · directory created",
        timestamp: "05:42",
      },
    ]
    const records = projectLocalTimelineRecords(notes)
    expect(records).toHaveLength(1)
    expect(records[0]).toMatchObject({
      key: "bash-start",
      kind: "tool_call",
      label: "Bash",
      body: "mkdir Misc",
      status: "completed",
      resultBody: "directory created",
      resultStatus: "completed",
    })
  })

  test("deduplicates and sorts typed items while preserving assistant segmentation", () => {
    const records = projectReactTimelineRecords([
      historyItem(2, "assistant_message", "after tool"),
      historyItem(0, "assistant_message", "before tool"),
      historyItem(1, "approval", "Needs confirmation"),
      historyItem(2, "assistant_message", "after tool"),
    ])
    expect(records.map(value => [value.key, value.body])).toEqual([
      ["item-0", "before tool"],
      ["item-1", "Needs confirmation"],
      ["item-2", "after tool"],
    ])
  })

  test("updates a matching tool invocation in place and renders one newest terminal", () => {
    const call = historyItem(1, "tool_call", "pnpm test", {
      itemRef: "call-item",
      label: "exec_command",
      status: "running",
      fields: [{ label: "call", value: "call-1" }],
    })
    const result = historyItem(2, "tool_result", "All tests passed", {
      itemRef: "result-item",
      status: "completed",
      fields: [{ label: "call", value: "call-1" }],
    })
    const records = projectReactTimelineRecords([
      call,
      result,
      historyItem(3, "lifecycle", "Interrupted", { status: "interrupted" }),
      historyItem(4, "lifecycle", "Completed after restart", { status: "completed" }),
    ])
    expect(records.map(value => value.key)).toEqual(["call-item", "item-4"])
    expect(records[0]).toMatchObject({ resultRef: "result-item", resultBody: "All tests passed", status: "completed" })
    expect(records.filter(value => value.kind === "lifecycle")).toHaveLength(1)
  })

  test("keeps authored loss, failure, and plan records while suppressing usage scaffolding", () => {
    const records = projectReactTimelineRecords([
      historyItem(0, "gap", "Cursor gap"),
      historyItem(1, "assistant_message", "[REDACTED: withheld]", { redacted: true }),
      historyItem(2, "usage", "Tokens", { fields: [{ label: "total", value: "42" }] }),
      historyItem(3, "error", "Turn failed", { status: "failed" }),
      historyItem(4, "plan", "Plan updated", { fields: [{ label: "1", value: "done" }] }),
    ])
    expect(records.map(value => value.kind)).toEqual(["gap", "assistant_message", "error", "plan"])
    expect(records[1]?.redacted).toBe(true)
  })

  test("suppresses internal history scaffolding and redacted reasoning placeholders", () => {
    const records = projectReactTimelineRecords([
      historyItem(0, "session", "Session metadata"),
      historyItem(1, "context", "Working directory"),
      historyItem(2, "metadata", "Provider metadata"),
      historyItem(3, "usage", "Token usage update"),
      historyItem(4, "reasoning", "[REDACTED: reasoning not persisted as summary]", { redacted: true }),
      historyItem(5, "assistant_message", "Useful answer"),
    ])
    expect(records.map(value => value.body)).toEqual(["Useful answer"])
  })

  test("uses the bounded Markdown parser and never creates an attacker-controlled link", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    root.render(<SafeReactMarkdown value={"## Safe **text** [open](javascript:alert(1))\n\n```ts\nconst ok = true\n```"} />)
    await settle()
    expect(container.querySelector("h2")?.textContent).toBe("Safe text open (javascript:alert(1))")
    expect(container.querySelector("strong")?.textContent).toBe("text")
    expect(container.querySelector("pre code")?.textContent).toBe("const ok = true")
    expect(container.querySelector("a")).toBeNull()
    expect(container.innerHTML).not.toContain("<script")
    root.unmount()
  })

  test("message rows never render the removed top metadata bar", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    root.render(<ReactTimeline sessionKey="thread-1" records={[record("answer", 0)]}
      loadedItemCount={1} offset={0} totalItems={1} loadingEdge={null} report={report} />)
    await settle()
    expect(container.querySelector(".oa-react-message-meta")).toBeNull()
    expect(container.querySelector('[data-timeline-key="answer"]')?.textContent).toContain("answer")
    root.unmount()
  })

  test("ports the T3 user bubble, right alignment, hover-only timestamp/actions, and copy control", async () => {
    const { window, container } = installDom()
    const copied: Array<string> = []
    Object.defineProperty(window.navigator, "clipboard", {
      configurable: true,
      value: { writeText: async (value: string) => { copied.push(value) } },
    })
    const root = createRoot(container)
    const user = {
      ...record("prompt", 0),
      kind: "user_message" as const,
      label: "You",
      body: "Keep the OpenAgents blue.",
      timestamp: "1970-01-01T00:00:00.000Z",
    }
    root.render(<ReactTimeline sessionKey="thread-user-bubble" records={[user]}
      loadedItemCount={1} offset={0} totalItems={1} loadingEdge={null} report={report} />)
    await settle()

    const row = container.querySelector<HTMLElement>('[data-timeline-key="prompt"]')
    const bubble = row?.querySelector<HTMLElement>('[data-slot="user-message-bubble"]')
    const actions = row?.querySelector<HTMLElement>('[data-slot="user-message-actions"]')
    expect(row?.classList.contains("oa-react-user-message-row")).toBe(true)
    expect(bubble?.classList.contains("oa-react-user-message-bubble")).toBe(true)
    expect(actions?.classList.contains("oa-react-user-message-actions")).toBe(true)
    expect(actions?.textContent).not.toContain("1970")

    const copy = row?.querySelector<HTMLButtonElement>('[aria-label="Copy message"]')
    expect(copy).not.toBeNull()
    copy?.click()
    await settle()
    expect(copied).toEqual(["Keep the OpenAgents blue."])
    expect(copy?.querySelector("svg")?.classList.contains("text-primary")).toBe(true)
    root.unmount()
  })

  test("collapses only long user messages and exposes an in-bubble keyboard toggle", async () => {
    expect(shouldCollapseUserMessage("short prompt")).toBe(false)
    expect(shouldCollapseUserMessage(Array.from({ length: 9 }, (_, index) => `line ${index}`).join("\n"))).toBe(true)
    expect(shouldCollapseUserMessage("x".repeat(601))).toBe(true)

    const { container } = installDom()
    const root = createRoot(container)
    const user = {
      ...record("long-prompt", 0),
      kind: "user_message" as const,
      label: "You",
      body: Array.from({ length: 9 }, (_, index) => `line ${index}`).join("\n"),
    }
    root.render(<ReactTimeline sessionKey="thread-long-user" records={[user]}
      loadedItemCount={1} offset={0} totalItems={1} loadingEdge={null} report={report} />)
    await settle()
    const body = container.querySelector<HTMLElement>('[data-user-message-collapsible="true"]')
    const toggle = container.querySelector<HTMLButtonElement>(".oa-react-user-message-toggle")
    expect(body?.dataset.userMessageCollapsed).toBe("true")
    expect(toggle?.textContent).toBe("Show full message")
    toggle?.click()
    await settle()
    expect(body?.dataset.userMessageCollapsed).toBe("false")
    expect(toggle?.getAttribute("aria-expanded")).toBe("true")
    expect(toggle?.textContent).toBe("Show less")
    root.unmount()
  })

  test("renders T3-style assistant copy, timestamp, and details metadata only when settled", async () => {
    const { window, container } = installDom()
    const copied: string[] = []
    Object.defineProperty(window.navigator, "clipboard", {
      configurable: true,
      value: { writeText: async (value: string) => { copied.push(value) } },
    })
    const root = createRoot(container)
    root.render(<ReactTimeline sessionKey="thread-assistant-actions" records={[{
      ...record("settled-answer", 0),
      body: "A settled answer.",
      timestamp: "2026-07-17T11:32:00.000Z",
      status: "completed",
    }]} loadedItemCount={1} offset={0} totalItems={1} loadingEdge={null} report={report} />)
    await settle()
    const actions = container.querySelector<HTMLElement>('[data-slot="assistant-message-actions"]')
    expect(actions?.textContent).toContain("Details")
    expect(actions?.textContent).toMatch(/\d{1,2}:\d{2}/u)
    actions?.querySelector<HTMLButtonElement>('[aria-label="Copy message"]')?.click()
    await settle()
    expect(copied).toEqual(["A settled answer."])

    root.render(<ReactTimeline sessionKey="thread-assistant-actions" records={[{
      ...record("settled-answer", 0), body: "Streaming…", status: "running",
    }]} loadedItemCount={1} offset={0} totalItems={1} loadingEdge={null} report={report} />)
    await settle()
    expect(container.querySelector('[aria-label="Copy message"]')).toBeNull()
    root.unmount()
  })

  test("shows assistant metadata only on the terminal assistant chunk in each user turn", async () => {
    const user = (key: string, sequence: number): ReactTimelineRecord => ({
      ...record(key, sequence), kind: "user_message", label: "You",
    })
    const records = [
      user("user-1", 0), record("commentary-1", 1), record("answer-1", 2),
      user("user-2", 3), record("commentary-2", 4), record("answer-2", 5),
    ]
    expect([...deriveAssistantMetaKeys(records)]).toEqual(["answer-1", "answer-2"])

    const { container } = installDom()
    const root = createRoot(container)
    root.render(<ReactTimeline sessionKey="thread-assistant-segments" records={records}
      loadedItemCount={records.length} offset={0} totalItems={records.length} loadingEdge={null} report={report} />)
    await settle()
    expect(container.querySelector('[data-timeline-key="commentary-1"] [data-slot="assistant-message-actions"]')).toBeNull()
    expect(container.querySelector('[data-timeline-key="answer-1"] [data-slot="assistant-message-actions"]')).not.toBeNull()
    expect(container.querySelector('[data-timeline-key="commentary-2"] [data-slot="assistant-message-actions"]')).toBeNull()
    expect(container.querySelector('[data-timeline-key="answer-2"] [data-slot="assistant-message-actions"]')).not.toBeNull()
    root.unmount()
  })

  test("composes the shadcn scroller accessibility and stable turn-anchor contract", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    const user = { ...record("prompt", 0), kind: "user_message" as const, label: "You" }
    root.render(<ReactTimeline sessionKey="thread-1" records={[user, record("answer", 1)]}
      loadedItemCount={2} offset={0} totalItems={2} loadingEdge={null} working report={report} />)
    await settle()
    const viewport = container.querySelector('[data-slot="message-scroller-viewport"]')
    const content = container.querySelector('[data-slot="message-scroller-content"]')
    const prompt = container.querySelector('[data-message-id="prompt"]')
    expect(viewport?.getAttribute("role")).toBe("region")
    expect(viewport?.getAttribute("tabindex")).toBe("0")
    expect(content?.getAttribute("role")).toBe("log")
    expect(content?.getAttribute("aria-relevant")).toBe("additions")
    expect(content?.getAttribute("aria-busy")).toBe("true")
    expect(content?.classList.contains("w-full")).toBe(true)
    expect(content?.classList.contains("min-w-0")).toBe(true)
    expect(prompt?.getAttribute("data-scroll-anchor")).toBe("true")
    expect(prompt?.classList.contains("w-full")).toBe(true)
    expect(prompt?.classList.contains("min-w-0")).toBe(true)
    expect(container.querySelector('[data-icon-name="ArrowDown"]')).not.toBeNull()
    expect(container.querySelector('[aria-label="Jump to latest"]')).not.toBeNull()
    root.unmount()
  })

  test("derives one navigable minimap stop per user turn with assistant context", async () => {
    const firstUser = { ...record("prompt-1", 0), kind: "user_message" as const, label: "You", body: "First prompt" }
    const secondUser = { ...record("prompt-2", 2), kind: "user_message" as const, label: "You", body: "Second prompt" }
    expect(deriveReactTimelineTurns([
      firstUser, { ...record("answer-1", 1), body: "First answer" }, secondUser, { ...record("answer-2", 3), body: "Second answer" },
    ])).toEqual([
      { id: "prompt-1", userPreview: "First prompt", assistantPreview: "First answer" },
      { id: "prompt-2", userPreview: "Second prompt", assistantPreview: "Second answer" },
    ])

    const { container } = installDom()
    const root = createRoot(container)
    root.render(<ReactTimeline sessionKey="thread-minimap" records={[firstUser, record("answer-1", 1), secondUser, record("answer-2", 3)]}
      loadedItemCount={4} offset={0} totalItems={4} loadingEdge={null} report={report} />)
    await settle()
    const buttons = container.querySelectorAll<HTMLButtonElement>('.oa-react-timeline-minimap button')
    expect(buttons).toHaveLength(2)
    expect(buttons[0]?.getAttribute("aria-label")).toContain("First prompt")
    const target = container.querySelector<HTMLElement>('[data-message-id="prompt-2"]')
    let selected = false
    if (target !== null) target.scrollIntoView = () => { selected = true }
    buttons[1]?.click()
    expect(selected).toBe(true)
    root.unmount()
  })
})

describe("React timeline scroll contract", () => {
  test("keeps completed reasoning in the primary trace while settled tools remain foldable", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    const reasoning: ReactTimelineRecord = {
      ...record("reasoning", 0),
      kind: "reasoning",
      label: "Reasoning",
      body: "Checked the cache and found the session token had expired.",
      status: "completed",
      item: {
        kind: "reasoning",
        source: "codex",
        summary: "Checked the cache and found the session token had expired.",
        status: "completed",
      },
    }
    const work = (key: string, sequence: number): ReactTimelineRecord => ({
      ...record(key, sequence), kind: "tool_call", label: "Run command", status: "completed",
    })
    root.render(<ReactTimeline sessionKey="thread-reasoning-primary" records={[
      reasoning, work("done-a", 1), work("done-b", 2),
    ]} loadedItemCount={3} offset={0} totalItems={3} loadingEdge={null} report={report} />)
    await settle()

    const reasoningRow = container.querySelector<HTMLElement>('[data-message-id="reasoning"]')
    expect(reasoningRow?.textContent).toContain("Checked the cache")
    expect(reasoningRow?.querySelector("summary")).toBeNull()
    expect(container.querySelector('.oa-react-work-group-summary')?.textContent).toBe("Worked2 activities")
    expect(container.querySelector('[data-timeline-key="done-a"]')).toBeNull()
    root.unmount()
  })

  test("folds settled work, exposes active work, and shows streaming state without accounting noise", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    const work = (key: string, sequence: number, status: string): ReactTimelineRecord => ({
      ...record(key, sequence), kind: "tool_call", label: "Run command", status, resultBody: `${key} result`,
    })
    root.render(<ReactTimeline sessionKey="thread-1" records={[
      work("done-a", 0, "completed"), work("done-b", 1, "completed"), record("answer", 2),
      work("prior", 3, "completed"), work("active", 4, "running"),
    ]} loadedItemCount={5} offset={0} totalItems={5} loadingEdge={null} working report={report} />)
    await settle()
    const summaries = [...container.querySelectorAll(".oa-react-work-group-summary")].map(node => node.textContent)
    expect(summaries).toEqual(["Worked2 activities", "+1 previous1 activity"])
    const settledToggle = container.querySelector<HTMLButtonElement>('.oa-react-work-group-summary')
    const settledGroup = settledToggle?.closest<HTMLElement>(".oa-react-work-group") ?? null
    const viewport = container.querySelector<HTMLElement>('[data-slot="message-scroller-viewport"]')
    if (settledGroup !== null && viewport !== null) {
      Object.defineProperty(viewport, "clientHeight", { configurable: true, value: 100 })
      Object.defineProperty(viewport, "scrollHeight", { configurable: true, value: 400 })
      viewport.scrollTop = 50
      settledGroup.getBoundingClientRect = () => {
        const expanded = settledToggle?.getAttribute("aria-expanded") === "true"
        return { top: 0, bottom: expanded ? 180 : 100, height: expanded ? 180 : 100 } as DOMRect
      }
    }
    expect(settledToggle?.getAttribute("aria-expanded")).toBe("false")
    expect(container.querySelector('[data-timeline-key="done-a"]')).toBeNull()
    settledToggle?.click()
    await settle()
    expect(settledToggle?.getAttribute("aria-expanded")).toBe("true")
    expect(viewport?.scrollTop).toBe(130)
    expect(container.querySelector('[data-timeline-key="done-a"]')).not.toBeNull()
    expect(container.querySelector('[data-icon-name="ChevronRight"]')?.getAttribute("data-expanded")).toBe("true")
    settledToggle?.click()
    await settle()
    expect(settledToggle?.getAttribute("aria-expanded")).toBe("false")
    expect(container.querySelector('[data-timeline-key="done-a"]')).toBeNull()
    expect(container.querySelector('[data-timeline-key="active"]')?.textContent).toContain("Running")
    expect(container.querySelector('.oa-react-working[aria-label="Codex is working"]')).not.toBeNull()
    expect(container.textContent).not.toContain("Token usage update")
    root.unmount()
  })

  test("replaces generic working copy with an explicit waiting-for-answer status", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    root.render(<ReactTimeline sessionKey="thread-question" records={[record("prompt", 0)]}
      loadedItemCount={1} offset={0} totalItems={1} loadingEdge={null}
      waitingForAnswer report={report} />)
    await settle()
    expect(container.querySelector('[aria-label="Waiting for your answer"]')?.textContent).toBe("Waiting for your answer")
    expect(container.querySelector(".oa-react-working")).toBeNull()
    expect(container.querySelector('[data-slot="message-scroller-content"]')?.getAttribute("aria-busy")).toBe("false")
    root.unmount()
  })

  test("preserves the first visible variable-height row synchronously on prepend", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    root.render(<ReactTimeline sessionKey="thread-1" records={[record("b", 1), record("c", 2)]} loadedItemCount={2} offset={1} totalItems={3} loadingEdge={null} report={report} />)
    await settle()
    const scroll = container.querySelector(".oa-react-timeline-scroll") as HTMLDivElement
    Object.defineProperty(scroll, "clientHeight", { configurable: true, value: 100 })
    Object.defineProperty(scroll, "scrollHeight", { configurable: true, get: () => [...scroll.querySelectorAll<HTMLElement>("[data-timeline-key]")].reduce((sum, row) => sum + (row.dataset.timelineKey === "a" ? 60 : row.dataset.timelineKey === "c" ? 80 : 40), 0) })
    scroll.scrollTop = 20
    root.render(<ReactTimeline sessionKey="thread-1" records={[record("a", 0), record("b", 1), record("c", 2)]} loadedItemCount={3} offset={0} totalItems={3} loadingEdge={null} report={report} />)
    await settle()
    await flushResizeObservers()
    expect(scroll.scrollTop).toBe(80)
    root.unmount()
  })

  test("holds manual reader position on append, offers new activity, and follows at the live edge", async () => {
    const { window, container } = installDom()
    const focusSentinel = window.document.createElement("button")
    focusSentinel.textContent = "Keep focus"
    window.document.body.appendChild(focusSentinel)
    focusSentinel.focus()
    const root = createRoot(container)
    const render = (records: ReadonlyArray<ReactTimelineRecord>) => root.render(
      <ReactTimeline sessionKey="thread-1" records={records} loadedItemCount={records.length} offset={0} totalItems={records.length} loadingEdge={null} report={report} />,
    )
    render([record("a", 0), record("b", 1), record("c", 2), record("d", 3)])
    await settle()
    const scroll = container.querySelector(".oa-react-timeline-scroll") as HTMLDivElement
    Object.defineProperty(scroll, "clientHeight", { configurable: true, value: 100 })
    Object.defineProperty(scroll, "scrollHeight", { configurable: true, get: () => scroll.querySelectorAll("[data-timeline-key]").length * 40 })
    scroll.scrollTop = 20
    scroll.dispatchEvent(new window.Event("wheel", { bubbles: true }) as unknown as Event)
    scroll.dispatchEvent(new window.Event("scroll", { bubbles: true }) as unknown as Event)
    await settle()
    render([record("a", 0), record("b", 1), record("c", 2), record("d", 3), record("e", 4)])
    await settle()
    expect(scroll.scrollTop).toBe(20)
    expect(window.document.activeElement).toBe(focusSentinel)
    expect(container.querySelector(".oa-react-new-activity")?.textContent).toContain("Jump to latest")
    ;(container.querySelector(".oa-react-new-activity") as HTMLButtonElement).click()
    await settle()
    expect(scroll.scrollTop).toBe(scroll.scrollHeight - scroll.clientHeight)
    render([record("a", 0), record("b", 1), record("c", 2), record("d", 3), record("e", 4), record("f", 5)])
    await settle()
    await flushResizeObservers()
    expect(scroll.scrollTop).toBe(scroll.scrollHeight - scroll.clientHeight)
    expect(container.querySelector(".oa-react-new-activity")?.getAttribute("data-active")).toBe("false")
    root.unmount()
  })

  test("requests typed older and newer pages from the measured scroll edges", async () => {
    const { window, container } = installDom()
    const received: Array<string> = []
    const pagingReport: IntentReporter = ref => Effect.sync(() => { received.push(ref.name) })
    const root = createRoot(container)
    const rows = [record("a", 0), record("b", 1)]
    root.render(<ReactTimeline sessionKey="thread-1" records={rows} loadedItemCount={2} offset={2} totalItems={10} loadingEdge={null} report={pagingReport} />)
    await settle()
    const scroll = container.querySelector(".oa-react-timeline-scroll") as HTMLDivElement
    Object.defineProperty(scroll, "clientHeight", { configurable: true, value: 100 })
    Object.defineProperty(scroll, "scrollHeight", { configurable: true, value: 500 })
    scroll.scrollTop = 0
    scroll.dispatchEvent(new Event("scroll", { bubbles: true }))
    await settle()
    expect(received).toEqual(["HistoryOlderRequested"])
    root.unmount()

    const second = window.document.createElement("div") as unknown as HTMLDivElement
    window.document.body.appendChild(second as never)
    const newerRoot = createRoot(second)
    newerRoot.render(<ReactTimeline sessionKey="thread-1" records={rows} loadedItemCount={2} offset={0} totalItems={10} loadingEdge={null} report={pagingReport} />)
    await settle()
    const newerScroll = second.querySelector(".oa-react-timeline-scroll") as HTMLDivElement
    Object.defineProperty(newerScroll, "clientHeight", { configurable: true, value: 100 })
    Object.defineProperty(newerScroll, "scrollHeight", { configurable: true, value: 500 })
    newerScroll.scrollTop = 450
    newerScroll.dispatchEvent(new Event("scroll", { bubbles: true }))
    await settle()
    expect(received).toEqual(["HistoryOlderRequested", "HistoryNewerRequested"])
    newerRoot.unmount()
  })

  test("session replacement cannot retain stale prior content", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    root.render(<ReactTimeline sessionKey="old" records={[record("old-item", 0)]} loadedItemCount={1} offset={0} totalItems={1} loadingEdge={null} report={report} />)
    await settle()
    root.render(<ReactTimeline sessionKey="new" records={[record("new-item", 0)]} loadedItemCount={1} offset={0} totalItems={1} loadingEdge={null} report={report} />)
    await settle()
    expect(container.textContent).not.toContain("old-item")
    expect(container.textContent).toContain("new-item")
    root.unmount()
  })

  test("same-key streaming updates replace in place and remain pinned only at the live edge", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    const streaming = (body: string): ReactTimelineRecord => ({ ...record("stream", 0), body })
    root.render(<ReactTimeline sessionKey="thread-1" records={[streaming("hel")]} loadedItemCount={1} offset={0} totalItems={1} loadingEdge={null} report={report} />)
    await settle()
    const scroll = container.querySelector(".oa-react-timeline-scroll") as HTMLDivElement
    Object.defineProperty(scroll, "clientHeight", { configurable: true, value: 100 })
    Object.defineProperty(scroll, "scrollHeight", {
      configurable: true,
      get: () => scroll.textContent?.includes("hello") ? 150 : 120,
    })
    scroll.scrollTop = 20
    const before = scroll.querySelector('[data-timeline-key="stream"]')
    root.render(<ReactTimeline sessionKey="thread-1" records={[streaming("hello")]} loadedItemCount={1} offset={0} totalItems={1} loadingEdge={null} report={report} />)
    await settle()
    await flushResizeObservers()
    const rows = scroll.querySelectorAll('[data-timeline-key="stream"]')
    expect(rows).toHaveLength(1)
    expect(rows[0]).toBe(before)
    expect(rows[0]?.textContent).toContain("hello")
    expect(scroll.scrollTop).toBe(scroll.scrollHeight - scroll.clientHeight)
    root.unmount()
  })
})

describe("React timeline performance corpus", () => {
  test("projects the maximum 500-item history page within the bounded unit budget", () => {
    const corpus = Array.from({ length: 500 }, (_, index) => historyItem(index, index % 9 === 0 ? "tool_call" : "assistant_message", `item ${index}`))
    const started = performance.now()
    const records = projectReactTimelineRecords(corpus)
    const elapsed = performance.now() - started
    expect(records).toHaveLength(500)
    expect(elapsed).toBeLessThan(50)
  })

  test("renders and stream-updates the 500-item bound, then tears every row down", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    const corpus = Array.from({ length: 500 }, (_, index) => record(`perf-${index}`, index))
    const started = performance.now()
    root.render(<ReactTimeline sessionKey="perf" records={corpus} loadedItemCount={500} offset={0} totalItems={500} loadingEdge={null} report={report} />)
    await settle()
    expect(container.querySelectorAll("[data-timeline-key]")).toHaveLength(500)
    root.render(<ReactTimeline sessionKey="perf" records={[
      ...corpus.slice(0, -1),
      { ...corpus.at(-1)!, body: "streamed terminal update" },
    ]} loadedItemCount={500} offset={0} totalItems={500} loadingEdge={null} report={report} />)
    await settle()
    expect(container.querySelectorAll("[data-timeline-key]")).toHaveLength(500)
    expect(performance.now() - started).toBeLessThan(2_000)
    root.unmount()
    expect(container.querySelectorAll("[data-timeline-key]")).toHaveLength(0)
  })
})

describe("typed WorkbenchItem surfacing on timeline records (#8859)", () => {
  const commandItem = {
    kind: "command",
    source: "codex",
    command: "pnpm test",
    status: "completed",
    exitCode: 0,
    durationMs: 950,
    outputTail: "42 passed",
  } as const

  test("local records carry the typed item from the trace note", () => {
    const records = projectLocalTimelineRecords([
      {
        key: "bash-start",
        role: "system" as const,
        text: "Bash · started",
        timestamp: "05:41",
        meta: { trace: { toolName: "Bash", phase: "started" as const, summary: '{"command":"pnpm test"}' } },
      },
      {
        key: "bash-result",
        role: "system" as const,
        text: "Bash · ok · 42 passed",
        timestamp: "05:42",
        meta: { trace: { toolName: "Bash", phase: "ok" as const, summary: "42 passed", item: commandItem } },
      },
    ])
    expect(records).toHaveLength(1)
    expect(records[0]!.item).toEqual(commandItem)
    // Existing string presentation is unchanged.
    expect(records[0]).toMatchObject({ kind: "tool_call", label: "Bash", status: "completed" })
  })

  test("history records surface the typed sidecar; the result row's item wins", () => {
    const startedItem = { ...commandItem, status: "in_progress" as const }
    const records = projectReactTimelineRecords([
      historyItem(1, "tool_call", "pnpm test", {
        itemRef: "call-item",
        label: "exec_command",
        status: "running",
        fields: [{ label: "call", value: "call-9" }],
        item: startedItem,
      }),
      historyItem(2, "tool_result", "All tests passed", {
        itemRef: "result-item",
        status: "completed",
        fields: [{ label: "call", value: "call-9" }],
        item: commandItem,
      }),
    ])
    expect(records).toHaveLength(1)
    expect(records[0]!.item).toEqual(commandItem)
  })

  test("renders the typed command card with lifecycle metrics and cap honesty", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    root.render(<ReactTimeline
      sessionKey="thread-command"
      records={[{
        ...record("command-row", 0),
        kind: "tool_call",
        label: "Bash",
        status: "completed",
        item: {
          ...commandItem,
          cwd: "/safe/repo",
          commandSource: "agent",
          outputCapReached: true,
        },
      }]}
      loadedItemCount={1}
      offset={0}
      totalItems={1}
      loadingEdge={null}
      report={report}
    />)
    await settle()
    const card = container.querySelector<HTMLElement>('[data-kind="commandExecution"]')
    expect(card?.dataset.status).toBe("completed")
    expect(card?.textContent).toContain("pnpm test")
    expect(card?.textContent).toContain("EXIT: 0")
    expect(card?.textContent).toContain("950MS")
    expect(card?.textContent).toContain("/safe/repo")
    expect(card?.textContent).toContain("Earlier output omitted")
    root.unmount()
  })

  test("renders typed file changes with patch status, tallies, and expandable diff lines", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    root.render(<ReactTimeline
      sessionKey="thread-files"
      records={[{
        ...record("file-row", 0),
        kind: "tool_call",
        label: "FileChange",
        status: "running",
        item: {
          kind: "fileChange",
          source: "codex",
          status: "in_progress",
          scope: "turn",
          changes: [{
            path: "src/a.ts",
            kind: "update",
            adds: 1,
            dels: 1,
            diff: "@@ -1 +1 @@\n-old\n+new",
            diffCapReached: true,
          }],
        },
      }]}
      loadedItemCount={1}
      offset={0}
      totalItems={1}
      loadingEdge={null}
      report={report}
    />)
    await settle()
    const card = container.querySelector<HTMLElement>('[data-kind="fileChange"]')
    expect(card?.dataset.status).toBe("running")
    expect(card?.textContent).toContain("Turn diff")
    expect(card?.textContent).toContain("PATCH: RUNNING")
    expect(card?.textContent).toContain("[MOD]")
    expect(card?.textContent).toContain("+1−1")
    expect(card?.querySelector('[data-diff-line="remove"]')?.textContent).toBe("-old")
    expect(card?.querySelector('[data-diff-line="add"]')?.textContent).toBe("+new")
    expect(card?.textContent).toContain("Diff truncated")
  })
})

describe("plan unification across all three sources (T8 #8865)", () => {
  test("a live plan_updated note projects via note.runtime, NOT the broken text-prefix regex", () => {
    const records = projectLocalTimelineRecords([
      {
        key: "turn-1-plan",
        role: "system" as const,
        // The real note text every plan note carries is the literal string
        // below — it never matched the old `/^Plan\s*·/` prefix check, so
        // this card silently degraded to a generic system-message notice
        // before T8.
        text: "Plan updated",
        timestamp: "10:00",
        runtime: {
          kind: "plan" as const,
          entries: [{ step: "Reproduce the bug", status: "completed" as const }, { step: "Ship the fix", status: "in_progress" as const }],
          prose: "Investigate, then land the fix behind a flag.",
        },
      },
    ])
    expect(records).toHaveLength(1)
    expect(records[0]).toMatchObject({ kind: "plan", label: "Plan", body: "Investigate, then land the fix behind a flag." })
    expect(records[0]!.item).toEqual({
      kind: "plan",
      source: "local",
      entries: [{ step: "Reproduce the bug", status: "completed" }, { step: "Ship the fix", status: "in_progress" }],
      prose: "Investigate, then land the fix behind a flag.",
    })
  })

  test("a prose-only plan note (the previously-dropped `plan` ThreadItem path) still projects a typed item", () => {
    const records = projectLocalTimelineRecords([
      {
        key: "turn-2-plan",
        role: "system" as const,
        text: "Plan updated",
        timestamp: "10:01",
        runtime: { kind: "plan" as const, entries: [], prose: "Collaboration-mode write-up." },
      },
    ])
    expect(records[0]!.item).toEqual({ kind: "plan", source: "local", entries: [], prose: "Collaboration-mode write-up." })
    expect(records[0]!.body).toBe("Collaboration-mode write-up.")
  })

  test("a history plan/todo_list row's typed item dispatches through the SAME DesktopPlanCard as a live plan", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    const historyPlanItem = { kind: "plan" as const, source: "codex" as const, entries: [{ step: "Audit the queue", status: "pending" as const }] }
    root.render(<ReactTimeline
      sessionKey="thread-history-plan"
      records={[{ ...record("history-plan-row", 0), kind: "plan", label: "Plan", item: historyPlanItem }]}
      loadedItemCount={1} offset={0} totalItems={1} loadingEdge={null} report={report}
    />)
    await settle()
    const card = container.querySelector<HTMLElement>('[data-kind="plan"]')
    expect(card).not.toBeNull()
    expect(card?.textContent).toContain("Audit the queue")
    root.unmount()
  })

  test("in-place plan updates never remount the card: same DOM node, same React key across entry changes", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    const planRecord = (entries: ReadonlyArray<{ step: string; status: "pending" | "in_progress" | "completed" }>): ReactTimelineRecord => ({
      ...record("turn-1-plan", 0),
      kind: "plan",
      label: "Plan",
      item: { kind: "plan" as const, source: "local" as const, entries },
    })
    root.render(<ReactTimeline sessionKey="thread-plan" records={[planRecord([{ step: "a", status: "in_progress" }])]} loadedItemCount={1} offset={0} totalItems={1} loadingEdge={null} report={report} />)
    await settle()
    const before = container.querySelector('[data-timeline-key="turn-1-plan"]')
    expect(before?.textContent).toContain("0 of 1 done")
    root.render(<ReactTimeline sessionKey="thread-plan" records={[planRecord([{ step: "a", status: "completed" }, { step: "b", status: "in_progress" }])]} loadedItemCount={1} offset={0} totalItems={1} loadingEdge={null} report={report} />)
    await settle()
    const rows = container.querySelectorAll('[data-timeline-key="turn-1-plan"]')
    expect(rows).toHaveLength(1) // in place, not appended/duplicated
    expect(rows[0]).toBe(before) // the SAME DOM node — never remounted
    expect(rows[0]?.textContent).toContain("1 of 2 done")
    root.unmount()
  })
})

// ---------------------------------------------------------------------------
// Delegated-agent collab states + subagent activity (#8867 T10, epic #8857
// Wave 2). Today's live `child_started`/`child_activity`/`child_completed`/
// `child_failed` FableLocalEvents (codex-app-server-turn.ts's
// `collabAgentToolCall`/`subAgentActivity` handling) project as a
// `runtime: {kind:"child"}` note (local-harness.ts); before this change that
// note fell through to a flat system-notice line on the PRIMARY React
// timeline (only the `?renderer=compatibility` fallback in shell.ts +
// runtime-cards.ts rendered it as a real card with Interrupt). These tests
// prove the primary path now shows the same delegated-agent status through
// `DesktopAgentGroup`, and that the Interrupt affordance transfers to the new
// call site using the SAME `childInterruptable` predicate and the SAME
// `DesktopChildInterruptRequested` intent the compatibility renderer
// dispatches (runtime-cards.test.ts, untouched, still proves that renderer
// unregressed).
// ---------------------------------------------------------------------------
describe("delegated-agent collab states on the primary React timeline (#8867)", () => {
  const childNote = (overrides: Partial<{
    status: "running" | "completed" | "failed"
    steered: null | { action: "message" | "interrupt"; outcome: "interrupted" | "delivered" | "unsupported" | "not_found"; detail: string }
    title: string
    detail: string
  }> = {}) => ({
    key: "turn-1-child-child-9",
    role: "system" as const,
    text: "Delegate child · summarizing",
    timestamp: "05:41",
    runtime: {
      kind: "child" as const,
      turnRef: "turn-1",
      childRef: "child-9",
      status: "running" as const,
      title: "Summarize the task",
      detail: "reading files",
      steered: null,
      ...overrides,
    },
  })

  test("a live delegate-child note projects as a typed agent item, not flattened notice text", () => {
    const records = projectLocalTimelineRecords([childNote()])
    expect(records).toHaveLength(1)
    expect(records[0]!.item).toMatchObject({
      kind: "agent",
      source: "codex",
      status: "in_progress",
      children: [{ threadRef: "child-9", status: "running", nickname: "Summarize the task" }],
    })
    expect(records[0]!.runtimeChild).toEqual({ turnRef: "turn-1", childRef: "child-9", interruptable: true })
  })

  test("a completed child is no longer interruptable and maps to the completed bucket", () => {
    const records = projectLocalTimelineRecords([childNote({ status: "completed" })])
    expect(records[0]!.item).toMatchObject({ status: "completed", children: [{ status: "completed" }] })
    expect(records[0]!.runtimeChild?.interruptable).toBe(false)
  })

  test("a failed child maps to the errored child bucket and the failed item status", () => {
    const records = projectLocalTimelineRecords([childNote({ status: "failed" })])
    expect(records[0]!.item).toMatchObject({ status: "failed", children: [{ status: "errored" }] })
    expect(records[0]!.runtimeChild?.interruptable).toBe(false)
  })

  test("an already-steered running child is no longer interruptable (childInterruptable reused verbatim)", () => {
    const records = projectLocalTimelineRecords([childNote({
      steered: { action: "interrupt", outcome: "interrupted", detail: "child interrupt requested" },
    })])
    expect(records[0]!.runtimeChild?.interruptable).toBe(false)
  })

  test("queue runtime notes are UNCHANGED (still fall through to generic note text); plan is T8's, not mine", () => {
    // `plan` runtime notes get their OWN typed-item branch from T8 (#8865,
    // unrelated to this change) — only asserting `queue` here to avoid
    // claiming credit for that lane's fix.
    const queueRecords = projectLocalTimelineRecords([{
      key: "turn-1-queue",
      role: "system" as const,
      text: "Queued follow-up (#1)",
      timestamp: "05:41",
      runtime: { kind: "queue" as const, turnRef: "turn-1", queueRef: "q1", position: 1 },
    }])
    expect(queueRecords).toHaveLength(1)
    expect(queueRecords[0]!.item).toBeUndefined()
  })

  test("renders a running delegate child through DesktopAgentGroup with an Interrupt control", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    const records = projectLocalTimelineRecords([childNote()])
    root.render(<ReactTimeline sessionKey="thread-1" records={records}
      loadedItemCount={1} offset={0} totalItems={1} loadingEdge={null} report={report} />)
    await settle()
    const card = container.querySelector<HTMLElement>('[data-kind="collabAgentToolCall"]')
    expect(card).not.toBeNull()
    expect(card?.textContent).toContain("Summarize the task")
    // Exact wire-status label (design spec badge formula), not the coarse "Running" bucket text.
    expect(card?.textContent).toContain("RUNNING")
    const interruptButton = card?.querySelector<HTMLButtonElement>(".oa-react-agent-interrupt")
    expect(interruptButton).not.toBeNull()
    expect(interruptButton?.getAttribute("aria-label")).toBe("Interrupt Summarize the task")
    root.unmount()
  })

  test("clicking Interrupt dispatches DesktopChildInterruptRequested with the exact turnRef/childRef", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    const received: Array<unknown> = []
    const capturingReport: IntentReporter = (ref, payload) =>
      Effect.sync(() => received.push(resolveIntentRef(ref, payload)))
    const records = projectLocalTimelineRecords([childNote()])
    root.render(<ReactTimeline sessionKey="thread-1" records={records}
      loadedItemCount={1} offset={0} totalItems={1} loadingEdge={null} report={capturingReport} />)
    await settle()
    const interruptButton = container.querySelector<HTMLButtonElement>(".oa-react-agent-interrupt")
    interruptButton?.click()
    await settle()
    expect(received).toContainEqual({ name: "DesktopChildInterruptRequested", payload: { turnRef: "turn-1", childRef: "child-9" } })
    root.unmount()
  })

  test("a completed delegate child renders WITHOUT an Interrupt control", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    const records = projectLocalTimelineRecords([childNote({ status: "completed" })])
    root.render(<ReactTimeline sessionKey="thread-1" records={records}
      loadedItemCount={1} offset={0} totalItems={1} loadingEdge={null} report={report} />)
    await settle()
    const card = container.querySelector<HTMLElement>('[data-kind="collabAgentToolCall"]')
    expect(card?.querySelector(".oa-react-agent-interrupt")).toBeNull()
    expect(card?.textContent).toContain("COMPLETED")
    root.unmount()
  })

  test("history collaboration rows (codex-history.ts) surface the raw operation/activity fields as tags", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    const historyRecord: ReactTimelineRecord = {
      ...record("collab-1", 0),
      kind: "collaboration",
      label: "spawn agent",
      body: "Delegated implementation work",
      status: "running",
      fields: [
        { label: "agent", value: "child-thread-7" },
        { label: "operation", value: "spawn_agent" },
        { label: "activity", value: "started" },
      ],
    }
    root.render(<ReactTimeline sessionKey="thread-1" records={[historyRecord]}
      loadedItemCount={1} offset={0} totalItems={1} loadingEdge={null} report={report} />)
    await settle()
    const card = container.querySelector<HTMLElement>('[data-kind="collabAgentToolCall"]')
    expect(card?.querySelector('[data-operation="spawn"]')).not.toBeNull()
    expect(card?.querySelector('[data-activity="started"]')).not.toBeNull()
    root.unmount()
  })

  test("multiple collabAgentToolCall children render as separate rows in one group", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    const multiAgentRecord: ReactTimelineRecord = {
      ...record("collab-multi", 0),
      kind: "collaboration",
      label: "Delegated agents",
      body: "",
      status: null,
      fields: [],
      item: {
        kind: "agent",
        source: "codex",
        tool: "spawnAgent",
        prompt: "Implement the reviewer pass",
        status: "in_progress",
        children: [
          { threadRef: "child-a", status: "running", nickname: "protocol-scout" },
          { threadRef: "child-b", status: "completed", nickname: "timeline-builder" },
        ],
      },
    }
    root.render(<ReactTimeline sessionKey="thread-1" records={[multiAgentRecord]}
      loadedItemCount={1} offset={0} totalItems={1} loadingEdge={null} report={report} />)
    await settle()
    const rows = container.querySelectorAll(".oa-react-agent-card")
    expect(rows).toHaveLength(2)
    expect(container.querySelector('[data-operation="spawn"]')).not.toBeNull()
    expect(container.textContent).toContain("Implement the reviewer pass")
    expect(container.textContent).toContain("protocol-scout")
    expect(container.textContent).toContain("timeline-builder")
    root.unmount()
  })

  test("renders typed subAgentActivity with its friendly path instead of the wire discriminant", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    const activityRecord: ReactTimelineRecord = {
      ...record("subagent-activity", 0),
      kind: "collaboration",
      label: "subAgentActivity",
      body: "[subAgentActivity]",
      status: null,
      fields: [],
      item: {
        kind: "agent",
        source: "codex",
        status: "in_progress",
        activityKind: "interacted",
        agentPath: "reviewer",
        children: [{ threadRef: "child-thread-1", status: "running" }],
      },
    }
    root.render(<ReactTimeline sessionKey="thread-1" records={[activityRecord]}
      loadedItemCount={1} offset={0} totalItems={1} loadingEdge={null} report={report} />)
    await settle()
    expect(container.querySelector('[data-kind="collabAgentToolCall"]')).not.toBeNull()
    expect(container.textContent).toContain("reviewer")
    expect(container.textContent).toContain("interacted")
    expect(container.textContent).not.toContain("[subAgentActivity]")
    root.unmount()
  })
})

describe("streaming reasoning disclosure surfacing on timeline records (#8863 T6)", () => {
  const streamingItem = {
    kind: "reasoning",
    source: "codex",
    summary: "**Checking** the cache\n\n- session token\n- expiry",
    status: "in_progress",
  } as const
  const completedItem = {
    kind: "reasoning",
    source: "codex",
    summary: "Checked the cache and it was stale.",
    status: "completed",
  } as const

  test("local records merge the started/progress/completed reasoning trace into one typed item", () => {
    const records = projectLocalTimelineRecords([
      {
        key: "reasoning-start",
        role: "system" as const,
        text: "Reasoning · started",
        timestamp: "05:41",
        meta: { trace: { toolName: "Reasoning", phase: "started" as const, summary: "", itemRef: "item-r1", item: streamingItem } },
      },
      {
        key: "reasoning-progress",
        role: "system" as const,
        text: "Reasoning · running",
        timestamp: "05:41",
        meta: {
          trace: {
            toolName: "Reasoning", phase: "progress" as const, summary: "", itemRef: "item-r1",
            item: { ...streamingItem, summary: "Checking the cache and the token expiry" },
          },
        },
      },
      {
        key: "reasoning-result",
        role: "system" as const,
        text: "Reasoning · ok",
        timestamp: "05:42",
        meta: { trace: { toolName: "Reasoning", phase: "ok" as const, summary: "", itemRef: "item-r1", item: completedItem } },
      },
    ])
    // One card, not three — the FIFO/itemRef pairing already built for every
    // other typed tool card merges started/progress/completed by itemRef.
    expect(records).toHaveLength(1)
    expect(records[0]!.item).toEqual(completedItem)
  })

  test("adapts a persisted pre-typed reasoning note into the unlabelled Markdown presentation", async () => {
    const records = projectLocalTimelineRecords([{
      key: "legacy-reasoning",
      role: "system",
      text: "Reasoning · **Checking** the cache\n\n- token\n- expiry",
      timestamp: "05:41",
    }])
    expect(records).toHaveLength(1)
    expect(records[0]!.item).toMatchObject({
      kind: "reasoning",
      source: "local",
      summary: "**Checking** the cache\n\n- token\n- expiry",
      status: "completed",
    })

    const { container } = installDom()
    const root = createRoot(container)
    root.render(<ReactTimeline sessionKey="legacy-reasoning" records={records}
      loadedItemCount={1} offset={0} totalItems={1} loadingEdge={null} report={report} />)
    await settle()
    const card = container.querySelector<HTMLElement>(".oa-react-reasoning-disclosure")
    expect(card?.querySelector("strong")?.textContent).toBe("Checking")
    expect(card?.querySelectorAll("li")).toHaveLength(2)
    expect(card?.textContent).not.toContain("Reasoning")
    expect(card?.querySelector("summary")).toBeNull()
    root.unmount()
  })

  test("renders in-progress reasoning as only its safe Markdown body", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    // A lone work-kind record renders directly (no work-group fold); the
    // record's own `status` mirrors what the real note pipeline sets while a
    // tool_use/tool_progress trace is still open ("running"), which is also
    // what the timeline's own work-group heuristic keys off to avoid folding
    // an actively-streaming item away.
    root.render(<ReactTimeline
      sessionKey="thread-reasoning-streaming"
      records={[{ ...record("reasoning-streaming", 0), kind: "reasoning", status: "running", item: streamingItem }]}
      loadedItemCount={1}
      offset={0}
      totalItems={1}
      loadingEdge={null}
      report={report}
    />)
    await settle()
    const card = container.querySelector<HTMLElement>(".oa-react-reasoning-disclosure")
    expect(card?.dataset.status).toBe("running")
    expect(card?.textContent).toContain("Checking the cache")
    expect(card?.querySelector("strong")?.textContent).toBe("Checking")
    expect(card?.querySelectorAll("li")).toHaveLength(2)
    expect(card?.querySelector("summary")).toBeNull()
    expect(card?.textContent).not.toContain("Reasoning")
    root.unmount()
  })

  test("renders completed reasoning as the same unlabelled Markdown body", async () => {
    const { container } = installDom()
    const root = createRoot(container)
    root.render(<ReactTimeline
      sessionKey="thread-reasoning-completed"
      records={[{ ...record("reasoning-completed", 0), kind: "reasoning", status: "completed", item: completedItem }]}
      loadedItemCount={1}
      offset={0}
      totalItems={1}
      loadingEdge={null}
      report={report}
    />)
    await settle()
    const card = container.querySelector<HTMLElement>(".oa-react-reasoning-disclosure")
    expect(card?.dataset.status).toBe("completed")
    expect(card?.textContent).toContain("Checked the cache and it was stale.")
    expect(card?.querySelector(".oa-react-markdown")).not.toBeNull()
    expect(card?.querySelector("summary")).toBeNull()
    expect(card?.textContent).not.toContain("Reasoning")
    root.unmount()
  })
})

import { afterEach, describe, expect, test } from "vite-plus/test"
import type { IntentReporter } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"
import { Window } from "happy-dom"
import { createRoot } from "react-dom/client"

import type { CodexHistoryItem } from "../codex-history-contract.ts"
import {
  ReactTimeline,
  SafeReactMarkdown,
  projectReactTimelineRecords,
  type ReactTimelineRecord,
} from "./react-timeline.tsx"

const restores: Array<() => void> = []
const installDom = () => {
  const window = new Window({ url: "http://localhost/" })
  const values = {
    window,
    document: window.document,
    navigator: window.navigator,
    Node: window.Node,
    Element: window.Element,
    HTMLElement: window.HTMLElement,
    Event: window.Event,
    MouseEvent: window.MouseEvent,
  }
  const previous = new Map<string, PropertyDescriptor | undefined>()
  for (const [name, value] of Object.entries(values)) {
    previous.set(name, Object.getOwnPropertyDescriptor(globalThis, name))
    Object.defineProperty(globalThis, name, { configurable: true, writable: true, value })
  }
  const originalRect = window.HTMLElement.prototype.getBoundingClientRect
  window.HTMLElement.prototype.getBoundingClientRect = function(this: unknown) {
    const element = this as unknown as HTMLElement
    if (!element.dataset.timelineKey) return { x: 0, y: 0, top: 0, left: 0, right: 720, bottom: 100, width: 720, height: 100, toJSON: () => ({}) } as unknown as DOMRect
    const scroll = element.closest<HTMLElement>(".oa-react-timeline-scroll")
    const rows = scroll === null ? [] : [...scroll.querySelectorAll<HTMLElement>("[data-timeline-key]")]
    const rowHeight = (row: HTMLElement): number => row.dataset.timelineKey === "a" ? 60 : row.dataset.timelineKey === "c" ? 80 : 40
    const height = rowHeight(element)
    const top = rows.slice(0, rows.indexOf(element)).reduce((total, row) => total + rowHeight(row), 0) - (scroll?.scrollTop ?? 0)
    return { x: 0, y: top, top, left: 0, right: 720, bottom: top + height, width: 720, height, toJSON: () => ({}) } as unknown as DOMRect
  } as typeof window.HTMLElement.prototype.getBoundingClientRect
  restores.push(() => {
    window.HTMLElement.prototype.getBoundingClientRect = originalRect
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
const settle = (): Promise<void> => new Promise(resolve => setTimeout(resolve, 0))

describe("React typed timeline projection", () => {
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
})

describe("React timeline scroll contract", () => {
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
    expect(container.querySelector('[data-timeline-key="active"]')?.textContent).toContain("Running")
    expect(container.querySelector('.oa-react-working[aria-label="Codex is working"]')).not.toBeNull()
    expect(container.textContent).not.toContain("Token usage update")
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
    render([record("a", 0), record("b", 1), record("c", 2), record("d", 3), record("e", 4)])
    await settle()
    expect(scroll.scrollTop).toBe(20)
    expect(window.document.activeElement).toBe(focusSentinel)
    expect(container.querySelector(".oa-react-new-activity")?.textContent).toContain("Jump to latest")
    ;(container.querySelector(".oa-react-new-activity") as HTMLButtonElement).click()
    await settle()
    expect(scroll.scrollTop).toBe(scroll.scrollHeight)
    render([record("a", 0), record("b", 1), record("c", 2), record("d", 3), record("e", 4), record("f", 5)])
    await settle()
    expect(scroll.scrollTop).toBe(scroll.scrollHeight)
    expect(container.querySelector(".oa-react-new-activity")).toBeNull()
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
    const rows = scroll.querySelectorAll('[data-timeline-key="stream"]')
    expect(rows).toHaveLength(1)
    expect(rows[0]).toBe(before)
    expect(rows[0]?.textContent).toContain("hello")
    expect(scroll.scrollTop).toBe(150)
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

import { Button } from "#components/ui/button"
import { IntentRef, type IntentError, type IntentReporter, type JsonPayload, type MarkdownBlock, type MarkdownInline } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"
import type { ReactElement, ReactNode } from "react"
import { Component, createElement } from "react"

import type { CodexHistoryItem, CodexHistoryPage } from "../codex-history-contract.ts"
import type { DesktopNoteEntry } from "./shell.ts"
import { parseChatMarkdown } from "./markdown.ts"
import { humanizeToolInvocation } from "./tool-cards.ts"

const terminalStatuses = new Set([
  "canceled", "cancelled", "completed", "errored", "failed", "interrupted", "shutdown",
  "task_complete", "task_completed", "turn_aborted", "turn_canceled", "turn_cancelled",
  "turn_complete", "turn_completed", "turn_failed", "turn_interrupted",
])

const field = (item: CodexHistoryItem, name: string): string | null =>
  item.fields.find(entry => entry.label.toLocaleLowerCase() === name)?.value ?? null

const normalizedStatus = (item: CodexHistoryItem): string =>
  (item.status ?? item.label).trim().toLocaleLowerCase().replaceAll(/[ .-]+/g, "_")

const isTerminal = (item: CodexHistoryItem): boolean =>
  item.kind === "lifecycle" && terminalStatuses.has(normalizedStatus(item))

export type ReactTimelineRecord = Readonly<{
  key: string
  itemRef: string
  sequence: number
  kind: CodexHistoryItem["kind"] | "local_message" | "question"
  label: string
  body: string
  timestamp: string
  status: string | null
  redacted: boolean
  fields: CodexHistoryItem["fields"]
  resultRef: string | null
  resultBody: string | null
  resultStatus: string | null
}>

const recordFromItem = (
  item: CodexHistoryItem,
  result: CodexHistoryItem | undefined,
): ReactTimelineRecord => {
  const args = item.kind === "tool_call" ? item.summary || field(item, "input") || "" : ""
  const humanized = item.kind === "tool_call" ? humanizeToolInvocation(item.label, args) : null
  return {
    key: item.itemRef,
    itemRef: item.itemRef,
    sequence: item.sequence,
    kind: item.kind,
    label: humanized?.title ?? item.label,
    body: humanized?.detail || item.summary || "No display text",
    timestamp: item.timestamp,
    status: result?.status ?? item.status,
    redacted: item.redacted || result?.redacted === true,
    fields: item.fields,
    resultRef: result?.itemRef ?? null,
    resultBody: result?.summary ?? null,
    resultStatus: result?.status ?? null,
  }
}

/** Pure presentation over the bounded history contract; no provider parsing. */
export const projectReactTimelineRecords = (
  source: ReadonlyArray<CodexHistoryItem>,
): ReadonlyArray<ReactTimelineRecord> => {
  const byRef = new Map<string, CodexHistoryItem>()
  for (const item of source) byRef.set(item.itemRef, item)
  const items = [...byRef.values()].sort(
    (left, right) => left.sequence - right.sequence || left.itemRef.localeCompare(right.itemRef),
  )
  const newestTerminal = items.filter(isTerminal).at(-1)?.itemRef ?? null
  const resultByCall = new Map<string, CodexHistoryItem>()
  for (const item of items) {
    if (item.kind !== "tool_result") continue
    const callRef = field(item, "call")
    if (callRef !== null) resultByCall.set(callRef, item)
  }
  const consumed = new Set<string>()
  const records: Array<ReactTimelineRecord> = []
  for (const item of items) {
    if (isTerminal(item) && item.itemRef !== newestTerminal) continue
    if (item.kind === "tool_call") {
      const callRef = field(item, "call")
      const result = callRef === null ? undefined : resultByCall.get(callRef)
      if (result !== undefined) consumed.add(result.itemRef)
      records.push(recordFromItem(item, result))
      continue
    }
    if (item.kind === "tool_result" && consumed.has(item.itemRef)) continue
    records.push(recordFromItem(item, undefined))
  }
  return records
}

export const projectLocalTimelineRecords = (
  notes: ReadonlyArray<DesktopNoteEntry>,
): ReadonlyArray<ReactTimelineRecord> => notes.map((note, index) => ({
  key: note.key,
  itemRef: note.key,
  sequence: index,
  kind: note.question === undefined ? "local_message" : "question",
  label: note.question === undefined
    ? note.role === "user" ? "You" : note.role === "assistant" ? "Assistant" : "System"
    : note.question.kind === "tool_approval" ? "Tool approval"
      : note.question.kind === "plan_review" ? "Plan review" : "Question",
  body: note.question?.questions[0]?.question ?? note.text,
  timestamp: note.timestamp,
  status: note.question?.status ?? note.runtime?.kind ?? null,
  redacted: false,
  fields: [],
  resultRef: null,
  resultBody: null,
  resultStatus: null,
}))

const Inline = ({ nodes }: { readonly nodes: ReadonlyArray<MarkdownInline> }): ReactNode => nodes.map((node, index) => {
  if (node.kind === "text") return <span key={index}>{node.text}</span>
  if (node.kind === "code") return <code key={index}>{node.text}</code>
  if (node.kind === "strong") return <strong key={index}><Inline nodes={node.children} /></strong>
  if (node.kind === "emphasis") return <em key={index}><Inline nodes={node.children} /></em>
  return null
})

const Blocks = ({ blocks }: { readonly blocks: ReadonlyArray<MarkdownBlock> }): ReactNode => blocks.map((block, index) => {
  if (block.kind === "paragraph") return <p key={index}><Inline nodes={block.children} /></p>
  if (block.kind === "heading") return createElement(`h${block.level}`, { key: index }, <Inline nodes={block.children} />)
  if (block.kind === "blockquote") return <blockquote key={index}><Blocks blocks={block.children} /></blockquote>
  if (block.kind === "list") {
    const Tag = block.ordered ? "ol" : "ul"
    return <Tag key={index}>{block.items.map((item, itemIndex) => <li key={itemIndex}><Blocks blocks={item} /></li>)}</Tag>
  }
  return null
})

export const SafeReactMarkdown = ({ value }: { readonly value: string }): ReactElement =>
  <div className="oa-react-markdown">
    {parseChatMarkdown(value).map((segment, index) => segment.kind === "markdown"
      ? <Blocks key={index} blocks={segment.blocks} />
      : segment.kind === "code"
        ? <pre key={index}><code data-language={segment.language}>{segment.code}</code></pre>
        : <hr key={index} />)}
  </div>

const recordTone = (record: ReactTimelineRecord): string => {
  if (record.redacted) return "redacted"
  if (record.kind === "gap") return "gap"
  if (record.kind === "error" || ["failed", "errored", "interrupted"].includes(record.status ?? "")) return "danger"
  if (record.kind === "reasoning" || record.kind === "plan") return "reasoning"
  if (record.kind === "tool_call" || record.kind === "tool_result" || record.kind === "approval") return "tool"
  if (record.kind === "usage" || record.kind === "metadata" || record.kind === "context") return "muted"
  if (record.kind === "user_message" || record.label === "You") return "user"
  return "default"
}

const dispatch = (report: IntentReporter, name: string, payload: JsonPayload = null): void => {
  void Effect.runPromise(report(IntentRef(name), payload) as Effect.Effect<void, IntentError>).catch(() => {})
}

export const TimelineItem = ({ record, report }: {
  readonly record: ReactTimelineRecord
  readonly report: IntentReporter
}): ReactElement => {
  const prose = ["assistant_message", "user_message", "agent_message", "local_message"].includes(record.kind)
  return <article
    className="oa-react-timeline-item"
    data-timeline-key={record.key}
    data-kind={record.kind}
    data-tone={recordTone(record)}
    role="listitem"
    aria-label={`${record.label}. Item ${record.sequence + 1}`}
  >
    <header><strong>{record.label}</strong><span>{record.status ?? record.timestamp}</span></header>
    {record.redacted
      ? <p className="oa-react-loss-notice">Content withheld by the history redaction policy.</p>
      : prose
        ? <SafeReactMarkdown value={record.body} />
        : <p>{record.body}</p>}
    {record.kind === "plan" && record.fields.length > 0
      ? <ol className="oa-react-plan-steps">{record.fields.map((entry, index) => <li key={`${entry.label}:${index}`}><strong>{entry.label}</strong><span>{entry.value}</span></li>)}</ol>
      : null}
    {record.kind === "usage" && record.fields.length > 0
      ? <dl className="oa-react-usage">{record.fields.map((entry, index) => <div key={`${entry.label}:${index}`}><dt>{entry.label}</dt><dd>{entry.value}</dd></div>)}</dl>
      : null}
    {record.resultRef === null ? null : <div className="oa-react-tool-result" data-result-ref={record.resultRef}>
      <strong>{record.resultStatus === "failed" ? "Tool failed" : "Tool result"}</strong>
      <p>{record.resultBody || "No result text"}</p>
    </div>}
    {record.kind === "local_message" || record.kind === "question" ? null
      : <Button className="oa-react-item-details" type="button" variant="ghost" size="xs"
          onClick={() => dispatch(report, "HistoryItemSelected", record.itemRef)}
          aria-label={`Show details for ${record.label}, item ${record.sequence + 1}`}>Details</Button>}
  </article>
}

class TimelineItemBoundary extends Component<Readonly<{
  record: ReactTimelineRecord
  report: IntentReporter
}>, Readonly<{ failed: boolean }>> {
  state = { failed: false }
  static getDerivedStateFromError(): Readonly<{ failed: boolean }> { return { failed: true } }
  render(): ReactElement {
    if (this.state.failed) return <article className="oa-react-timeline-item" data-timeline-key={this.props.record.key}
      data-kind="presentation_error" data-tone="danger" role="listitem" aria-label={`Item ${this.props.record.sequence + 1} unavailable`}>
      <header><strong>Item unavailable</strong><span>Presentation error</span></header>
      <p>This item could not be displayed. No completion state was inferred.</p>
    </article>
    return <TimelineItem record={this.props.record} report={this.props.report} />
  }
}

type TimelineProps = Readonly<{
  sessionKey: string
  records: ReadonlyArray<ReactTimelineRecord>
  loadedItemCount: number
  offset: number
  totalItems: number
  loadingEdge: "top" | "bottom" | null
  report: IntentReporter
}>
type TimelineState = Readonly<{ newActivity: boolean; announcement: string }>
type ScrollSnapshot = Readonly<{
  sessionChanged: boolean
  atEnd: boolean
  scrollTop: number
  anchorKey: string | null
  anchorOffset: number
  recordsChanged: boolean
  appended: boolean
}> | null

const atLiveEdge = (element: HTMLElement): boolean =>
  element.scrollHeight - element.scrollTop - element.clientHeight <= 2

/** getSnapshotBeforeUpdate captures old geometry before React mutates rows. */
export class ReactTimeline extends Component<TimelineProps, TimelineState> {
  state: TimelineState = { newActivity: false, announcement: "" }
  private scrollElement: HTMLDivElement | null = null
  private requestedEdge: "top" | "bottom" | null = null

  componentDidMount(): void {
    if (this.scrollElement !== null) this.scrollElement.scrollTop = this.scrollElement.scrollHeight
  }

  getSnapshotBeforeUpdate(previous: TimelineProps): ScrollSnapshot {
    const element = this.scrollElement
    if (element === null) return null
    const previousKeys = previous.records.map(record => record.key)
    const nextKeys = this.props.records.map(record => record.key)
    const signature = (record: ReactTimelineRecord): string =>
      `${record.key}:${record.status ?? ""}:${record.body.length}:${record.resultStatus ?? ""}:${record.resultBody?.length ?? 0}`
    const previousSignatures = previous.records.map(signature)
    const nextSignatures = this.props.records.map(signature)
    const recordsChanged = previousSignatures.join("\u0000") !== nextSignatures.join("\u0000")
    const viewport = element.getBoundingClientRect()
    const rows = [...element.querySelectorAll<HTMLElement>("[data-timeline-key]")]
    const anchor = rows.find(row => row.getBoundingClientRect().bottom > viewport.top) ?? null
    return {
      sessionChanged: previous.sessionKey !== this.props.sessionKey,
      atEnd: atLiveEdge(element),
      scrollTop: element.scrollTop,
      anchorKey: anchor?.dataset.timelineKey ?? null,
      anchorOffset: anchor === null ? 0 : anchor.getBoundingClientRect().top - viewport.top,
      recordsChanged,
      appended: recordsChanged && previousKeys.length > 0 &&
        (nextKeys.at(-1) !== previousKeys.at(-1) || nextSignatures.at(-1) !== previousSignatures.at(-1)),
    }
  }

  componentDidUpdate(previous: TimelineProps, _state: TimelineState, snapshot: ScrollSnapshot): void {
    const element = this.scrollElement
    if (element === null || snapshot === null) return
    if (previous.loadingEdge !== this.props.loadingEdge && this.props.loadingEdge === null) this.requestedEdge = null
    if (!snapshot.recordsChanged && !snapshot.sessionChanged) return
    if (snapshot.sessionChanged) {
      element.scrollTop = element.scrollHeight
      if (this.state.newActivity) this.setState({ newActivity: false, announcement: "Conversation changed" })
      return
    }
    const anchor = snapshot.anchorKey === null
      ? null
      : [...element.querySelectorAll<HTMLElement>("[data-timeline-key]")].find(row => row.dataset.timelineKey === snapshot.anchorKey) ?? null
    const prepended = previous.records.length > 0 && this.props.records.some(record => record.key === previous.records[0]?.key) &&
      this.props.records[0]?.key !== previous.records[0]?.key
    if ((prepended || !snapshot.atEnd) && anchor !== null) {
      element.scrollTop += anchor.getBoundingClientRect().top - element.getBoundingClientRect().top - snapshot.anchorOffset
      if (snapshot.appended && !this.state.newActivity) this.setState({ newActivity: true, announcement: "New activity available" })
      return
    }
    if (snapshot.appended && snapshot.atEnd) {
      element.scrollTop = element.scrollHeight
      this.setState({ newActivity: false, announcement: "Conversation updated" })
      return
    }
    element.scrollTop = snapshot.scrollTop
    if (snapshot.appended && !this.state.newActivity) this.setState({ newActivity: true, announcement: "New activity available" })
  }

  private readonly onScroll = (): void => {
    const element = this.scrollElement
    if (element === null) return
    if (atLiveEdge(element) && this.state.newActivity) this.setState({ newActivity: false, announcement: "At latest activity" })
    if (this.props.loadingEdge !== null || this.requestedEdge !== null) return
    if (this.props.offset > 0 && element.scrollTop < element.clientHeight * 1.5) {
      this.requestedEdge = "top"
      dispatch(this.props.report, "HistoryOlderRequested")
      return
    }
    const windowEnd = this.props.offset + this.props.loadedItemCount
    if (windowEnd < this.props.totalItems && element.scrollHeight - element.scrollTop - element.clientHeight < element.clientHeight * 1.5) {
      this.requestedEdge = "bottom"
      dispatch(this.props.report, "HistoryNewerRequested")
    }
  }

  private readonly jumpToLatest = (): void => {
    const element = this.scrollElement
    if (element === null) return
    element.scrollTop = element.scrollHeight
    this.setState({ newActivity: false, announcement: "At latest activity" })
  }

  render(): ReactElement {
    return <section className="oa-react-timeline-region" aria-label="Conversation timeline">
      <div className="oa-react-sr-only" aria-live="polite" aria-atomic="true">{this.state.announcement}</div>
      <div ref={element => { this.scrollElement = element }} className="oa-react-timeline-scroll"
        data-timeline-session={this.props.sessionKey} onScroll={this.onScroll} role="list"
        aria-label={`${this.props.records.length} loaded conversation items of ${this.props.totalItems}`}>
        {this.props.loadingEdge === "top"
          ? <p className="oa-react-timeline-loading" role="status">Fetching earlier items…</p>
          : this.props.offset > 0
            ? <p className="oa-react-timeline-position">Showing items {this.props.offset + 1}–{this.props.offset + this.props.loadedItemCount} of {this.props.totalItems}</p>
            : null}
        {this.props.records.map(record => <TimelineItemBoundary key={record.key} record={record} report={this.props.report} />)}
        {this.props.loadingEdge === "bottom" ? <p className="oa-react-timeline-loading" role="status">Fetching newer items…</p> : null}
      </div>
      {this.state.newActivity ? <Button className="oa-react-new-activity" size="sm" type="button" onClick={this.jumpToLatest}>Jump to latest</Button> : null}
    </section>
  }
}

export const ConversationTimeline = ({ page, notes, loadingEdge, report }: {
  readonly page: CodexHistoryPage | null
  readonly notes: ReadonlyArray<DesktopNoteEntry>
  readonly loadingEdge: "top" | "bottom" | null
  readonly report: IntentReporter
}): ReactElement => {
  if (page === null && notes.length === 0) return <section className="oa-react-timeline-empty" aria-label="Conversation"><p>Start a conversation with Codex.</p></section>
  const records = page === null ? projectLocalTimelineRecords(notes) : projectReactTimelineRecords(page.items)
  return <ReactTimeline sessionKey={page?.selectedThreadRef ?? "local"} records={records}
    loadedItemCount={page?.items.length ?? records.length} offset={page?.offset ?? 0}
    totalItems={page?.totalItems ?? records.length} loadingEdge={loadingEdge} report={report} />
}

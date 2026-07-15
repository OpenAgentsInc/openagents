import { Button } from "#components/ui/button"
import {
  MessageScroller,
  MessageScrollerButton,
  MessageScrollerContent,
  MessageScrollerItem,
  MessageScrollerProvider,
  MessageScrollerViewport,
} from "#components/ui/message-scroller"
import { ComponentValueBinding, IntentRef, type IntentError, type IntentReporter, type JsonPayload, type MarkdownBlock, type MarkdownInline } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"
import type { ReactElement, ReactNode } from "react"
import { Component, createElement, useEffect, useRef } from "react"
import { Folder } from "lucide-react"

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
    // Transport/accounting scaffolding stays available in the bounded history
    // inspector; it is not a primary conversation row.
    if (["session", "context", "metadata", "usage"].includes(item.kind)) continue
    // Persisted reasoning can intentionally be only a redaction marker. Absence
    // is the honest primary presentation, not a false failure card.
    if (item.kind === "reasoning" && item.redacted) continue
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
): ReadonlyArray<ReactTimelineRecord> => notes.flatMap((note, index) => {
  if (note.role === "system" && /^(Usage|Connected)\s*·/i.test(note.text)) return []
  const kind: ReactTimelineRecord["kind"] = note.question !== undefined
    ? "question"
    : note.role !== "system"
      ? "local_message"
      : /^Reasoning\s*·/i.test(note.text)
        ? "reasoning"
        : /^Plan\s*·/i.test(note.text)
          ? "plan"
          : /^Approval\s*·/i.test(note.text)
            ? "approval"
            : /^Turn (completed|complete|canceled|cancelled)$/i.test(note.text)
              ? "lifecycle"
              : /^Turn (failed|interrupted)|error/i.test(note.text)
                ? "error"
                : /\s·\s(?:running|completed|failed|errored)$/i.test(note.text)
                  ? "tool_call"
                  : "system_message"
  const label = note.question !== undefined
    ? note.question.kind === "tool_approval" ? "Tool approval"
      : note.question.kind === "plan_review" ? "Plan review" : "Question"
    : note.role === "user" ? "You"
      : note.role === "assistant" ? "Assistant"
        : kind === "tool_call" ? note.text.split(" · ")[0] || "Tool"
          : kind === "reasoning" ? "Reasoning"
            : kind === "plan" ? "Plan"
              : "System"
  return [{
    key: note.key,
    itemRef: note.key,
    sequence: index,
    kind,
    label,
    body: note.question?.questions[0]?.question ?? note.text.replace(/^(Reasoning|Plan|Approval)\s*·\s*/i, ""),
    timestamp: note.timestamp,
    status: note.question?.status ?? note.runtime?.kind ?? null,
    redacted: false,
    fields: [],
    resultRef: null,
    resultBody: null,
    resultStatus: null,
  }]
})

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

const isUserRecord = (record: ReactTimelineRecord): boolean =>
  record.kind === "user_message" || record.label === "You"

const isMessageRecord = (record: ReactTimelineRecord): boolean =>
  isUserRecord(record) || ["assistant_message", "agent_message", "local_message"].includes(record.kind)

const isWorkRecord = (record: ReactTimelineRecord): boolean =>
  ["reasoning", "tool_call", "tool_result", "approval", "collaboration"].includes(record.kind)

const compact = (value: string, limit = 180): string => {
  const normalized = value.replaceAll("\\n", " ").replaceAll(/\s+/g, " ").trim()
  return normalized.length <= limit ? normalized : `${normalized.slice(0, limit - 1)}…`
}

const dispatch = (report: IntentReporter, name: string, payload: JsonPayload = null): void => {
  void Effect.runPromise(report(
    payload === null ? IntentRef(name) : IntentRef(name, ComponentValueBinding()), payload,
  ) as Effect.Effect<void, IntentError>).catch(() => {})
}

export const TimelineItem = ({ record, report }: {
  readonly record: ReactTimelineRecord
  readonly report: IntentReporter
}): ReactElement => {
  if (record.kind === "lifecycle" && !["failed", "errored", "interrupted"].includes(record.status ?? "")) {
    return <span data-timeline-key={record.key} data-kind="lifecycle" hidden />
  }
  if (isWorkRecord(record)) return <details className="oa-react-work-entry"
    data-timeline-key={record.key} data-kind={record.kind} role="listitem">
    <summary>
      <span className="oa-react-work-label">{record.label}</span>
      <span className="oa-react-work-preview">{compact(record.resultBody || record.body)}</span>
      <span className="oa-react-work-status" data-status={record.status ?? "completed"}>
        {["failed", "errored", "interrupted"].includes(record.status ?? "") ? "Failed" : record.status === "running" ? "Running" : "Done"}
      </span>
    </summary>
    <div className="oa-react-work-detail">
      {record.redacted ? <p>Details unavailable.</p> : <pre><code>{record.body}</code></pre>}
      {record.resultBody === null ? null : <><strong>{record.resultStatus === "failed" ? "Result · failed" : "Result"}</strong><pre><code>{record.resultBody}</code></pre></>}
      <Button className="oa-react-item-details" type="button" variant="ghost" size="xs"
        onClick={() => dispatch(report, "HistoryItemSelected", record.itemRef)}>Inspect event</Button>
    </div>
  </details>

  if (record.kind === "plan") return <article className="oa-react-plan" data-timeline-key={record.key} data-kind="plan" role="listitem">
    <header><strong>Plan</strong><span>{record.status}</span></header>
    <SafeReactMarkdown value={record.body} />
    {record.fields.length > 0 ? <ol>{record.fields.map((entry, index) => <li key={`${entry.label}:${index}`}><span>{entry.value}</span><small>{entry.label}</small></li>)}</ol> : null}
  </article>

  const danger = record.kind === "error" || record.kind === "gap" || ["failed", "errored", "interrupted"].includes(record.status ?? "")
  if (!isMessageRecord(record) || danger || record.redacted) return <article className="oa-react-notice"
    data-timeline-key={record.key} data-kind={record.kind} data-danger={danger ? "true" : "false"} role="listitem">
    <strong>{danger ? record.label : "Update"}</strong>
    <span>{record.redacted ? "Message content unavailable." : record.body}</span>
  </article>

  return <article
    className="oa-react-timeline-item"
    data-timeline-key={record.key}
    data-kind={record.kind}
    data-tone={isUserRecord(record) ? "user" : "assistant"}
    role="listitem"
    aria-label={`${record.label}. Item ${record.sequence + 1}`}
  >
    <SafeReactMarkdown value={record.body} />
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
  working?: boolean
  report: IntentReporter
}>
const TimelineRecords = ({ records, report }: {
  readonly records: ReadonlyArray<ReactTimelineRecord>
  readonly report: IntentReporter
}): ReactElement => {
  const output: Array<ReactElement> = []
  for (let index = 0; index < records.length;) {
    const record = records[index]!
    if (!isWorkRecord(record)) {
      output.push(<MessageScrollerItem key={record.key} messageId={record.key} scrollAnchor={isUserRecord(record)}>
        <TimelineItemBoundary record={record} report={report} />
      </MessageScrollerItem>)
      index += 1
      continue
    }
    const group: Array<ReactTimelineRecord> = []
    while (index < records.length && isWorkRecord(records[index]!)) group.push(records[index++]!)
    if (group.length === 1) {
      output.push(<MessageScrollerItem key={group[0]!.key} messageId={group[0]!.key}>
        <TimelineItemBoundary record={group[0]!} report={report} />
      </MessageScrollerItem>)
      continue
    }
    const running = group.some(entry => entry.status === "running")
    const visible = running ? group.slice(-1) : []
    const folded = running ? group.slice(0, -1) : group
    const groupKey = `work:${group[0]!.key}:${group.at(-1)!.key}`
    output.push(<MessageScrollerItem key={groupKey} messageId={groupKey}>
      <div className="oa-react-work-group" role="listitem">
        <details>
          <summary className="oa-react-work-group-summary"><strong>{running ? `+${folded.length} previous` : "Worked"}</strong><span>{folded.length} {folded.length === 1 ? "activity" : "activities"}</span></summary>
          <div role="list">{folded.map(entry => <TimelineItemBoundary key={entry.key} record={entry} report={report} />)}</div>
        </details>
        {visible.map(entry => <TimelineItemBoundary key={entry.key} record={entry} report={report} />)}
      </div>
    </MessageScrollerItem>)
  }
  return <>{output}</>
}

const TimelineScroller = (props: TimelineProps): ReactElement => {
  const viewportRef = useRef<HTMLDivElement>(null)
  const requestedEdge = useRef<"top" | "bottom" | null>(null)
  const previousLoadingEdge = useRef(props.loadingEdge)
  useEffect(() => {
    if (previousLoadingEdge.current !== props.loadingEdge && props.loadingEdge === null) requestedEdge.current = null
    previousLoadingEdge.current = props.loadingEdge
  }, [props.loadingEdge])
  const onScroll = (): void => {
    const element = viewportRef.current
    if (element === null || props.loadingEdge !== null || requestedEdge.current !== null) return
    if (props.offset > 0 && element.scrollTop < element.clientHeight * 1.5) {
      requestedEdge.current = "top"
      dispatch(props.report, "HistoryOlderRequested")
      return
    }
    const windowEnd = props.offset + props.loadedItemCount
    if (windowEnd < props.totalItems && element.scrollHeight - element.scrollTop - element.clientHeight < element.clientHeight * 1.5) {
      requestedEdge.current = "bottom"
      dispatch(props.report, "HistoryNewerRequested")
    }
  }
  const releaseReaderIntent = (): void => {
    const element = viewportRef.current
    if (element === null) return
    const event = typeof WheelEvent === "function"
      ? new WheelEvent("wheel", { bubbles: true, deltaY: 0 })
      : new Event("wheel", { bubbles: true })
    element.dispatchEvent(event)
  }
  return <MessageScroller className="oa-react-timeline-region" aria-label="Conversation timeline">
    <MessageScrollerViewport ref={viewportRef} className="oa-react-timeline-scroll"
      data-timeline-session={props.sessionKey} onScroll={onScroll}
      onPointerDownCapture={releaseReaderIntent} onSelect={releaseReaderIntent}
      aria-label={`${props.records.length} loaded conversation items of ${props.totalItems}`}>
      <MessageScrollerContent className="oa-react-timeline-content" aria-busy={props.working === true}>
        {props.loadingEdge === "top"
          ? <p className="oa-react-timeline-loading" role="status">Fetching earlier items…</p>
          : props.offset > 0
            ? <p className="oa-react-timeline-position">Showing items {props.offset + 1}–{props.offset + props.loadedItemCount} of {props.totalItems}</p>
            : null}
        <TimelineRecords records={props.records} report={props.report} />
        {props.working ? <MessageScrollerItem messageId="working-indicator"><div className="oa-react-working" role="status" aria-label="Codex is working">
          <span>Working</span><i /><i /><i />
        </div></MessageScrollerItem> : null}
        {props.loadingEdge === "bottom" ? <p className="oa-react-timeline-loading" role="status">Fetching newer items…</p> : null}
      </MessageScrollerContent>
    </MessageScrollerViewport>
    <MessageScrollerButton className="oa-react-new-activity" behavior="auto" aria-label="Jump to latest" title="Jump to latest" />
  </MessageScroller>
}

export const ReactTimeline = (props: TimelineProps): ReactElement =>
  <MessageScrollerProvider key={props.sessionKey} autoScroll defaultScrollPosition="last-anchor" scrollPreviousItemPeek={64}>
    <TimelineScroller {...props} />
  </MessageScrollerProvider>

export const ConversationTimeline = ({ page, notes, loadingEdge, working, workingDirectory, report }: {
  readonly page: CodexHistoryPage | null
  readonly notes: ReadonlyArray<DesktopNoteEntry>
  readonly loadingEdge: "top" | "bottom" | null
  readonly working?: boolean
  readonly workingDirectory: string | null
  readonly report: IntentReporter
}): ReactElement => {
  if (page === null && notes.length === 0) return <section className="oa-react-timeline-empty" aria-label="Conversation">
    <div className="oa-react-empty-conversation">
      <h2>Start a conversation with Codex</h2>
      <p className="oa-react-empty-working-directory" aria-label={workingDirectory === null ? "Working directory unavailable" : `Working directory: ${workingDirectory}`}>
        <Folder aria-hidden="true" data-icon-name="Folder" />
        <code title={workingDirectory ?? undefined}>{workingDirectory ?? "Working directory unavailable"}</code>
      </p>
    </div>
  </section>
  const records = page === null ? projectLocalTimelineRecords(notes) : projectReactTimelineRecords(page.items)
  return <ReactTimeline sessionKey={page?.selectedThreadRef ?? "local"} records={records}
    loadedItemCount={page?.items.length ?? records.length} offset={page?.offset ?? 0}
    totalItems={page?.totalItems ?? records.length} loadingEdge={loadingEdge} working={working} report={report} />
}

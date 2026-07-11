import { Badge, Button, ComponentValueBinding, IconButton, IntentRef, NavRail, SplitPane, Stack, StaticPayload, Table, Text, Timeline, defineIntent, type IconName, type TimelineEvent, type View } from "@effect-native/core"
import { Schema } from "@effect-native/core/effect"
import type { CodexHistoryCatalog, CodexHistoryItem, CodexHistoryPage } from "../codex-history-contract.ts"
import { chatMarkdownBody } from "./markdown.ts"
import { humanizeToolInvocation } from "./tool-cards.ts"

export type HistoryWorkspaceState = Readonly<{
  catalog: CodexHistoryCatalog
  page: CodexHistoryPage | null
  selectedItemRef: string | null
  railCollapsed: boolean
  expandedThreadRefs: ReadonlyArray<string>
  pendingThreadRef?: string | null
  visibleRootCount: number
}>
export const historyCatalogPageSize = 40
export const emptyHistoryWorkspaceState = (): HistoryWorkspaceState => ({ catalog: { roots: [], agents: [] }, page: null, selectedItemRef: null, railCollapsed: false, expandedThreadRefs: [], pendingThreadRef: null, visibleRootCount: historyCatalogPageSize })

export const HistoryConversationSelected = defineIntent("HistoryConversationSelected", Schema.String)
export const HistoryAgentSelected = defineIntent("HistoryAgentSelected", Schema.String)
export const HistoryItemSelected = defineIntent("HistoryItemSelected", Schema.String)
export const HistoryPageRequested = defineIntent("HistoryPageRequested", Schema.Number)
export const HistoryInspectorToggled = defineIntent("HistoryInspectorToggled", Schema.Null)
export const HistoryAgentExpandedToggled = defineIntent("HistoryAgentExpandedToggled", Schema.String)
export const HistoryCatalogMoreRequested = defineIntent("HistoryCatalogMoreRequested", Schema.Null)
export const historyWorkspaceIntents = [HistoryConversationSelected, HistoryAgentSelected, HistoryItemSelected, HistoryPageRequested, HistoryInspectorToggled, HistoryAgentExpandedToggled, HistoryCatalogMoreRequested] as const

const statusLabel = (status: string): string => status.slice(0,1).toUpperCase() + status.slice(1)
const agentStatusIcon = (status: NonNullable<CodexHistoryPage["agents"][number]>["status"]): IconName =>
  status === "completed" ? "Check"
    : status === "interrupted" || status === "errored" || status === "not_found" ? "X"
      : status === "running" ? "Play"
        : status === "pending" || status === "waiting" ? "Pause"
          : status === "shutdown" ? "Stop" : "Circle"
const timelinePreview = (value: string): string => value.length <= 360 ? value : `${value.slice(0,357)}…`
const timelineStatus = (item: CodexHistoryItem): "idle" | "active" | "success" | "failed" | "pending" =>
  item.kind === "error" || item.kind === "gap" || item.status === "failed" || item.status === "errored" ? "failed"
    : item.status === "running" || item.status === "streaming" ? "active"
      : item.status === "pending" || item.status === "waiting" ? "pending"
        : item.status === "completed" || item.status === "success" ? "success" : "idle"

const agentTimelineStatus = (status: NonNullable<CodexHistoryItem["relatedAgent"]>["status"]): "idle" | "active" | "success" | "failed" | "pending" =>
  status === "running" ? "active"
    : status === "pending" || status === "waiting" ? "pending"
      : status === "completed" ? "success"
        : status === "interrupted" || status === "errored" || status === "not_found" ? "failed" : "idle"

const historyField = (item: CodexHistoryItem, label: string): string | null =>
  item.fields.find((field) => field.label.toLowerCase() === label)?.value ?? null

const toolIcon = (label: string): IconName => {
  const name = label.toLowerCase()
  if (["exec", "exec_command", "write_stdin", "shell", "bash", "command_execution"].includes(name)) return "Terminal"
  if (["apply_patch", "edit", "write", "write_file", "create_file"].includes(name)) return "Code"
  if (["read", "read_file", "list", "glob", "grep", "find"].includes(name)) return "Folder"
  return "Tools"
}

const agentName = (value: string | null): string | null => {
  const name = value?.split("/").filter(Boolean).at(-1)
  return name === undefined ? null : name.replaceAll("_"," ")
}

const agentMessageTitle = (item: CodexHistoryItem): string => {
  const type=historyField(item,"message type")
  const task=agentName(historyField(item,"task"))
  const action=type==="NEW_TASK"?"Task assigned":type==="MESSAGE"?"Agent message":type==="CLOSE"?"Agent closed":"Agent message"
  return task===null?action:`${action} · ${task}`
}

const agentMessageRoute = (item: CodexHistoryItem): string => {
  const sender=agentName(historyField(item,"sender"));const recipient=agentName(historyField(item,"recipient"))
  return sender!==null&&recipient!==null?`${sender} → ${recipient}`:sender!==null?`From ${sender}`:recipient!==null?`To ${recipient}`:"Inter-agent handoff"
}

const agentMessageDetail = (item: CodexHistoryItem): string => {
  const payload=historyField(item,"payload")
  const route=agentMessageRoute(item)
  return payload===null?route:`${route} — ${payload}`
}

const historyVariant = (item: CodexHistoryItem): TimelineEvent["variant"] =>
  item.kind === "metadata" ? "metadata"
    : item.kind === "agent_message" ? "agent"
    : item.kind === "tool_call" || item.kind === "tool_result" || item.kind === "approval" || item.kind === "collaboration" ? "tool"
    : item.kind === "reasoning" || item.kind === "plan" ? "reasoning"
      : item.kind === "error" || item.kind === "gap" ? "error"
        : "message"

const historyIcon = (item: CodexHistoryItem): IconName =>
  item.kind === "metadata" ? "ChevronRight"
    : item.kind === "agent_message" ? "Agent"
    : item.kind === "tool_call" || item.kind === "tool_result" ? toolIcon(item.label)
    : item.kind === "collaboration" ? "Agent"
      : item.kind === "approval" ? "Check"
        : item.kind === "reasoning" || item.kind === "plan" ? "Sparkles"
          : item.kind === "error" || item.kind === "gap" ? "X"
            : "Chats"

const projectedTimelineEvent = (item: CodexHistoryItem, result?: CodexHistoryItem, expandedItemRef: string | null = null): TimelineEvent => {
  if (item.kind === "metadata") {
    const expanded = expandedItemRef === item.itemRef
    return {
      id: item.itemRef,
      key: `history-item-${item.itemRef}`,
      label: "Agent metadata",
      ...(expanded ? { detail: timelinePreview(item.summary), time: "Click to collapse" } : { time: "Click to expand" }),
      status: "idle",
      variant: "metadata",
      icon: expanded ? "ChevronDown" : "ChevronRight",
      accessibilityLabel: `Agent metadata, ${expanded ? "expanded. Click to collapse" : "collapsed. Click to expand"}`,
      refs: [item.threadRef],
    }
  }
  if (item.kind === "agent_message") {
    const label=agentMessageTitle(item);const detail=agentMessageDetail(item)
    return {id:item.itemRef,key:`history-item-${item.itemRef}`,label,detail:timelinePreview(detail),status:"idle",variant:"agent",icon:"Agent",accessibilityLabel:`${label}. ${detail}`,refs:[item.threadRef]}
  }
  if (item.kind === "collaboration" && item.relatedAgent !== undefined) {
    const agent = item.relatedAgent
    const detail = agent.latest === null
      ? "No child activity recorded yet."
      : `${agent.latest.label} — ${agent.latest.summary}`
    return {
      id: item.itemRef,
      key: `history-item-${item.itemRef}`,
      label: `Subagent · ${agent.title}`,
      detail: timelinePreview(detail),
      time: statusLabel(agent.status),
      status: agentTimelineStatus(agent.status),
      variant: "agent",
      icon: "Agent",
      onSelect: IntentRef("HistoryAgentSelected", StaticPayload(agent.threadRef)),
      accessibilityLabel: `Open subagent ${agent.title}. ${statusLabel(agent.status)}. Latest activity: ${detail}`,
      refs: [item.threadRef, agent.threadRef],
    }
  }
  const status = result?.status ?? item.status
  // Historical tool cards humanize through the SAME table chat tool cards use
  // (EP250 owner directive: no raw JSON — and never an opaque continuation
  // blob — in the default card body; raw input stays in the item inspector).
  const argsText = item.kind === "tool_call" ? item.summary || historyField(item, "input") || "" : ""
  const humanized = item.kind === "tool_call" ? humanizeToolInvocation(item.label, argsText) : null
  // Plain-text tool inputs (already human, e.g. "bun test") pass through
  // bounded; JSON-shaped inputs never render raw.
  const plainArgs = argsText.trimStart().startsWith("{") || argsText.trimStart().startsWith("[") ? "" : argsText
  const detail = humanized !== null
    ? humanized.detail || plainArgs || result?.summary || "Tool invocation"
    : item.summary || "No display text"
  const label = humanized !== null ? humanized.title : item.label
  return {
    id: item.itemRef,
    key: `history-item-${item.itemRef}`,
    label,
    detail: timelinePreview(detail),
    ...(status === null || status === undefined ? {} : { time: status }),
    status: timelineStatus(result ?? item),
    variant: historyVariant(item),
    icon: historyIcon(item),
    accessibilityLabel: `${label}. ${detail}. Source item ${item.sequence + 1}`,
    refs: [item.threadRef, ...(result === undefined ? [] : [result.itemRef])],
  }
}

/**
 * Message PROSE renders through the shared markdown projector (EP250 owner
 * directive: "i see assistant messages showing raw markdown what the fuck.
 * need to use the same markdown renderer we use elsewhere"). Prose entries
 * carry the full item so the view can render the same unboxed Markdown/
 * CodeBlock body chat assistant messages use; everything else stays a typed
 * timeline event row. Loss-accounting is untouched: every source item still
 * projects exactly once (as prose OR as an event, never both, never dropped
 * beyond the same intentional filters as before).
 */
export type HistoryEntryProjection =
  | Readonly<{ kind: "prose"; item: CodexHistoryItem }>
  | Readonly<{ kind: "events"; key: string; events: ReadonlyArray<TimelineEvent> }>

/**
 * Loss-accounting notices stay OUT of the markdown projection: a fully
 * redacted body (the `[REDACTED: …]` placeholder) renders as a plain styled
 * event row, never markdown-parsed prose.
 */
const isProseItem = (item: CodexHistoryItem): boolean =>
  (item.kind === "assistant_message" || item.kind === "user_message" || item.kind === "agent_message") &&
  !item.summary.trimStart().startsWith("[REDACTED:") &&
  (item.kind !== "agent_message" || historyField(item, "payload") !== null)

/** The prose text an item displays — agent messages display their payload. */
export const historyProseText = (item: CodexHistoryItem): string =>
  item.kind === "agent_message" ? historyField(item, "payload") ?? item.summary : item.summary

export const projectHistoryEntries = (items: ReadonlyArray<CodexHistoryItem>, expandedItemRef: string | null = null): ReadonlyArray<HistoryEntryProjection> => {
  const resultByCall = new Map<string, CodexHistoryItem>()
  for (const item of items) {
    if (item.kind !== "tool_result") continue
    const call = historyField(item, "call")
    if (call !== null) resultByCall.set(call, item)
  }
  const consumedResults = new Set<string>()
  const entries: Array<HistoryEntryProjection> = []
  let run: Array<TimelineEvent> = []
  const flushRun = (): void => {
    if (run.length === 0) return
    entries.push({ kind: "events", key: run[0]!.id, events: run })
    run = []
  }
  for (const item of items) {
    if (["usage", "session", "context", "lifecycle", "system_message"].includes(item.kind)) continue
    if (item.kind === "reasoning" && (item.summary.trim() === "" || item.summary.startsWith("[REDACTED:"))) continue
    if ((item.kind === "assistant_message" || item.kind === "user_message") && (item.summary.trim() === "" || item.summary.trim().toLowerCase() === item.label.trim().toLowerCase())) continue
    if (isProseItem(item)) {
      flushRun()
      entries.push({ kind: "prose", item })
      continue
    }
    if (item.kind === "tool_call") {
      const call = historyField(item, "call")
      const result = call === null ? undefined : resultByCall.get(call)
      if (result !== undefined) consumedResults.add(result.itemRef)
      run.push(projectedTimelineEvent(item, result, expandedItemRef))
      continue
    }
    if (item.kind === "tool_result" && consumedResults.has(item.itemRef)) continue
    run.push(projectedTimelineEvent(item, undefined, expandedItemRef))
  }
  flushRun()
  return entries
}

/** Flat event view of the non-prose entries (compat + tests). */
export const projectHistoryTimelineEvents = (items: ReadonlyArray<CodexHistoryItem>, expandedItemRef: string | null = null): ReadonlyArray<TimelineEvent> =>
  projectHistoryEntries(items, expandedItemRef).flatMap(entry => entry.kind === "events" ? entry.events : [])

/**
 * The visible agent roster of the open conversation — the exact rows the
 * right-rail Agents tree shows (collapsed subtrees excluded). Shared by the
 * tree view and the Cmd+Shift+Up/Down traversal shortcut so both walk the
 * same list.
 */
export const visibleHistoryAgents = (state: HistoryWorkspaceState): ReadonlyArray<CodexHistoryPage["agents"][number]> => {
  const allAgents = state.page?.agents ?? []
  const byId = new Map(allAgents.map(agent => [agent.threadRef, agent]))
  return allAgents.filter(agent => { let parent = agent.parentThreadRef; while (parent) { if (!state.expandedThreadRefs.includes(parent)) return false; parent = byId.get(parent)?.parentThreadRef ?? null } return true })
}

/**
 * Cmd+Shift+Up/Down agent traversal (EP250 owner directive: "just like
 * command up and down scrolsl thru chats, have command shift up and down go
 * up and down the agents of a convo."). Returns the threadRef the shortcut
 * should select, or null when there is nothing to do. Ends CLAMP — the same
 * boundary behavior as the Cmd+Up/Down conversation shortcut.
 */
export const historyAgentTraversalTarget = (state: HistoryWorkspaceState, delta: -1 | 1): string | null => {
  const page = state.page
  if (page === null) return null
  const agents = visibleHistoryAgents(state)
  if (agents.length === 0) return null
  const activeIndex = agents.findIndex(agent => agent.threadRef === page.selectedThreadRef)
  const targetIndex = Math.max(0, Math.min(agents.length - 1, activeIndex < 0 ? (delta > 0 ? 0 : agents.length - 1) : activeIndex + delta))
  const target = agents[targetIndex]!.threadRef
  return target === page.selectedThreadRef ? null : target
}

/**
 * Shifted-variant discrimination for the traversal shortcut: platform
 * modifier + Shift + ArrowUp/ArrowDown, no Alt. The UNSHIFTED chord stays
 * conversation traversal and never matches here.
 */
export const isHistoryAgentTraversalShortcut = (
  event: Readonly<{ key: string; metaKey: boolean; ctrlKey: boolean; altKey: boolean; shiftKey: boolean }>,
  platform: string | undefined,
): boolean =>
  (event.key === "ArrowUp" || event.key === "ArrowDown") &&
  event.shiftKey && !event.altKey &&
  (platform === "darwin" ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey)

const agentTree = (state: HistoryWorkspaceState): View => {
  const page = state.page; const allAgents = page?.agents ?? []; const agents = visibleHistoryAgents(state)
  return Stack({ key: "history-agent-tree-region", direction: "column", gap: "2", style: { minWidth: 0, minHeight: 0, flex: 1 }, a11y: { role: "region", label: "Agents" } }, [
    Text({ key: "history-agent-title", content: `Agents · ${agents.length}`, variant: "heading", color: "textPrimary" }),
    NavRail({ key: "history-agent-list", role: "tree", activeId: page === null ? undefined : `history-agent-${page.selectedThreadRef}`, style: { minHeight: 0, flex: 1 }, a11y: { role: "tree", label: `${allAgents.length} agents` }, sections: [{ id: "history-agent-tree", items: agents.map((agent, index) => ({
      id: `history-agent-${agent.threadRef}`,
      label: agent.title,
      icon: agentStatusIcon(agent.status),
      depth: agent.depth,
      selected: page?.selectedThreadRef === agent.threadRef,
      ...(agent.descendantCount>0 ? { expanded: state.expandedThreadRefs.includes(agent.threadRef) } : {}),
      positionInSet: index + 1,
      setSize: agents.length,
      accessibilityLabel: `${agent.title}, ${statusLabel(agent.status)}, ${agent.role??"role unavailable"}, ${agent.model??"model unavailable"}, ${agent.agentPath??"path unavailable"}, level ${agent.depth + 1}`,
      onSelect: IntentRef("HistoryAgentSelected", StaticPayload(agent.threadRef)),
      interactions: { onKey: [
        { key: "ArrowUp", preventDefault: true, intent: IntentRef("HistoryAgentSelected", StaticPayload(agents[Math.max(0,index-1)]?.threadRef ?? agent.threadRef)) },
        { key: "ArrowDown", preventDefault: true, intent: IntentRef("HistoryAgentSelected", StaticPayload(agents[Math.min(agents.length-1,index+1)]?.threadRef ?? agent.threadRef)) },
        { key: "Home", preventDefault: true, intent: IntentRef("HistoryAgentSelected", StaticPayload(agents[0]?.threadRef ?? agent.threadRef)) },
        { key: "End", preventDefault: true, intent: IntentRef("HistoryAgentSelected", StaticPayload(agents.at(-1)?.threadRef ?? agent.threadRef)) },
        { key: "ArrowLeft", preventDefault: true, intent: agent.descendantCount>0&&state.expandedThreadRefs.includes(agent.threadRef)?IntentRef("HistoryAgentExpandedToggled",StaticPayload(agent.threadRef)):IntentRef("HistoryAgentSelected", StaticPayload(agent.parentThreadRef ?? agent.threadRef)) },
        { key: "ArrowRight", preventDefault: true, intent: agent.descendantCount>0&&!state.expandedThreadRefs.includes(agent.threadRef)?IntentRef("HistoryAgentExpandedToggled",StaticPayload(agent.threadRef)):IntentRef("HistoryAgentSelected", StaticPayload(agents.find(candidate => candidate.parentThreadRef === agent.threadRef)?.threadRef ?? agent.threadRef)) },
      ] },
    })) }] }),
  ])
}

/**
 * Memoized markdown projection: historical prose is immutable, so the parsed
 * body is cached per item ref. Bounded — the cache resets when it grows past
 * the window a reader can plausibly accumulate.
 */
const proseBodyCache = new Map<string, ReadonlyArray<View>>()
const historyProseBody = (item: CodexHistoryItem, keyPrefix: string): ReadonlyArray<View> => {
  const text = historyProseText(item)
  const cacheKey = `${keyPrefix}:${item.itemRef}:${text.length}`
  const cached = proseBodyCache.get(cacheKey)
  if (cached !== undefined) return cached
  if (proseBodyCache.size > 4_000) proseBodyCache.clear()
  const body = chatMarkdownBody(`${keyPrefix}-${item.itemRef}-md`, text)
  proseBodyCache.set(cacheKey, body)
  return body
}

const proseHeader = (item: CodexHistoryItem): string | null =>
  item.kind === "user_message"
    ? "YOU"
    : item.kind === "agent_message"
      ? `${agentMessageTitle(item)} — ${agentMessageRoute(item)}`
      : null

/**
 * Unboxed prose row — the chat assistant treatment (markdown body, no box,
 * textFaint meta ladder) applied to historical message prose. The compact
 * details affordance dispatches the SAME HistoryItemSelected intent timeline
 * rows dispatch, so the inspector flow is one path.
 */
const proseRow = (item: CodexHistoryItem): View =>
  Stack(
    { key: `history-item-${item.itemRef}`, direction: "column", gap: "1", style: { width: "full" }, a11y: { role: "listitem", label: `${item.label} message. Source item ${item.sequence + 1}` } },
    [
      ...(proseHeader(item) === null ? [] : [Text({ key: `history-item-${item.itemRef}-header`, content: proseHeader(item)!, variant: "caption", color: "textFaint" })]),
      ...historyProseBody(item, "history-item"),
      Stack({ key: `history-item-${item.itemRef}-meta-row`, direction: "row", gap: "1", align: "center" }, [
        Button({
          key: `history-item-details-${item.itemRef}`,
          label: "details",
          variant: "ghost",
          style: { padding: "0", borderWidth: 0, typeScale: "caption", color: "textFaint" },
          onPress: IntentRef("HistoryItemSelected", StaticPayload(item.itemRef)),
          a11y: { label: `Show item details, ${item.label} message, source item ${item.sequence + 1}` },
        }),
      ]),
    ],
  )

const inspector = (state: HistoryWorkspaceState): View => {
  const item = state.page?.items.find(value => value.itemRef === state.selectedItemRef)
  if (!item || item.kind === "metadata") return agentTree(state)
  return Stack({ key: "history-item-inspector", direction: "column", gap: "2", style: { minWidth: 0, minHeight: 0, flex: 1 }, a11y: { role: "region", label: "Selected history item" } }, [
    Button({ key: "history-item-back", label: "Back to agents", variant: "ghost", onPress: IntentRef("HistoryItemSelected", StaticPayload("")), a11y: { label: "Back to agent tree" } }),
    Text({ key: "history-item-title", content: item.label, variant: "heading", color: "textPrimary" }),
    Badge({ key: "history-item-kind", label: item.kind, tone: item.kind === "gap" ? "warn" : "neutral" }),
    // Message prose in the inspector renders through the same markdown
    // projector as the transcript; loss-accounting notices stay plain text.
    ...(isProseItem(item)
      ? historyProseBody(item, "history-inspector")
      : [Text({ key: "history-item-summary", content: item.summary || "No display text", variant: "body", color: "textPrimary" })]),
    ...(item.fields.length === 0 ? [] : [Table({ key: "history-item-fields", columns: [{ id: "field", header: "Field" }, { id: "value", header: "Value" }], rows: item.fields.map((entry,index) => ({ id: String(index), cells: [Text({ key: `history-item-field-label-${index}`, content: entry.label, variant: "caption", color: "textMuted" }), Text({ key: `history-item-field-value-${index}`, content: entry.value, variant: "body", color: "textPrimary" })] })) })]),
    Text({ key: "history-item-source", content: `Source ${item.sourceType} · item ${item.sequence + 1}${item.redacted ? " · redacted" : ""}`, variant: "caption", color: item.redacted ? "warning" : "textMuted" }),
  ])
}

export const historyWorkspaceView = (state: HistoryWorkspaceState): View => {
  const page = state.page
  if (!page) return Stack({ key: "history-workspace-empty", direction: "column", gap: "2", style: { flex: 1, minWidth: 0 } }, [Text({ key: "history-empty-title", content: "Select a Codex conversation", variant: "heading", color: "textPrimary" }), Text({ key: "history-empty-copy", content: "Historical conversations and every discovered subagent are available without a 24-hour cutoff.", variant: "body", color: "textMuted" })])
  const center = Stack({ key: "history-center", direction: "column", gap: "2", style: { flex: 1, minWidth: 0, minHeight: 0 } }, [
    IconButton({key:"history-agents-drawer",icon:"Agent",accessibilityLabel:`${state.railCollapsed?"Open":"Close"} agents inspector, ${page.agents.length} agents`,onPress:IntentRef("HistoryInspectorToggled"),surface:"glass",a11y:{expanded:!state.railCollapsed}}),
    Stack({ key: "history-timeline-page", direction: "column", gap: "2", style: { flex: 1, minHeight: 0, minWidth: 0 }, a11y: { role: "list", label: `History items ${page.offset + 1} through ${Math.min(page.totalItems, page.offset + page.items.length)} of ${page.totalItems}` } },
      projectHistoryEntries(page.items, state.selectedItemRef).map(entry =>
        entry.kind === "prose"
          ? proseRow(entry.item)
          : Timeline({ key: `history-seg-${entry.key}`, ...(state.selectedItemRef === null ? {} : { selectedId: state.selectedItemRef }), onEventSelect: IntentRef("HistoryItemSelected", ComponentValueBinding()), events: entry.events }))),
    Stack({ key: "history-page-controls", direction: "row", gap: "2", align: "center" }, [Button({ key: "history-page-previous", label: "Previous", variant: "secondary", disabled: !page.hasPrevious, onPress: IntentRef("HistoryPageRequested", StaticPayload(Math.max(0,page.offset-page.limit))), a11y: { label: "Previous history page" } }), Text({ key: "history-page-range", content: `${page.offset + 1}–${Math.min(page.totalItems,page.offset+page.items.length)} of ${page.totalItems}`, variant: "caption", color: "textMuted" }), Button({ key: "history-page-next", label: "Next", variant: "secondary", disabled: !page.hasNext, onPress: IntentRef("HistoryPageRequested", StaticPayload(page.offset+page.limit)), a11y: { label: "Next history page" } })]),
  ])
  return SplitPane({ key: "history-workspace-split", orientation: "row", style: { flex: 1, minWidth: 0, minHeight: 0 }, onCollapseToggle: IntentRef("HistoryInspectorToggled"), panes: [{ id: "history-center", min: 360, content: center }, { id: "history-inspector", min: 280, max: 480, size: 336, collapsed: state.railCollapsed, content: inspector(state) }] })
}

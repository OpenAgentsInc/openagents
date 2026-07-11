import { Badge, Button, ComponentValueBinding, IconButton, IntentRef, NavRail, SplitPane, Stack, StaticPayload, Table, Text, Timeline, defineIntent, type IconName, type TimelineEvent, type View } from "@effect-native/core"
import { Schema } from "@effect-native/core/effect"
import type { CodexHistoryCatalog, CodexHistoryItem, CodexHistoryPage } from "../codex-history-contract.ts"

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

const toolTitle = (label: string): string => {
  const name = label.toLowerCase()
  if (["exec", "exec_command", "write_stdin", "shell", "bash", "command_execution"].includes(name)) return "Terminal"
  if (name === "apply_patch") return "Edited files"
  if (["read", "read_file"].includes(name)) return "Read file"
  if (["list", "glob", "grep", "find"].includes(name)) return "Searched files"
  return label.replaceAll("_", " ").replace(/^./, (value) => value.toUpperCase())
}

const historyVariant = (item: CodexHistoryItem): TimelineEvent["variant"] =>
  item.kind === "tool_call" || item.kind === "tool_result" || item.kind === "approval" || item.kind === "collaboration" ? "tool"
    : item.kind === "reasoning" || item.kind === "plan" ? "reasoning"
      : item.kind === "error" || item.kind === "gap" ? "error"
        : "message"

const historyIcon = (item: CodexHistoryItem): IconName =>
  item.kind === "tool_call" || item.kind === "tool_result" ? toolIcon(item.label)
    : item.kind === "collaboration" ? "Agent"
      : item.kind === "approval" ? "Check"
        : item.kind === "reasoning" || item.kind === "plan" ? "Sparkles"
          : item.kind === "error" || item.kind === "gap" ? "X"
            : "Chats"

const projectedTimelineEvent = (item: CodexHistoryItem, result?: CodexHistoryItem): TimelineEvent => {
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
  const detail = item.kind === "tool_call"
    ? item.summary || historyField(item, "input") || result?.summary || "Tool invocation"
    : item.summary || "No display text"
  const label = item.kind === "tool_call" ? toolTitle(item.label) : item.label
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
 * Turns the loss-accounted Codex event stream into the product timeline.
 * Raw usage, session, context, lifecycle, and developer/system records remain
 * in the page contract and inspector data; they are intentionally not chat
 * rows. Matching tool results are folded into their invocation.
 */
export const projectHistoryTimelineEvents = (items: ReadonlyArray<CodexHistoryItem>): ReadonlyArray<TimelineEvent> => {
  const resultByCall = new Map<string, CodexHistoryItem>()
  for (const item of items) {
    if (item.kind !== "tool_result") continue
    const call = historyField(item, "call")
    if (call !== null) resultByCall.set(call, item)
  }
  const consumedResults = new Set<string>()
  const events: Array<TimelineEvent> = []
  for (const item of items) {
    if (["usage", "session", "context", "lifecycle", "system_message"].includes(item.kind)) continue
    if (item.kind === "reasoning" && (item.summary.trim() === "" || item.summary.startsWith("[REDACTED:"))) continue
    if ((item.kind === "assistant_message" || item.kind === "user_message") && (item.summary.trim() === "" || item.summary.trim().toLowerCase() === item.label.trim().toLowerCase())) continue
    if (item.kind === "tool_call") {
      const call = historyField(item, "call")
      const result = call === null ? undefined : resultByCall.get(call)
      if (result !== undefined) consumedResults.add(result.itemRef)
      events.push(projectedTimelineEvent(item, result))
      continue
    }
    if (item.kind === "tool_result" && consumedResults.has(item.itemRef)) continue
    events.push(projectedTimelineEvent(item))
  }
  return events
}

const agentTree = (state: HistoryWorkspaceState): View => {
  const page = state.page; const allAgents = page?.agents ?? []; const byId=new Map(allAgents.map(agent=>[agent.threadRef,agent])); const agents=allAgents.filter(agent=>{let parent=agent.parentThreadRef;while(parent){if(!state.expandedThreadRefs.includes(parent))return false;parent=byId.get(parent)?.parentThreadRef??null}return true})
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

const inspector = (state: HistoryWorkspaceState): View => {
  const item = state.page?.items.find(value => value.itemRef === state.selectedItemRef)
  if (!item) return agentTree(state)
  return Stack({ key: "history-item-inspector", direction: "column", gap: "2", style: { minWidth: 0, minHeight: 0, flex: 1 }, a11y: { role: "region", label: "Selected history item" } }, [
    Button({ key: "history-item-back", label: "Back to agents", variant: "ghost", onPress: IntentRef("HistoryItemSelected", StaticPayload("")), a11y: { label: "Back to agent tree" } }),
    Text({ key: "history-item-title", content: item.label, variant: "heading", color: "textPrimary" }),
    Badge({ key: "history-item-kind", label: item.kind, tone: item.kind === "gap" ? "warn" : "neutral" }),
    Text({ key: "history-item-summary", content: item.summary || "No display text", variant: "body", color: "textPrimary" }),
    ...(item.fields.length === 0 ? [] : [Table({ key: "history-item-fields", columns: [{ id: "field", header: "Field" }, { id: "value", header: "Value" }], rows: item.fields.map((entry,index) => ({ id: String(index), cells: [Text({ key: `history-item-field-label-${index}`, content: entry.label, variant: "caption", color: "textMuted" }), Text({ key: `history-item-field-value-${index}`, content: entry.value, variant: "body", color: "textPrimary" })] })) })]),
    Text({ key: "history-item-source", content: `Source ${item.sourceType} · item ${item.sequence + 1}${item.redacted ? " · redacted" : ""}`, variant: "caption", color: item.redacted ? "warning" : "textMuted" }),
  ])
}

export const historyWorkspaceView = (state: HistoryWorkspaceState): View => {
  const page = state.page
  if (!page) return Stack({ key: "history-workspace-empty", direction: "column", gap: "2", style: { flex: 1, minWidth: 0 } }, [Text({ key: "history-empty-title", content: "Select a Codex conversation", variant: "heading", color: "textPrimary" }), Text({ key: "history-empty-copy", content: "Historical conversations and every discovered subagent are available without a 24-hour cutoff.", variant: "body", color: "textMuted" })])
  const center = Stack({ key: "history-center", direction: "column", gap: "2", style: { flex: 1, minWidth: 0, minHeight: 0 } }, [
    IconButton({key:"history-agents-drawer",icon:"Agent",accessibilityLabel:`${state.railCollapsed?"Open":"Close"} agents inspector, ${page.agents.length} agents`,onPress:IntentRef("HistoryInspectorToggled"),surface:"glass",a11y:{expanded:!state.railCollapsed}}),
    Timeline({ key: "history-timeline-page", ...(state.selectedItemRef === null ? {} : { selectedId: state.selectedItemRef }), onEventSelect: IntentRef("HistoryItemSelected", ComponentValueBinding()), style: { flex: 1, minHeight: 0, minWidth: 0 }, a11y: { role: "list", label: `History items ${page.offset + 1} through ${Math.min(page.totalItems, page.offset + page.items.length)} of ${page.totalItems}` }, events: projectHistoryTimelineEvents(page.items) }),
    Stack({ key: "history-page-controls", direction: "row", gap: "2", align: "center" }, [Button({ key: "history-page-previous", label: "Previous", variant: "secondary", disabled: !page.hasPrevious, onPress: IntentRef("HistoryPageRequested", StaticPayload(Math.max(0,page.offset-page.limit))), a11y: { label: "Previous history page" } }), Text({ key: "history-page-range", content: `${page.offset + 1}–${Math.min(page.totalItems,page.offset+page.items.length)} of ${page.totalItems}`, variant: "caption", color: "textMuted" }), Button({ key: "history-page-next", label: "Next", variant: "secondary", disabled: !page.hasNext, onPress: IntentRef("HistoryPageRequested", StaticPayload(page.offset+page.limit)), a11y: { label: "Next history page" } })]),
  ])
  return SplitPane({ key: "history-workspace-split", orientation: "row", style: { flex: 1, minWidth: 0, minHeight: 0 }, onCollapseToggle: IntentRef("HistoryInspectorToggled"), panes: [{ id: "history-center", min: 360, content: center }, { id: "history-inspector", min: 280, max: 480, size: 336, collapsed: state.railCollapsed, content: inspector(state) }] })
}

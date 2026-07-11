import { Badge, Button, IntentRef, List, SplitPane, Stack, StaticPayload, StatusBanner, Text, defineIntent, type KeyedView, type View } from "@effect-native/core"
import { Schema } from "@effect-native/core/effect"
import type { CodexHistoryCatalog, CodexHistoryItem, CodexHistoryPage } from "../codex-history-contract.ts"

export type HistoryWorkspaceState = Readonly<{
  catalog: CodexHistoryCatalog
  page: CodexHistoryPage | null
  selectedItemRef: string | null
  railCollapsed: boolean
  expandedThreadRefs: ReadonlyArray<string>
  pendingThreadRef?: string | null
}>
export const emptyHistoryWorkspaceState = (): HistoryWorkspaceState => ({ catalog: { roots: [], agents: [] }, page: null, selectedItemRef: null, railCollapsed: false, expandedThreadRefs: [], pendingThreadRef: null })

export const HistoryConversationSelected = defineIntent("HistoryConversationSelected", Schema.String)
export const HistoryAgentSelected = defineIntent("HistoryAgentSelected", Schema.String)
export const HistoryItemSelected = defineIntent("HistoryItemSelected", Schema.String)
export const HistoryPageRequested = defineIntent("HistoryPageRequested", Schema.Number)
export const HistoryInspectorToggled = defineIntent("HistoryInspectorToggled", Schema.Null)
export const HistoryAgentExpandedToggled = defineIntent("HistoryAgentExpandedToggled", Schema.String)
export const historyWorkspaceIntents = [HistoryConversationSelected, HistoryAgentSelected, HistoryItemSelected, HistoryPageRequested, HistoryInspectorToggled, HistoryAgentExpandedToggled] as const

const statusLabel = (status: string): string => status.slice(0,1).toUpperCase() + status.slice(1)
const itemRow = (item: CodexHistoryItem, selected: boolean): KeyedView => Button({
  key: `history-item-${item.itemRef}`,
  label: `${item.label} — ${item.summary || "No display text"}`,
  variant: "ghost",
  onPress: IntentRef("HistoryItemSelected", StaticPayload(item.itemRef)),
  a11y: { label: `${item.label}. ${item.summary}. Source item ${item.sequence + 1}`, selected },
  style: { width: "full", textAlign: "left", ...(selected ? { backgroundColor: "surfaceRaised" as const } : {}), ...(item.kind === "gap" ? { borderWidth: 1 as const, borderColor: "warning" as const } : { borderWidth: 0 as const }) },
}) as KeyedView

const agentTree = (state: HistoryWorkspaceState): View => {
  const page = state.page; const allAgents = page?.agents ?? []; const byId=new Map(allAgents.map(agent=>[agent.threadRef,agent])); const agents=allAgents.filter(agent=>{let parent=agent.parentThreadRef;while(parent){if(!state.expandedThreadRefs.includes(parent))return false;parent=byId.get(parent)?.parentThreadRef??null}return true})
  const selected=agents.find(agent=>agent.threadRef===page?.selectedThreadRef)
  return Stack({ key: "history-agent-tree-region", direction: "column", gap: "2", style: { minWidth: 0, minHeight: 0, flex: 1 }, a11y: { role: "tree", label: "Agents" } }, [
    Text({ key: "history-agent-title", content: `Agents · ${agents.length}`, variant: "heading", color: "textPrimary" }),
    ...(selected?[Text({key:"history-agent-config",content:[selected.agentPath??"path unavailable",selected.nickname,selected.role,selected.model,selected.reasoning,selected.sourceVersion].filter(Boolean).join(" · "),variant:"caption",color:"textMuted"})]:[]),
    List({ key: "history-agent-list", virtualize: agents.length > 30, estimatedItemSize: 44, style: { minHeight: 0, flex: 1 }, a11y: { role: "tree", label: `${agents.length} agents` } }, agents.map((agent, index) => Button({
      key: `history-agent-${agent.threadRef}`,
      label: `${"  ".repeat(agent.depth)}${agent.title} · ${statusLabel(agent.status)}`,
      variant: "ghost",
      onPress: IntentRef("HistoryAgentSelected", StaticPayload(agent.threadRef)),
      a11y: { role: "treeitem", label: `${agent.title}, ${statusLabel(agent.status)}, ${agent.role??"role unavailable"}, ${agent.model??"model unavailable"}, ${agent.agentPath??"path unavailable"}, level ${agent.depth + 1}`, selected: page?.selectedThreadRef === agent.threadRef, ...(agent.descendantCount>0?{expanded:state.expandedThreadRefs.includes(agent.threadRef)}:{}), level: agent.depth + 1, positionInSet: index + 1, setSize: agents.length, tabIndex: page?.selectedThreadRef === agent.threadRef ? 0 : -1 },
      interactions: { onKey: [
        { key: "ArrowUp", preventDefault: true, intent: IntentRef("HistoryAgentSelected", StaticPayload(agents[Math.max(0,index-1)]?.threadRef ?? agent.threadRef)) },
        { key: "ArrowDown", preventDefault: true, intent: IntentRef("HistoryAgentSelected", StaticPayload(agents[Math.min(agents.length-1,index+1)]?.threadRef ?? agent.threadRef)) },
        { key: "Home", preventDefault: true, intent: IntentRef("HistoryAgentSelected", StaticPayload(agents[0]?.threadRef ?? agent.threadRef)) },
        { key: "End", preventDefault: true, intent: IntentRef("HistoryAgentSelected", StaticPayload(agents.at(-1)?.threadRef ?? agent.threadRef)) },
        { key: "ArrowLeft", preventDefault: true, intent: agent.descendantCount>0&&state.expandedThreadRefs.includes(agent.threadRef)?IntentRef("HistoryAgentExpandedToggled",StaticPayload(agent.threadRef)):IntentRef("HistoryAgentSelected", StaticPayload(agent.parentThreadRef ?? agent.threadRef)) },
        { key: "ArrowRight", preventDefault: true, intent: agent.descendantCount>0&&!state.expandedThreadRefs.includes(agent.threadRef)?IntentRef("HistoryAgentExpandedToggled",StaticPayload(agent.threadRef)):IntentRef("HistoryAgentSelected", StaticPayload(agents.find(candidate => candidate.parentThreadRef === agent.threadRef)?.threadRef ?? agent.threadRef)) },
      ] },
      style: { width: "full", textAlign: "left", paddingLeft: agent.depth === 0 ? "1" : agent.depth === 1 ? "3" : "5", ...(page?.selectedThreadRef === agent.threadRef ? { backgroundColor: "surfaceRaised" as const } : {}) },
    }) as KeyedView)),
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
    ...item.fields.map((entry,index) => Stack({ key: `history-item-field-${index}`, direction: "column", gap: "1" }, [Text({ key: `history-item-field-label-${index}`, content: entry.label, variant: "caption", color: "textMuted" }), Text({ key: `history-item-field-value-${index}`, content: entry.value, variant: "body", color: "textPrimary" })])),
    Text({ key: "history-item-source", content: `Source ${item.sourceType} · item ${item.sequence + 1}${item.redacted ? " · redacted" : ""}`, variant: "caption", color: item.redacted ? "warning" : "textMuted" }),
  ])
}

export const historyWorkspaceView = (state: HistoryWorkspaceState): View => {
  const page = state.page
  if (!page) return Stack({ key: "history-workspace-empty", direction: "column", gap: "2", style: { flex: 1, minWidth: 0 } }, [Text({ key: "history-empty-title", content: "Select a Codex conversation", variant: "heading", color: "textPrimary" }), Text({ key: "history-empty-copy", content: "Historical conversations and every discovered subagent are available without a 24-hour cutoff.", variant: "body", color: "textMuted" })])
  const selected = page.agents.find(agent => agent.threadRef === page.selectedThreadRef)
  const center = Stack({ key: "history-center", direction: "column", gap: "2", style: { flex: 1, minWidth: 0, minHeight: 0 } }, [
    Stack({ key: "history-center-heading", direction: "row", gap: "2", align: "center" }, [Text({ key: "history-center-title", content: selected?.title ?? "Codex history", variant: "heading", color: "textPrimary" }), Badge({ key: "history-center-status", label: statusLabel(selected?.status ?? "unknown"), tone: selected?.status === "errored" ? "danger" : "neutral" }), Button({key:"history-agents-drawer",label:`Agents · ${page.agents.length}`,variant:"secondary",onPress:IntentRef("HistoryInspectorToggled"),a11y:{label:`${state.railCollapsed?"Open":"Close"} agents inspector, ${page.agents.length} agents`,expanded:!state.railCollapsed}})]),
    StatusBanner({ key: "history-completeness", tone: page.completeness.gaps === 0 ? "success" : "warn", message: `${page.completeness.source} source = ${page.completeness.rendered} rendered + ${page.completeness.redactions} redactions + ${page.completeness.gaps} gaps` }),
    List({ key: "history-timeline-page", virtualize: page.items.length > 30, estimatedItemSize: 56, style: { flex: 1, minHeight: 0, minWidth: 0 }, a11y: { role: "list", label: `History items ${page.offset + 1} through ${Math.min(page.totalItems, page.offset + page.items.length)} of ${page.totalItems}` } }, page.items.map(item => itemRow(item, item.itemRef === state.selectedItemRef))),
    Stack({ key: "history-page-controls", direction: "row", gap: "2", align: "center" }, [Button({ key: "history-page-previous", label: "Previous", variant: "secondary", disabled: !page.hasPrevious, onPress: IntentRef("HistoryPageRequested", StaticPayload(Math.max(0,page.offset-page.limit))), a11y: { label: "Previous history page" } }), Text({ key: "history-page-range", content: `${page.offset + 1}–${Math.min(page.totalItems,page.offset+page.items.length)} of ${page.totalItems}`, variant: "caption", color: "textMuted" }), Button({ key: "history-page-next", label: "Next", variant: "secondary", disabled: !page.hasNext, onPress: IntentRef("HistoryPageRequested", StaticPayload(page.offset+page.limit)), a11y: { label: "Next history page" } })]),
  ])
  return SplitPane({ key: "history-workspace-split", orientation: "row", style: { flex: 1, minWidth: 0, minHeight: 0 }, onCollapseToggle: IntentRef("HistoryInspectorToggled"), panes: [{ id: "history-center", min: 360, content: center }, { id: "history-inspector", min: 280, max: 480, size: 336, collapsed: state.railCollapsed, content: inspector(state) }] })
}

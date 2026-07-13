import { Badge, Button, ComponentValueBinding, IconButton, IntentRef, NavRail, SplitPane, Stack, StaticPayload, Table, Text, TextField, Timeline, Tooltip, defineIntent, type IconName, type TimelineEvent, type View } from "@effect-native/core"
import { Schema } from "@effect-native/core/effect"
import type { CodexHistoryCatalog, CodexHistoryItem, CodexHistoryPage, CodexHistorySearchResult, CodexHistorySource } from "../codex-history-contract.ts"
import type { DesktopThread } from "../chat-contract.ts"
import { chatMarkdownBody } from "./markdown.ts"
import { humanizeToolInvocation } from "./tool-cards.ts"

export type HistoryWorkspaceState = Readonly<{
  catalog: CodexHistoryCatalog
  /**
   * The loaded WINDOW of the open conversation: `items` accumulate as the
   * reader scrolls up (prepends) or down (appends); `offset` is the window
   * start; `totalItems`/`completeness` stay whole-conversation truth. Fetch
   * order changed for the bottom-anchored flow (EP250) — what counts as
   * rendered/redacted/gap did not.
   */
  page: CodexHistoryPage | null
  selectedItemRef: string | null
  railCollapsed: boolean
  expandedThreadRefs: ReadonlyArray<string>
  pendingThreadRef?: string | null
  visibleRootCount: number
  /** Which window edge is fetching — drives the honest thin loading row. */
  loadingEdge: "top" | "bottom" | null
  /**
   * Free-text session search (#8712 H4). A cache over the loss-accounted
   * catalog/page truth — never authority. `searchResults` is the ranked
   * response for `searchQuery`; opening a content result windows the session
   * on its matching item (reusing the bottom-anchored restore-to-item flow).
   */
  searchQuery: string
  searchResults: ReadonlyArray<CodexHistorySearchResult>
  searchTruncated: boolean
  /** App-local threads are the H1 resume-picker candidates. Keeping this list
   * separate from provider history prevents an imported rollout from being
   * mistaken for a resumable local SDK thread. */
  localThreads?: ReadonlyArray<DesktopThread>
  resumePickerOpen?: boolean
  actionNotice?: string | null
}>
export const historyCatalogPageSize = 40
export const historyItemPageSize = 50
export const emptyHistoryWorkspaceState = (): HistoryWorkspaceState => ({ catalog: { roots: [], agents: [] }, page: null, selectedItemRef: null, railCollapsed: false, expandedThreadRefs: [], pendingThreadRef: null, visibleRootCount: historyCatalogPageSize, loadingEdge: null, searchQuery: "", searchResults: [], searchTruncated: false })

export const HistoryConversationSelected = defineIntent("HistoryConversationSelected", Schema.String)
export const HistoryAgentSelected = defineIntent("HistoryAgentSelected", Schema.String)
export const HistoryItemSelected = defineIntent("HistoryItemSelected", Schema.String)
export const HistoryOlderRequested = defineIntent("HistoryOlderRequested", Schema.Null)
export const HistoryNewerRequested = defineIntent("HistoryNewerRequested", Schema.Null)
export const HistoryInspectorToggled = defineIntent("HistoryInspectorToggled", Schema.Null)
export const HistoryAgentExpandedToggled = defineIntent("HistoryAgentExpandedToggled", Schema.String)
export const HistoryCatalogMoreRequested = defineIntent("HistoryCatalogMoreRequested", Schema.Null)
export const HistorySearchChanged = defineIntent("HistorySearchChanged", Schema.String)
export const HistorySearchResultOpened = defineIntent("HistorySearchResultOpened", Schema.String)
export const HistorySearchCleared = defineIntent("HistorySearchCleared", Schema.Null)
export const HistoryResumePickerToggled = defineIntent("HistoryResumePickerToggled", Schema.Null)
export const HistoryResumeThreadSelected = defineIntent("HistoryResumeThreadSelected", Schema.String)
export const HistoryForkRequested = defineIntent("HistoryForkRequested", Schema.Struct({
  sourceThreadRef: Schema.String,
  throughSequence: Schema.NullOr(Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0))),
}))
export const historyWorkspaceIntents = [HistoryConversationSelected, HistoryAgentSelected, HistoryItemSelected, HistoryOlderRequested, HistoryNewerRequested, HistoryInspectorToggled, HistoryAgentExpandedToggled, HistoryCatalogMoreRequested, HistorySearchChanged, HistorySearchResultOpened, HistorySearchCleared, HistoryResumePickerToggled, HistoryResumeThreadSelected, HistoryForkRequested] as const

/** Human source badge for a merged catalog/search row (#8712 H3). */
export const historySourceBadgeLabel = (source: CodexHistorySource): string => source === "claude" ? "Claude" : "Codex"

/** Whether the search surface is active (non-blank query). */
export const historySearchActive = (state: HistoryWorkspaceState): boolean => state.searchQuery.trim() !== ""

/**
 * The matching item to open a search result on — a content result windows on
 * its exact item; a title result opens the session at its end. Returns the
 * restore anchor the bottom-anchored fetch plan understands.
 */
export const historySearchOpenAnchor = (result: CodexHistorySearchResult): Readonly<{ kind: "item"; itemRef: string }> | Readonly<{ kind: "end" }> =>
  result.matchItemRef === null ? { kind: "end" } : { kind: "item", itemRef: result.matchItemRef }

/** Sidebar rows for the ranked search results (source-badged, open-at-item). */
export const historySearchResultSidebarItems = (state: HistoryWorkspaceState): ReadonlyArray<Readonly<{ id: string; label: string; meta: string; accessibilityLabel: string; onSelect: ReturnType<typeof IntentRef> }>> =>
  state.searchResults.map(result => ({
    id: `sidebar-search-${result.threadRef}`,
    label: result.title,
    meta: historySourceBadgeLabel(result.source),
    accessibilityLabel: `Open ${historySourceBadgeLabel(result.source)} session ${result.title}, ${result.matchKind === "title" ? "title match" : `matches: ${result.snippet}`}`,
    onSelect: IntentRef("HistorySearchResultOpened", StaticPayload(result.threadRef)),
  }))

/** The search field view (shared by the sidebar host). */
export const historySearchField = (state: HistoryWorkspaceState): View =>
  TextField({
    key: "history-search-field",
    value: state.searchQuery,
    placeholder: "Search all sessions…",
    a11y: { label: "Search Codex and Claude session titles and content" },
    onChange: IntentRef("HistorySearchChanged", ComponentValueBinding()),
    style: { width: "full" },
  })

// ---------------------------------------------------------------------------
// Bottom-anchored windowed loading (EP250 owner directive: "you need to show
// the most recent messages, starting at bottom, and auto load them as i
// scroll up, smartly loading before the cursor"). Pure window math — the
// completeness equation and counted gaps are whole-conversation and change
// with FETCH ORDER never.
// ---------------------------------------------------------------------------

/** Open at the END: the offset of the final page of a conversation. */
export const historyTailOffset = (totalItems: number, limit: number = historyItemPageSize): number =>
  Math.max(0, totalItems - limit)

/** The containing-page offset for one item — restore windows around it. */
export const historyItemPageOffset = (sequence: number, limit: number = historyItemPageSize): number =>
  Math.max(0, Math.floor(sequence / limit) * limit)

const dedupeHistoryItems = (items: ReadonlyArray<CodexHistoryItem>): ReadonlyArray<CodexHistoryItem> => {
  const seen = new Set<string>()
  return items.filter(item => seen.has(item.itemRef) ? false : (seen.add(item.itemRef), true))
}

/** Prepend an older fetched page onto the loaded window (scroll-up fill). */
export const mergeHistoryWindowUp = (window: CodexHistoryPage, older: CodexHistoryPage): CodexHistoryPage => {
  const items = dedupeHistoryItems([...older.items, ...window.items])
  const offset = Math.min(window.offset, older.offset)
  return { ...window, items, offset, hasPrevious: offset > 0, hasNext: offset + items.length < window.totalItems }
}

/** Append a newer fetched page onto the loaded window (scroll-down fill). */
export const mergeHistoryWindowDown = (window: CodexHistoryPage, newer: CodexHistoryPage): CodexHistoryPage => {
  const items = dedupeHistoryItems([...window.items, ...newer.items])
  return { ...window, items, hasPrevious: window.offset > 0, hasNext: window.offset + items.length < window.totalItems }
}

/**
 * SMART PREFETCH: trigger the older-page fetch when the reader crosses
 * ~1.5 viewport heights from the top of loaded content — the page is ready
 * BEFORE the boundary is hit.
 */
export const historyPrefetchViewportFactor = 1.5
export const historyShouldFetchOlder = (input: Readonly<{ scrollTop: number; clientHeight: number; offset: number; loadingEdge: "top" | "bottom" | null }>): boolean =>
  input.loadingEdge === null && input.offset > 0 && input.scrollTop < input.clientHeight * historyPrefetchViewportFactor

export const historyShouldFetchNewer = (input: Readonly<{ scrollTop: number; clientHeight: number; scrollHeight: number; windowEnd: number; totalItems: number; loadingEdge: "top" | "bottom" | null }>): boolean =>
  input.loadingEdge === null && input.windowEnd < input.totalItems &&
  input.scrollHeight - (input.scrollTop + input.clientHeight) < input.clientHeight * historyPrefetchViewportFactor

/** Honest position caption at the loading edge. */
export const historyPositionCaption = (page: CodexHistoryPage): string => {
  const end = Math.min(page.totalItems, page.offset + page.items.length)
  return end >= page.totalItems && page.offset === 0
    ? `Showing all ${page.totalItems.toLocaleString("en-US")} items`
    : `Showing ${(page.offset + 1).toLocaleString("en-US")}–${end.toLocaleString("en-US")} of ${page.totalItems.toLocaleString("en-US")}`
}

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
    : item.kind === "context" ? "metadata"
    : item.kind === "agent_message" ? "agent"
    : item.kind === "tool_call" || item.kind === "tool_result" || item.kind === "approval" || item.kind === "collaboration" ? "tool"
    : item.kind === "reasoning" || item.kind === "plan" ? "reasoning"
      : item.kind === "error" || item.kind === "gap" ? "error"
        : "message"

const historyIcon = (item: CodexHistoryItem): IconName =>
  item.kind === "metadata" ? "ChevronRight"
    : item.kind === "context" ? "History"
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
    if (["usage", "session", "lifecycle", "system_message"].includes(item.kind) ||
      (item.kind === "context" && item.label !== "History compacted")) continue
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
        Tooltip(
          { key: `history-item-details-tooltip-${item.itemRef}`, content: "Details", placement: { side: "top", align: "start" } },
          [IconButton({
            key: `history-item-details-${item.itemRef}`,
            icon: "InfoCircle",
            accessibilityLabel: `Show item details, ${item.label} message, source item ${item.sequence + 1}`,
            onPress: IntentRef("HistoryItemSelected", StaticPayload(item.itemRef)),
          })],
        ),
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
  const localThreads = state.localThreads ?? []
  const selectedSequence = state.selectedItemRef === null
    ? page.items.at(-1)?.sequence ?? null
    : page.items.find(item => item.itemRef === state.selectedItemRef)?.sequence ?? null
  const actionBar = Stack({ key: "history-thread-actions", direction: "column", gap: "1", style: { width: "full" } }, [
    Stack({ key: "history-thread-action-buttons", direction: "row", gap: "2", align: "center" }, [
      Tooltip(
        { key: "history-resume-picker-tooltip", content: "Resume local chat", placement: { side: "bottom", align: "start" } },
        [IconButton({
          key: "history-resume-picker-toggle",
          icon: "History",
          accessibilityLabel: localThreads.length === 0 ? "Resume local chat unavailable, no local chats" : `Choose one of ${localThreads.length} local chats to resume`,
          disabled: localThreads.length === 0,
          surface: "glass",
          onPress: IntentRef("HistoryResumePickerToggled"),
        })],
      ),
      Tooltip(
        { key: "history-fork-tooltip", content: "Fork from here", placement: { side: "bottom", align: "start" } },
        [IconButton({
          key: "history-fork-from-here",
          icon: "Branch",
          accessibilityLabel: selectedSequence === null ? "Fork unavailable, no history item selected" : `Fork ${historySourceBadgeLabel(page.agents.find(agent => agent.threadRef === page.rootThreadRef)?.source ?? "codex")} session through item ${selectedSequence + 1} into a new local chat`,
          disabled: selectedSequence === null,
          surface: "glass",
          onPress: IntentRef("HistoryForkRequested", StaticPayload({ sourceThreadRef: page.selectedThreadRef, throughSequence: selectedSequence })),
        })],
      ),
    ]),
    ...((state.resumePickerOpen ?? false) ? [Stack({ key: "history-resume-picker", direction: "column", gap: "1", a11y: { role: "region", label: "Local chats available to resume" } }, localThreads.map(thread =>
      Button({
        key: `history-resume-thread-${thread.id}`,
        label: thread.title,
        variant: "ghost",
        onPress: IntentRef("HistoryResumeThreadSelected", StaticPayload(thread.id)),
        a11y: { label: `Resume local chat ${thread.title}` },
      }),
    ))] : []),
    ...(state.actionNotice === undefined || state.actionNotice === null ? [] : [Text({ key: "history-action-notice", content: state.actionNotice, variant: "caption", color: "warning" })]),
  ])
  const center = Stack({ key: "history-center", direction: "column", gap: "2", style: { flex: 1, minWidth: 0, minHeight: 0 } }, [
    actionBar,
    IconButton({key:"history-agents-drawer",icon:"Agent",accessibilityLabel:`${state.railCollapsed?"Open":"Close"} agents inspector, ${page.agents.length} agents`,onPress:IntentRef("HistoryInspectorToggled"),surface:"glass",a11y:{expanded:!state.railCollapsed}}),
    Stack({ key: `history-timeline-page-${page.selectedThreadRef}`, direction: "column", gap: "2", preserveScrollAnchor: true, style: { flex: 1, minHeight: 0, minWidth: 0 }, a11y: { role: "list", label: `History items ${page.offset + 1} through ${Math.min(page.totalItems, page.offset + page.items.length)} of ${page.totalItems}` } }, [
      // Honest thin loading row / position caption at the top loading edge.
      // No Previous/Next pager: older pages auto-load as the reader scrolls
      // up (EP250 bottom-anchored flow).
      ...(state.loadingEdge === "top"
        ? [Text({ key: "history-fetch-earlier", content: "Fetching earlier items…", variant: "caption", color: "textFaint" })]
        : page.offset > 0
          ? [Text({ key: "history-position-caption", content: historyPositionCaption(page), variant: "caption", color: "textFaint" })]
          : []),
      ...projectHistoryEntries(page.items, state.selectedItemRef).map(entry =>
        entry.kind === "prose"
          ? proseRow(entry.item)
          : Timeline({ key: `history-seg-${entry.key}`, ...(state.selectedItemRef === null ? {} : { selectedId: state.selectedItemRef }), onEventSelect: IntentRef("HistoryItemSelected", ComponentValueBinding()), events: entry.events })),
      ...(state.loadingEdge === "bottom"
        ? [Text({ key: "history-fetch-newer", content: "Fetching newer items…", variant: "caption", color: "textFaint" })]
        : []),
    ]),
  ])
  return SplitPane({ key: "history-workspace-split", orientation: "row", style: { flex: 1, minWidth: 0, minHeight: 0 }, onCollapseToggle: IntentRef("HistoryInspectorToggled"), panes: [{ id: "history-center", min: 360, content: center }, { id: "history-inspector", min: 280, max: 480, size: 336, collapsed: state.railCollapsed, content: inspector(state) }] })
}

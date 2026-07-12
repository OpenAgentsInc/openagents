import type { LiveAgentGraphEntity } from "@openagentsinc/khala-sync"

type LiveAgentGraphNode = LiveAgentGraphEntity["nodes"][number]
type LiveAgentGraphStatus = LiveAgentGraphNode["status"]

export type LiveAgentGraphAuthority = "live" | "historical"

export type LiveAgentGraphTone =
  | "active"
  | "attention"
  | "success"
  | "danger"
  | "muted"

/**
 * One typed per-node token attribution input. The canonical
 * `openagents.live_agent_graph.v1` snapshot deliberately carries no usage
 * fields, so token truth arrives as a separate typed ledger (for example the
 * desktop-local fold's `usageAttributions()`). `usageTruth: "exact"` is
 * honored only when the reported usage split is complete and well-formed;
 * anything else is presented as loss-accounted, never synthesized.
 */
export type LiveAgentGraphTokenUsage = Readonly<{
  inputTokens: number
  cachedInputTokens: number
  outputTokens: number
  reasoningTokens: number
  totalTokens: number
}>

export type LiveAgentGraphTokenAttribution = Readonly<{
  agentRef: string
  usageTruth: "exact" | "unreported"
  usage: LiveAgentGraphTokenUsage | null
}>

export type LiveAgentGraphTokenTruth = "exact" | "partial" | "unreported"

export type LiveAgentGraphPresentationRow = Readonly<{
  agentRef: string
  graphRef: string
  parentAgentRef: string | null
  depth: number
  label: string
  status: LiveAgentGraphStatus
  statusLabel: string
  tone: LiveAgentGraphTone
  providerLabel: string
  runtimeLabel: string
  sessionLabel: string
  worktreeLabel: string
  toolLabel: string | null
  elapsedLabel: string
  terminalLabel: string | null
  attentionLabel: string | null
  tokenTruth: LiveAgentGraphTokenTruth
  tokensLabel: string
  canControl: boolean
}>

export type LiveAgentGraphPresentation = Readonly<{
  authority: LiveAgentGraphAuthority
  authorityLabel: "Live" | "Historical import"
  graphRef: string
  rows: ReadonlyArray<LiveAgentGraphPresentationRow>
  totalCount: number
  hiddenCount: number
  activeCount: number
  attentionCount: number
  terminalCount: number
  updatedAt: string
}>

const terminalStatuses = new Set<LiveAgentGraphStatus>([
  "completed",
  "failed",
  "canceled",
  "interrupted",
])

const titleCase = (value: string): string =>
  value.split("_").map(part => part.length === 0
    ? part
    : `${part[0]!.toUpperCase()}${part.slice(1)}`).join(" ")

const compactRef = (value: string): string => {
  const tail = value.split(/[.:/]/).filter(Boolean).at(-1) ?? value
  return tail.length <= 10 ? tail : `…${tail.slice(-8)}`
}

const providerLabel = (node: LiveAgentGraphNode): string =>
  node.provider.state === "known"
    ? titleCase(node.provider.kind)
    : `Provider unavailable · ${titleCase(node.provider.reason)}`

const runtimeLabel = (node: LiveAgentGraphNode): string =>
  node.runtime.state === "known"
    ? titleCase(node.runtime.kind)
    : `Runtime unavailable · ${titleCase(node.runtime.reason)}`

const worktreeLabel = (node: LiveAgentGraphNode): string =>
  node.worktree.state === "known"
    ? `Worktree ${compactRef(node.worktree.worktreeRef)}`
    : `Worktree unavailable · ${titleCase(node.worktree.reason)}`

const statusTone = (node: LiveAgentGraphNode): LiveAgentGraphTone => {
  if (node.attention.state !== "none") return "attention"
  if (node.status === "running" || node.status === "queued") return "active"
  if (node.status === "completed") return "success"
  if (node.status === "failed" || node.status === "interrupted") return "danger"
  return "muted"
}

const elapsedLabel = (node: LiveAgentGraphNode, nowMs: number): string => {
  const start = Date.parse(node.startedAt ?? node.createdAt)
  const end = node.endedAt === null ? nowMs : Date.parse(node.endedAt)
  if (!Number.isFinite(start) || !Number.isFinite(end)) return "Elapsed unavailable"
  const seconds = Math.max(0, Math.floor((end - start) / 1_000))
  if (seconds < 60) return `${seconds}s elapsed`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}m ${seconds % 60}s elapsed`
  const hours = Math.floor(minutes / 60)
  return `${hours}h ${minutes % 60}m elapsed`
}

const attentionLabel = (node: LiveAgentGraphNode): string | null => {
  if (node.attention.state === "none") return null
  if (node.attention.state === "unknown") {
    return `Attention unavailable · ${titleCase(node.attention.reason)}`
  }
  return `${titleCase(node.attention.state)} needs attention`
}

const terminalLabel = (node: LiveAgentGraphNode): string | null =>
  node.terminal.state === "terminal"
    ? `Finished · ${titleCase(node.terminal.reason)}`
    : node.terminal.state === "unknown"
      ? `Terminal state unavailable · ${titleCase(node.terminal.reason)}`
      : null

const TOKENS_UNREPORTED_LABEL = "Unreported"

const formatCount = (value: number): string =>
  String(value).replace(/\B(?=(\d{3})+(?!\d))/g, ",")

const isExactUsage = (usage: LiveAgentGraphTokenUsage | null): usage is LiveAgentGraphTokenUsage =>
  usage !== null &&
  [
    usage.inputTokens,
    usage.cachedInputTokens,
    usage.outputTokens,
    usage.reasoningTokens,
    usage.totalTokens,
  ].every(value => Number.isSafeInteger(value) && value >= 0)

type TokenFacts = Readonly<{ tokenTruth: LiveAgentGraphTokenTruth; tokensLabel: string }>

const UNREPORTED_TOKEN_FACTS: TokenFacts = {
  tokenTruth: "unreported",
  tokensLabel: TOKENS_UNREPORTED_LABEL,
}

/**
 * Deterministic per-node token facts. A node is `exact` only when every
 * attribution recorded for it carries a complete well-formed usage split;
 * a mix of exact and unreported turns is presented as `partial` with the
 * exact remainder named; malformed exact claims are demoted to unreported.
 */
const tokenFactsByAgent = (
  attributions: ReadonlyArray<LiveAgentGraphTokenAttribution>,
): Map<string, TokenFacts> => {
  const grouped = new Map<string, { exact: LiveAgentGraphTokenUsage[]; unreported: number }>()
  for (const attribution of attributions) {
    const entry = grouped.get(attribution.agentRef) ?? { exact: [], unreported: 0 }
    if (attribution.usageTruth === "exact" && isExactUsage(attribution.usage)) {
      entry.exact.push(attribution.usage)
    } else {
      entry.unreported += 1
    }
    grouped.set(attribution.agentRef, entry)
  }
  const facts = new Map<string, TokenFacts>()
  for (const [agentRef, entry] of grouped) {
    if (entry.exact.length === 0) {
      facts.set(agentRef, UNREPORTED_TOKEN_FACTS)
      continue
    }
    const input = entry.exact.reduce((sum, usage) => sum + usage.inputTokens, 0)
    const output = entry.exact.reduce((sum, usage) => sum + usage.outputTokens, 0)
    const total = entry.exact.reduce((sum, usage) => sum + usage.totalTokens, 0)
    if (entry.unreported === 0) {
      facts.set(agentRef, {
        tokenTruth: "exact",
        tokensLabel: `${formatCount(input)} in · ${formatCount(output)} out · ${formatCount(total)} total · exact`,
      })
      continue
    }
    facts.set(agentRef, {
      tokenTruth: "partial",
      tokensLabel: `${formatCount(total)} total from ${entry.exact.length} exact turn${entry.exact.length === 1 ? "" : "s"} · ${entry.unreported} unreported`,
    })
  }
  return facts
}

const toolLabel = (node: LiveAgentGraphNode): string | null =>
  node.currentTool.state === "known"
    ? `${node.currentTool.toolName} · ${titleCase(node.currentTool.status)}`
    : node.currentTool.state === "unknown"
      ? `Action unavailable · ${titleCase(node.currentTool.reason)}`
      : null

const statusRank = (node: LiveAgentGraphNode): number => {
  if (node.attention.state !== "none") return 0
  if (node.status === "running") return 1
  if (node.status === "waiting_for_input") return 2
  if (node.status === "queued") return 3
  if (node.status === "failed" || node.status === "interrupted") return 4
  if (node.status === "completed") return 5
  return 6
}

const compareNodes = (left: LiveAgentGraphNode, right: LiveAgentGraphNode): number =>
  statusRank(left) - statusRank(right) ||
  left.createdAt.localeCompare(right.createdAt) ||
  left.agentRef.localeCompare(right.agentRef)

const rowFor = (
  graph: LiveAgentGraphEntity,
  node: LiveAgentGraphNode,
  authority: LiveAgentGraphAuthority,
  depth: number,
  siblingIndex: number,
  nowMs: number,
  tokenFacts: Map<string, TokenFacts>,
): LiveAgentGraphPresentationRow => {
  const relation = node.parent.kind === "root" ? "root" : "subagent"
  const tokens = tokenFacts.get(node.agentRef) ?? UNREPORTED_TOKEN_FACTS
  return {
    agentRef: node.agentRef,
    graphRef: graph.graphRef,
    parentAgentRef: node.parent.kind === "agent" ? node.parent.agentRef : null,
    depth,
    label: `${providerLabel(node)} ${relation}${relation === "subagent" ? ` ${siblingIndex + 1}` : ""} · ${compactRef(node.agentRef)}`,
    status: node.status,
    statusLabel: titleCase(node.status),
    tone: statusTone(node),
    providerLabel: providerLabel(node),
    runtimeLabel: runtimeLabel(node),
    sessionLabel: `Session ${compactRef(node.sessionRef)}`,
    worktreeLabel: worktreeLabel(node),
    toolLabel: toolLabel(node),
    elapsedLabel: elapsedLabel(node, nowMs),
    terminalLabel: terminalLabel(node),
    attentionLabel: attentionLabel(node),
    tokenTruth: tokens.tokenTruth,
    tokensLabel: tokens.tokensLabel,
    canControl: authority === "live" && !terminalStatuses.has(node.status),
  }
}

export const projectLiveAgentGraphPresentation = (
  graph: LiveAgentGraphEntity,
  options: Readonly<{
    authority?: LiveAgentGraphAuthority
    maxRows?: number
    nowMs?: number
    tokenAttributions?: ReadonlyArray<LiveAgentGraphTokenAttribution>
  }> = {},
): LiveAgentGraphPresentation => {
  const authority = options.authority ?? "live"
  const maxRows = Math.max(1, Math.min(2_000, Math.trunc(options.maxRows ?? 200)))
  const nowMs = options.nowMs ?? Date.now()
  const tokenFacts = tokenFactsByAgent(options.tokenAttributions ?? [])
  const byParent = new Map<string, LiveAgentGraphNode[]>()
  const roots: LiveAgentGraphNode[] = []
  for (const node of graph.nodes) {
    if (node.parent.kind !== "agent") {
      roots.push(node)
      continue
    }
    const children = byParent.get(node.parent.agentRef) ?? []
    children.push(node)
    byParent.set(node.parent.agentRef, children)
  }
  roots.sort(compareNodes)
  for (const children of byParent.values()) children.sort(compareNodes)

  const rows: LiveAgentGraphPresentationRow[] = []
  const visit = (node: LiveAgentGraphNode, depth: number, siblingIndex: number): void => {
    if (rows.length >= maxRows) return
    rows.push(rowFor(graph, node, authority, depth, siblingIndex, nowMs, tokenFacts))
    const children = byParent.get(node.agentRef) ?? []
    children.forEach((child, index) => visit(child, depth + 1, index))
  }
  roots.forEach((root, index) => visit(root, 0, index))

  return {
    authority,
    authorityLabel: authority === "live" ? "Live" : "Historical import",
    graphRef: graph.graphRef,
    rows,
    totalCount: graph.nodes.length,
    hiddenCount: Math.max(0, graph.nodes.length - rows.length),
    activeCount: graph.nodes.filter(node => !terminalStatuses.has(node.status)).length,
    attentionCount: graph.nodes.filter(node => node.attention.state !== "none").length,
    terminalCount: graph.nodes.filter(node => terminalStatuses.has(node.status)).length,
    updatedAt: graph.updatedAt,
  }
}

export const resolveLiveAgentGraphSelection = (
  presentation: LiveAgentGraphPresentation,
  requestedAgentRef: string | null,
): string | null => {
  if (requestedAgentRef !== null && presentation.rows.some(row => row.agentRef === requestedAgentRef)) {
    return requestedAgentRef
  }
  return presentation.rows[0]?.agentRef ?? null
}

export const newestLiveAgentGraph = (
  graphs: ReadonlyArray<LiveAgentGraphEntity>,
): LiveAgentGraphEntity | null =>
  [...graphs].sort((left, right) =>
    right.attachmentGeneration - left.attachmentGeneration ||
    right.cursor - left.cursor ||
    right.updatedAt.localeCompare(left.updatedAt) ||
    right.graphRef.localeCompare(left.graphRef))[0] ?? null

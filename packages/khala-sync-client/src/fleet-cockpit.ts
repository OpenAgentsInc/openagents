import type { ConfirmedAgentRun } from "./agent-timeline.js"
import type { ConfirmedRuntimeInteraction } from "./runtime-interactions.js"

export type FleetAuthority = "live" | "offline" | "stale" | "revoked" | "unknown"
export type FleetRunAction = "pause" | "cancel" | "resume" | "retry" | "close"
export type FleetAttentionAction = "approve" | "deny"

export type FleetCockpitSource = Readonly<{
  threadRef: string
  title: string
  authority: FleetAuthority
  run: ConfirmedAgentRun
  interactions: ReadonlyArray<ConfirmedRuntimeInteraction>
  agentRefs: ReadonlyArray<string>
  repositoryRef?: string
  receiptRefs: ReadonlyArray<string>
}>

export type FleetCockpitCard = Readonly<{
  threadRef: string
  title: string
  authority: FleetAuthority
  runRef: string
  runVersion: number
  status: ConfirmedAgentRun["status"]
  provider: "codex" | "claude" | "openagents" | "unknown"
  workContextRef: string | null
  repositoryRef: string | null
  agentRefs: ReadonlyArray<string>
  receiptRefs: ReadonlyArray<string>
  attention: ReadonlyArray<{
    interactionRef: string
    turnRef: string
    version: number
    kind: ConfirmedRuntimeInteraction["kind"]
    title: string
    actions: ReadonlyArray<FleetAttentionAction>
  }>
  actions: ReadonlyArray<FleetRunAction>
}>

const provider = (runtime: ConfirmedAgentRun["runtime"] | undefined): FleetCockpitCard["provider"] =>
  runtime === "codex" || runtime === "opencode_codex" ? "codex"
    : runtime === "claude_code" ? "claude"
      : runtime === "openagents_native" ? "openagents" : "unknown"

export const fleetRunActions = (
  authority: FleetAuthority,
  status: ConfirmedAgentRun["status"],
): ReadonlyArray<FleetRunAction> => {
  if (authority !== "live") return []
  if (status === "queued" || status === "running" || status === "waiting_for_input") {
    return status === "running" ? ["pause", "cancel"] : ["cancel"]
  }
  if (status === "canceled") return ["resume", "retry", "close"]
  return ["retry", "close"]
}

export const projectFleetCockpitCard = (source: FleetCockpitSource): FleetCockpitCard => ({
  threadRef: source.threadRef,
  title: source.title.slice(0, 160),
  authority: source.authority,
  runRef: source.run.runRef,
  runVersion: source.run.version,
  status: source.run.status,
  provider: provider(source.run.runtime),
  workContextRef: source.run.workContextRef ?? null,
  repositoryRef: source.repositoryRef ?? null,
  agentRefs: [...new Set(source.agentRefs)].slice(0, 100),
  receiptRefs: [...new Set(source.receiptRefs)].slice(0, 100),
  attention: source.interactions
    .filter(interaction => interaction.status === "pending")
    .map(interaction => ({
      interactionRef: interaction.interactionRef,
      turnRef: interaction.turnId,
      version: interaction.version,
      kind: interaction.kind,
      title: interaction.displayTitle.slice(0, 120),
      actions: interaction.kind === "tool_approval" ? ["approve", "deny"] as const : [],
    })),
  actions: fleetRunActions(source.authority, source.run.status),
})

export type FleetRunCommand = Readonly<{
  action: FleetRunAction
  threadRef: string
  runRef: string
  expectedVersion: number
}>
export type FleetAttentionCommand = Readonly<{
  action: FleetAttentionAction
  threadRef: string
  runRef: string
  interactionRef: string
  expectedRunVersion: number
  expectedInteractionVersion: number
}>

export const admitFleetRunCommand = (
  card: FleetCockpitCard,
  action: FleetRunAction,
): FleetRunCommand | null =>
  card.authority === "live" && card.actions.includes(action)
    ? { action, threadRef: card.threadRef, runRef: card.runRef, expectedVersion: card.runVersion }
    : null

export const admitFleetAttentionCommand = (
  card: FleetCockpitCard,
  interactionRef: string,
  action: FleetAttentionAction,
): FleetAttentionCommand | null => {
  if (card.authority !== "live") return null
  const attention = card.attention.find(item => item.interactionRef === interactionRef)
  return attention !== undefined && attention.actions.includes(action)
    ? {
        action,
        threadRef: card.threadRef,
        runRef: card.runRef,
        interactionRef,
        expectedRunVersion: card.runVersion,
        expectedInteractionVersion: attention.version,
      }
    : null
}

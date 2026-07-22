/**
 * META-1 (#9180): the one-agent identity front door.
 *
 * The main thing the user talks with in the Desktop UI is ONE named
 * persistent agent — "OpenAgents" — fronting the existing chat, router, and
 * Full Auto machinery. This module is presentation + continuity ONLY: it
 * names the identity, derives the honest per-response lane/model attribution
 * from metadata the host already stamps (`DesktopMessageMeta.lane`/`model`,
 * #8712/#9081), and projects the Full Auto runs bound to the active
 * conversation so delegated work appears ATTRIBUTED inside that conversation
 * with a link to the existing read-only run view. It grants no authority:
 * routing stays with the deterministic fail-closed gates, dispatch stays with
 * the existing lane hosts, and every affordance here resolves to an intent
 * that already exists (`DesktopFullAutoRunOpened`).
 *
 * The name is not new copy: the routing-disclosure card (#9127) already
 * speaks as "OpenAgents routed to <subagent>", and the Boot Sequence banner
 * already reads "Initializing OpenAgents". META-1 promotes that existing
 * in-product identity to the conversation's front door instead of inventing
 * a persona.
 */
import type { DesktopMessageMeta } from "../chat-contract.ts"
import type { FullAutoRunProjection } from "../full-auto-run-ipc-contract.ts"
import { fullAutoRunStatusLabel } from "./full-auto-workspace.ts"

/** The one persistent agent identity the Desktop conversation fronts. */
export const DESKTOP_AGENT_NAME = "OpenAgents"

/**
 * Honest inline attribution for a completed agent response: which lane
 * (harness) and effective model produced it, from the host-stamped message
 * metadata. Lane refs are shown verbatim ("codex-local", "claude-local",
 * "acp:grok-cli") — a neutral machine label, never a claim the meta-agent
 * did the work itself. Returns undefined when the host recorded no lane, so
 * the projection never invents an attribution it cannot cite.
 */
export const agentLaneAttribution = (
  meta: Pick<DesktopMessageMeta, "lane" | "model"> | undefined,
): string | undefined => {
  const lane = meta?.lane?.trim() ?? ""
  if (lane === "") return undefined
  const model = meta?.model?.trim() ?? ""
  return model === "" ? `via ${lane}` : `via ${lane} · ${model}`
}

/**
 * A Full Auto run bound to the active conversation, projected as a linked
 * run card datum. Pure filter over the run-list projection the dedicated
 * Full Auto surface already consumes — same refs, same status vocabulary.
 */
export type AgentConversationRunLink = Readonly<{
  runRef: string
  title: string
  statusLabel: string
  lane: string | null
}>

export const projectAgentConversationRunLinks = (
  runs: ReadonlyArray<FullAutoRunProjection>,
  activeThreadId: string | null,
): ReadonlyArray<AgentConversationRunLink> =>
  activeThreadId === null
    ? []
    : runs
        .filter(run => run.threadRef === activeThreadId)
        .map(run => ({
          runRef: run.runRef,
          title: run.title,
          statusLabel: fullAutoRunStatusLabel(run),
          lane: run.lane,
        }))

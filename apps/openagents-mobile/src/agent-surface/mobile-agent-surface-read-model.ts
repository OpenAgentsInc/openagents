/**
 * AFS-12 mobile agent-surface read model.
 *
 * Mobile is a remote-controller and projection surface. It REUSES the shared safe
 * projections from `@openagentsinc/agent-surface` to decode the SAME frozen AFS-00
 * fixtures Desktop decodes, then composes a bounded agent graph and a safe message
 * drilldown. It executes NO agent, dispatches NO provider, hosts NO Apple FM
 * helper, and holds NO action authority: it only decodes safe bytes and reads
 * facts. It imports only the portable schema and projection subpaths — never the
 * Desktop host, the Apple FM Node adapter, a Node store, or a provider SDK.
 */
import type { SafeMessageChainEntry } from "@openagentsinc/agent-runtime-schema"
import {
  isLiveCardState,
  summarizeSurfaceFacts,
  surfaceFactsAreSecretFree,
  type SafeSurfaceScenarioFacts,
  type SurfaceFactSummary,
} from "@openagentsinc/agent-surface"
import { readAfsBaselineSurfaceFacts } from "@openagentsinc/agent-surface/afs-baseline-surface-corpus"

/** A bounded, read-only agent-graph node the mobile surface renders. */
export interface MobileAgentGraphNode {
  readonly scenario: string
  readonly requestRef: string
  readonly threadRef: string
  readonly cardState: SafeSurfaceScenarioFacts["card"]["cardState"]
  readonly live: boolean
  readonly provider: SafeSurfaceScenarioFacts["card"]["provider"]
  readonly localOnly: boolean
  readonly messageCount: number
  readonly refusalReason: string | null
}

/** Decode the shared AFS-00 baseline corpus into equivalent safe facts on mobile. */
export const readMobileAgentSurfaceScenarios = (): ReadonlyArray<SafeSurfaceScenarioFacts> =>
  readAfsBaselineSurfaceFacts()

/** The compact cross-surface fact summary the mobile surface produces. */
export const readMobileAgentSurfaceFactSummary = (): ReadonlyArray<SurfaceFactSummary> =>
  readMobileAgentSurfaceScenarios().map(summarizeSurfaceFacts)

/** Compose the bounded read-only agent-graph nodes the mobile surface shows. */
export const readMobileAgentGraph = (): ReadonlyArray<MobileAgentGraphNode> =>
  readMobileAgentSurfaceScenarios().map((facts) => ({
    scenario: facts.scenario,
    requestRef: facts.card.requestRef,
    threadRef: facts.card.threadRef,
    cardState: facts.card.cardState,
    live: isLiveCardState(facts.card.cardState),
    provider: facts.card.provider,
    localOnly: facts.card.localOnly,
    messageCount: facts.card.messageCount,
    refusalReason: facts.recovery.refusalReason,
  }))

/** The bounded safe message drilldown for one decoded scenario, by request ref. */
export const readMobileMessageDrilldown = (requestRef: string): ReadonlyArray<SafeMessageChainEntry> =>
  readMobileAgentSurfaceScenarios().find((facts) => facts.card.requestRef === requestRef)?.messageChain ?? []

/** True when every decoded mobile fact is secret-free (the mobile privacy-fence oracle). */
export const mobileAgentSurfaceFactsAreSecretFree = (): boolean =>
  readMobileAgentSurfaceScenarios().every((facts) => surfaceFactsAreSecretFree(facts))

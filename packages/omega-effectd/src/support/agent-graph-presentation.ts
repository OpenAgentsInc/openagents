/**
 * App boundary for the provider-neutral confirmed graph presentation model.
 * Renderer modules import this sibling surface so package ownership remains
 * outside the Effect Native renderer boundary.
 */
export {
  newestLiveAgentGraph,
  projectLiveAgentGraphPresentation,
  resolveLiveAgentGraphSelection,
} from "@openagentsinc/khala-sync-client"

export type {
  LiveAgentGraphPresentation,
  LiveAgentGraphPresentationRow,
  LiveAgentGraphTokenAttribution,
  LiveAgentGraphTokenTruth,
  LiveAgentGraphTokenUsage,
  LiveAgentGraphTone,
} from "@openagentsinc/khala-sync-client"

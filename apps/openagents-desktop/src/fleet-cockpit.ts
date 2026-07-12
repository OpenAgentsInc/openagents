/**
 * Main-owned facade for the shared authoritative Fleet contract.
 *
 * Renderer modules remain constrained to Effect Native and Desktop sibling
 * seams; this perimeter is the single package import and exports no host
 * authority.
 */
export {
  admitFleetRunCommand,
  admitFleetAttentionCommand,
  projectFleetCockpitCard,
  type FleetAuthority,
  type FleetCockpitCard,
  type FleetAttentionAction,
  type FleetAttentionCommand,
  type FleetRunAction,
  type FleetRunCommand,
} from "@openagentsinc/khala-sync-client"

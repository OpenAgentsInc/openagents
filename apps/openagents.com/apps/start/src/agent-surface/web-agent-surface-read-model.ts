/**
 * AFS-12 web agent-surface read model.
 *
 * The web app is a local-browser and remote-supervision surface. It REUSES the
 * shared safe projections from `@openagentsinc/agent-surface` to decode the SAME
 * frozen AFS-00 fixtures Desktop decodes, then composes a bounded, read-only
 * supervision view. It has NO turn execution, NO provider dispatch, and NO action
 * authority: it only decodes safe bytes and reads facts. It imports only the
 * portable schema and projection subpaths — never the Desktop host, the Apple FM
 * Node adapter, a Node store, or a provider SDK.
 */
import {
  isLiveCardState,
  summarizeSurfaceFacts,
  surfaceFactsAreSecretFree,
  type SafeSurfaceScenarioFacts,
  type SurfaceFactSummary,
} from "@openagentsinc/agent-surface";
import { readAfsBaselineSurfaceFacts } from "@openagentsinc/agent-surface/afs-baseline-surface-corpus";

/** A bounded, read-only supervision row the web pane renders for one turn/agent card. */
export interface WebSupervisionRow {
  readonly scenario: string;
  readonly requestRef: string;
  readonly threadRef: string;
  readonly providerTurnRef: string | null;
  readonly cardState: SafeSurfaceScenarioFacts["card"]["cardState"];
  readonly live: boolean;
  readonly provider: SafeSurfaceScenarioFacts["card"]["provider"];
  readonly dataDestination: SafeSurfaceScenarioFacts["card"]["dataDestination"];
  readonly localOnly: boolean;
  readonly usageTruth: SafeSurfaceScenarioFacts["card"]["usageTruth"];
  readonly messageCount: number;
  readonly routeOutcome: "admitted" | "closed" | null;
  readonly refusalReason: string | null;
}

/** Decode the shared AFS-00 baseline corpus into equivalent safe facts on web. */
export const readWebAgentSurfaceScenarios = (): ReadonlyArray<SafeSurfaceScenarioFacts> =>
  readAfsBaselineSurfaceFacts();

/** The compact cross-surface fact summary the web surface produces. */
export const readWebAgentSurfaceFactSummary = (): ReadonlyArray<SurfaceFactSummary> =>
  readWebAgentSurfaceScenarios().map(summarizeSurfaceFacts);

/** Compose the bounded read-only supervision rows the web pane renders. */
export const readWebSupervisionRows = (): ReadonlyArray<WebSupervisionRow> =>
  readWebAgentSurfaceScenarios().map((facts) => ({
    scenario: facts.scenario,
    requestRef: facts.card.requestRef,
    threadRef: facts.card.threadRef,
    providerTurnRef: facts.card.providerTurnRef,
    cardState: facts.card.cardState,
    live: isLiveCardState(facts.card.cardState),
    provider: facts.card.provider,
    dataDestination: facts.card.dataDestination,
    localOnly: facts.card.localOnly,
    usageTruth: facts.card.usageTruth,
    messageCount: facts.card.messageCount,
    routeOutcome: facts.route?.outcome ?? null,
    refusalReason: facts.recovery.refusalReason,
  }));

/** True when every decoded web fact is secret-free (the web privacy-fence oracle). */
export const webAgentSurfaceFactsAreSecretFree = (): boolean =>
  readWebAgentSurfaceScenarios().every((facts) => surfaceFactsAreSecretFree(facts));

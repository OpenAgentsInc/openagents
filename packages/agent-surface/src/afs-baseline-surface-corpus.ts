/**
 * AFS-12 cross-surface baseline corpus.
 *
 * This module binds the frozen AFS-00 safe fixtures (from
 * `@openagentsinc/agent-runtime-schema/afs-baseline-fixtures`) into one ordered
 * scenario corpus, then decodes it with the shared read/compose reader. Desktop,
 * web, and mobile all decode this SAME corpus to equivalent facts. The corpus is
 * safe bytes only: it carries no helper secret, raw argument, raw output, local
 * path, or token, so a read surface never gains execution authority by consuming
 * it.
 *
 * The corpus lives on a subpath, not the package root, so the public projector
 * entry stays free of fixture data.
 */
import {
  explicitProviderProjectionFixture,
  explicitProviderRouteDecisionFixture,
  helperFailureProjectionFixture,
  helperFailureRefusalReason,
  localAnswerProjectionFixture,
  localAnswerRouteDecisionFixture,
  malformedOutputProjectionFixture,
  malformedOutputRefusalReason,
  standbyProjectionFixture,
  unavailableProviderProjectionFixture,
  unavailableProviderRefusalReason,
  unavailableProviderRouteDecisionFixture,
} from "@openagentsinc/agent-runtime-schema/afs-baseline-fixtures";

import {
  readSafeSurfaceScenario,
  summarizeSurfaceFacts,
  type SafeSurfaceScenarioFacts,
  type SafeSurfaceScenarioInput,
  type SurfaceFactSummary,
} from "./index.js";

/** The ordered AFS-00 baseline scenarios as safe cross-surface byte inputs. */
export const afsBaselineSurfaceScenarios: ReadonlyArray<SafeSurfaceScenarioInput> = [
  {
    scenario: "local_answer",
    projection: localAnswerProjectionFixture,
    routeDecision: localAnswerRouteDecisionFixture,
  },
  {
    scenario: "standby",
    projection: standbyProjectionFixture,
  },
  {
    scenario: "explicit_provider",
    projection: explicitProviderProjectionFixture,
    routeDecision: explicitProviderRouteDecisionFixture,
  },
  {
    scenario: "malformed_output",
    projection: malformedOutputProjectionFixture,
    refusalReason: malformedOutputRefusalReason,
  },
  {
    scenario: "helper_failure",
    projection: helperFailureProjectionFixture,
    refusalReason: helperFailureRefusalReason,
  },
  {
    scenario: "unavailable_provider",
    projection: unavailableProviderProjectionFixture,
    routeDecision: unavailableProviderRouteDecisionFixture,
    refusalReason: unavailableProviderRefusalReason,
  },
];

/** Decode the whole baseline corpus with the shared reader. */
export const readAfsBaselineSurfaceFacts = (): ReadonlyArray<SafeSurfaceScenarioFacts> =>
  afsBaselineSurfaceScenarios.map(readSafeSurfaceScenario);

/** Summarize the decoded baseline corpus with the shared compact reducer. */
export const readAfsBaselineSurfaceFactSummary = (): ReadonlyArray<SurfaceFactSummary> =>
  readAfsBaselineSurfaceFacts().map(summarizeSurfaceFacts);

/**
 * The canonical, hand-authored cross-surface fact anchor. Every surface — Desktop,
 * web, and mobile — must decode `afsBaselineSurfaceScenarios` to exactly these
 * summaries. It is the equivalence oracle: if any surface decodes to different
 * facts, its test fails against this shared constant.
 */
export const afsBaselineSurfaceFactSummary: ReadonlyArray<SurfaceFactSummary> = [
  {
    scenario: "local_answer",
    cardState: "done",
    terminal: true,
    refusalReason: null,
    messageCount: 2,
    provider: "apple_fm",
    dataDestination: "on_device_local",
    localOnly: true,
    usageTruth: "estimated",
    routeOutcome: "admitted",
    routeSelected: "provider.apple_fm.1",
    routeDataDestination: "on_device_local",
    routeLocalOnly: true,
    contextManifestRef: "context.local.1",
  },
  {
    scenario: "standby",
    cardState: "queued",
    terminal: false,
    refusalReason: null,
    messageCount: 0,
    provider: "apple_fm",
    dataDestination: "on_device_local",
    localOnly: true,
    usageTruth: "unknown",
    routeOutcome: null,
    routeSelected: null,
    routeDataDestination: null,
    routeLocalOnly: null,
    contextManifestRef: null,
  },
  {
    scenario: "explicit_provider",
    cardState: "done",
    terminal: true,
    refusalReason: null,
    messageCount: 1,
    provider: "codex",
    dataDestination: "remote_provider",
    localOnly: false,
    usageTruth: "exact",
    routeOutcome: "admitted",
    routeSelected: "provider.codex.1",
    routeDataDestination: "remote_provider",
    routeLocalOnly: false,
    contextManifestRef: "context.explicit.1",
  },
  {
    scenario: "malformed_output",
    cardState: "refused",
    terminal: true,
    refusalReason: "malformed_output",
    messageCount: 0,
    provider: "apple_fm",
    dataDestination: "on_device_local",
    localOnly: true,
    usageTruth: "unknown",
    routeOutcome: null,
    routeSelected: null,
    routeDataDestination: null,
    routeLocalOnly: null,
    contextManifestRef: null,
  },
  {
    scenario: "helper_failure",
    cardState: "failed",
    terminal: true,
    refusalReason: "helper_missing",
    messageCount: 0,
    provider: "apple_fm",
    dataDestination: "on_device_local",
    localOnly: true,
    usageTruth: "unknown",
    routeOutcome: null,
    routeSelected: null,
    routeDataDestination: null,
    routeLocalOnly: null,
    contextManifestRef: null,
  },
  {
    scenario: "unavailable_provider",
    cardState: "refused",
    terminal: true,
    refusalReason: "route_closed_no_candidate",
    messageCount: 0,
    provider: null,
    dataDestination: "on_device_local",
    localOnly: true,
    usageTruth: "unknown",
    routeOutcome: "closed",
    routeSelected: null,
    routeDataDestination: null,
    routeLocalOnly: null,
    contextManifestRef: "context.unavailable.1",
  },
];

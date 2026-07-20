/**
 * AFS-00 baseline fixtures.
 *
 * These fixtures freeze the current behavior of the Apple FM chat path and the
 * explicit provider path as decodable facts. Each scenario is expressed with the
 * AFS-00 frozen schemas. The safe turn and projection fixtures are decoded by
 * the Desktop, web, and mobile decoders to equivalent facts; the shared schema
 * is the one decoder all three surfaces use.
 *
 * Scenarios: local answer, standby, explicit provider turn, malformed Apple FM
 * output, helper failure, and unavailable provider.
 */
const AT = "2026-07-20T08:00:00Z" as const;

/** Scenario 1: a local Apple FM answer. It selects the local lane; it does not delegate. */
export const localAnswerRouteDecisionFixture = {
  schema: "openagents.agent_route_decision.v1" as const,
  outcome: "admitted" as const,
  routeDecisionRef: "route.local.1",
  requestRef: "request.local.1",
  selected: "provider.apple_fm.1",
  effective: "provider.apple_fm.1",
  admittedCandidateSet: ["provider.apple_fm.1"],
  policyArtifactRef: "artifact.policy.1",
  contextManifestRef: "context.local.1",
  disclosure: {
    dataDestination: "on_device_local" as const,
    costClass: "local_resource_only" as const,
    localOnly: true,
    providerRef: "provider.apple_fm.1",
  },
  decisionReason: "admitted_first_candidate" as const,
  dispositions: [],
  decidedAt: AT,
};

export const localAnswerProjectionFixture = {
  schema: "openagents.agent_turn_projection.v1" as const,
  threadRef: "thread.1",
  requestRef: "request.local.1",
  cardState: "done" as const,
  candidate: "apple_fm" as const,
  dataDestination: "on_device_local" as const,
  usageTruth: "estimated" as const,
  localOnly: true,
  updatedAt: AT,
  messageChain: [
    { entryRef: "entry.local.user.1", role: "user" as const, text: "hi" },
    { entryRef: "entry.local.assistant.1", role: "assistant" as const, text: "local answer" },
  ],
  evidenceRefs: ["evidence.local.1"],
};

/** Scenario 2: standby. Apple FM is still booting; the card is queued. */
export const standbyProjectionFixture = {
  schema: "openagents.agent_turn_projection.v1" as const,
  threadRef: "thread.1",
  requestRef: "request.standby.1",
  cardState: "queued" as const,
  candidate: "apple_fm" as const,
  dataDestination: "on_device_local" as const,
  usageTruth: "unknown" as const,
  localOnly: true,
  updatedAt: AT,
  messageChain: [],
  evidenceRefs: [],
};

/** Scenario 3: an explicit provider turn. It selects a remote lane and dispatches. */
export const explicitProviderRouteDecisionFixture = {
  schema: "openagents.agent_route_decision.v1" as const,
  outcome: "admitted" as const,
  routeDecisionRef: "route.explicit.1",
  requestRef: "request.explicit.1",
  selected: "provider.codex.1",
  effective: "provider.codex.1",
  admittedCandidateSet: ["provider.codex.1", "provider.apple_fm.1"],
  policyArtifactRef: "artifact.policy.1",
  contextManifestRef: "context.explicit.1",
  disclosure: {
    dataDestination: "remote_provider" as const,
    costClass: "metered_provider_tokens" as const,
    localOnly: false,
    providerRef: "provider.codex.1",
  },
  decisionReason: "forced_explicit_provider" as const,
  dispositions: [],
  decidedAt: AT,
};

export const explicitProviderProjectionFixture = {
  schema: "openagents.agent_turn_projection.v1" as const,
  threadRef: "thread.1",
  requestRef: "request.explicit.1",
  providerTurnRef: "providerturn.codex.1",
  cardState: "done" as const,
  candidate: "codex" as const,
  dataDestination: "remote_provider" as const,
  usageTruth: "exact" as const,
  localOnly: false,
  updatedAt: AT,
  messageChain: [{ entryRef: "entry.explicit.assistant.1", role: "assistant" as const, text: "dispatched." }],
  evidenceRefs: ["evidence.explicit.1"],
};

/** Scenario 4: malformed Apple FM output. It refuses; it never dispatches. */
export const malformedOutputReceiptFixture = {
  schema: "openagents.agent_turn_receipt.v1" as const,
  requestRef: "request.malformed.1",
  routeDecisionRef: "route.malformed.1",
  decision: "failed" as const,
  usageTruth: "unknown" as const,
  evidenceRefs: ["evidence.malformed.1"],
};

export const malformedOutputProjectionFixture = {
  schema: "openagents.agent_turn_projection.v1" as const,
  threadRef: "thread.1",
  requestRef: "request.malformed.1",
  cardState: "refused" as const,
  candidate: "apple_fm" as const,
  dataDestination: "on_device_local" as const,
  usageTruth: "unknown" as const,
  localOnly: true,
  updatedAt: AT,
  messageChain: [],
  evidenceRefs: ["evidence.malformed.1"],
};

/** The malformed-output refusal reason, from the frozen refusal vocabulary. */
export const malformedOutputRefusalReason = "malformed_output" as const;

/** Scenario 5: helper failure. The helper is missing or unreachable; the turn fails. */
export const helperFailureProjectionFixture = {
  schema: "openagents.agent_turn_projection.v1" as const,
  threadRef: "thread.1",
  requestRef: "request.helper.1",
  cardState: "failed" as const,
  candidate: "apple_fm" as const,
  dataDestination: "on_device_local" as const,
  usageTruth: "unknown" as const,
  localOnly: true,
  updatedAt: AT,
  messageChain: [],
  evidenceRefs: ["evidence.helper.1"],
};

export const helperFailureRefusalReason = "helper_missing" as const;

/** Scenario 6: unavailable provider. The route closes fail-closed; nothing dispatches. */
export const unavailableProviderRouteDecisionFixture = {
  schema: "openagents.agent_route_decision.v1" as const,
  outcome: "closed" as const,
  routeDecisionRef: "route.unavailable.1",
  requestRef: "request.unavailable.1",
  policyArtifactRef: "artifact.policy.1",
  contextManifestRef: "context.unavailable.1",
  decisionReason: "no_candidate_fail_closed" as const,
  dispositions: [
    {
      providerRef: "provider.codex.1",
      candidate: "codex" as const,
      disposition: "refused" as const,
      reason: "account_not_ready" as const,
    },
  ],
  decidedAt: AT,
};

export const unavailableProviderProjectionFixture = {
  schema: "openagents.agent_turn_projection.v1" as const,
  threadRef: "thread.1",
  requestRef: "request.unavailable.1",
  cardState: "refused" as const,
  dataDestination: "on_device_local" as const,
  usageTruth: "unknown" as const,
  localOnly: true,
  updatedAt: AT,
  messageChain: [],
  evidenceRefs: [],
};

export const unavailableProviderRefusalReason = "route_closed_no_candidate" as const;

/** Every safe projection fixture the three surface decoders must decode alike. */
export const afsBaselineSafeProjectionFixtures = [
  localAnswerProjectionFixture,
  standbyProjectionFixture,
  explicitProviderProjectionFixture,
  malformedOutputProjectionFixture,
  helperFailureProjectionFixture,
  unavailableProviderProjectionFixture,
] as const;

/** A recommendation fixture: advisory only, never an admitted decision. */
export const routeRecommendationFixture = {
  schema: "openagents.agent_route_recommendation.v1" as const,
  candidate: "codex" as const,
  taskClass: "delegate" as const,
  reasonCode: "needs_delegation" as const,
  confidence: 0.82,
};

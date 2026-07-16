import { STABLE_METHOD_MANIFEST } from "@openagentsinc/agent-client-protocol/stable";

import { STABLE_CONFORMANCE_CASES, assertStableManifestCoverage, stableCaseKey } from "./cases.ts";
import {
  CONTENT_BLOCK_FIXTURES,
  SESSION_UPDATE_FIXTURES,
  STOP_REASONS,
  TOOL_KINDS,
  TOOL_STATUSES,
} from "./variants.ts";

export const FAULT_CASES = [
  ["parser", "malformed-frame"],
  ["parser", "oversized-frame"],
  ["protocol", "unknown-method"],
  ["protocol", "invalid-params"],
  ["protocol", "duplicate-response"],
  ["protocol", "late-response"],
  ["authority", "reverse-timeout"],
  ["authority", "reverse-refusal"],
  ["authority", "capability-lie"],
  ["authority", "auth-omission"],
  ["authority", "auth-failure"],
  ["projection", "partial-output"],
  ["projection", "update-after-completion"],
  ["lifecycle", "exit-startup"],
  ["lifecycle", "exit-initialize"],
  ["lifecycle", "exit-authenticate"],
  ["lifecycle", "exit-session"],
  ["lifecycle", "exit-prompt"],
  ["lifecycle", "exit-drain"],
  ["transport", "slow-consumer"],
  ["transport", "queue-overload"],
  ["recovery", "cancellation-race"],
  ["recovery", "replay-live-interleaving"],
  ["recovery", "restart-generation-crossing"],
] as const;

export const buildCoverageReport = () => {
  const coverage = assertStableManifestCoverage();
  return {
    format: "openagents-acp-coverage-v1",
    openagentsRevision: "issue-8890-fixture-baseline",
    schemaRelease: STABLE_METHOD_MANIFEST.schemaRelease,
    sourceSha256: STABLE_METHOD_MANIFEST.sourceSha256,
    ...coverage,
    methods: STABLE_CONFORMANCE_CASES.map((value) => ({
      key: stableCaseKey(value),
      kind: value.kind,
      supportState: value.supportState,
      capabilityState: value.capabilityState,
    })),
    variants: {
      contentBlocks: CONTENT_BLOCK_FIXTURES.map((value) => value.type),
      sessionUpdates: SESSION_UPDATE_FIXTURES.map((value) => value.sessionUpdate),
      stopReasons: STOP_REASONS,
      toolKinds: TOOL_KINDS,
      toolStatuses: TOOL_STATUSES,
      unknownFutureVariant: "explicit-private-native-failure",
    },
  };
};

export const buildCompatibilityMatrix = () => ({
  format: "openagents-acp-compatibility-matrix-v1",
  openagentsRevision: "issue-8890-fixture-baseline",
  protocol: "Agent Client Protocol",
  schemaRelease: STABLE_METHOD_MANIFEST.schemaRelease,
  schemaSha256: STABLE_METHOD_MANIFEST.sourceSha256,
  platform: "hermetic-node-subprocess",
  rows: STABLE_CONFORMANCE_CASES.map((value) => ({
    peer: "scripted-peer",
    peerVersion: "1",
    authMode: value.method === "authenticate" ? "cached_token-fixture" : "not-applicable",
    capability: STABLE_METHOD_MANIFEST.members.find(
      (member) => stableCaseKey(member) === stableCaseKey(value),
    )?.requiredCapability,
    method: value.method,
    direction: value.direction,
    proof: "fixture",
    result: "declared-for-execution",
  })),
  namedPeerProfiles: [
    {
      peer: "grok",
      version: "source-c68e39f",
      proof: "source-audited-synthetic",
      result: "not-a-live-compatibility-claim",
    },
    {
      peer: "cursor",
      version: "t3-bde0a4c0",
      proof: "secondary-adapter-audited-synthetic",
      result: "not-a-live-compatibility-claim",
    },
  ],
});

export const buildFaultMatrix = () => ({
  format: "openagents-acp-fault-matrix-v1",
  bounded: true,
  timeoutMs: 2000,
  rows: FAULT_CASES.map(([layer, fault]) => ({
    layer,
    fault,
    deterministicOracle: "executed-by-conformance-test-and-runtime-report",
    result: "declared-for-execution",
  })),
});

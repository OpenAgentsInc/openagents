import { execFileSync } from "node:child_process";

import { STABLE_METHOD_MANIFEST } from "@openagentsinc/agent-client-protocol/stable";

import { FAULT_CASES } from "./artifacts.ts";
import { STABLE_CONFORMANCE_CASES, stableCaseKey } from "./cases.ts";
import { executeFaultCase } from "./faults.ts";
import { definePeerScenario, runPeerScenario } from "./harness.ts";

export const runExecutedConformanceReport = async () => {
  const outbound = STABLE_CONFORMANCE_CASES.filter(
    (value) => value.direction !== "agent-to-client",
  );
  const reverse = STABLE_CONFORMANCE_CASES.filter((value) => value.direction === "agent-to-client");
  const scenario = definePeerScenario({
    name: "executed-stable-matrix",
    actions: outbound.map((value) => ({
      method: value.method,
      result: value.result,
      ...(value.method === "initialize"
        ? {
            notifications: reverse
              .filter((item) => item.kind === "notification")
              .map((item) => ({ method: item.method, params: item.params })),
            reverseRequests: reverse
              .filter((item) => item.kind === "request")
              .map((item) => ({ method: item.method, params: item.params })),
          }
        : {}),
    })),
  });
  const stable = await runPeerScenario(
    scenario,
    outbound.map((value) => ({
      method: value.method,
      params: value.params,
      kind: value.kind,
    })),
    Object.fromEntries(
      reverse
        .filter((value) => value.kind === "request")
        .map((value) => [value.method, () => value.result]),
    ),
  );
  const faultRows = [];
  for (const [layer, fault] of FAULT_CASES) {
    // Serial execution keeps resource and deadline evidence attributable.
    // eslint-disable-next-line no-await-in-loop -- deterministic report order is intentional.
    faultRows.push(await executeFaultCase(layer, fault));
  }
  const failed = faultRows.filter((row) => row.result !== "pass");
  if (failed.length > 0)
    throw new Error(`fault matrix failed: ${failed.map((row) => row.fault).join(",")}`);
  const openagentsRevision = execFileSync("git", ["rev-parse", "HEAD"], {
    encoding: "utf8",
  }).trim();
  return {
    format: "openagents-acp-executed-conformance-v1",
    generatedAt: new Date().toISOString(),
    openagentsRevision,
    protocol: "Agent Client Protocol",
    schemaRelease: STABLE_METHOD_MANIFEST.schemaRelease,
    schemaSha256: STABLE_METHOD_MANIFEST.sourceSha256,
    platform: `${process.platform}-${process.arch}-node-${process.versions.node}`,
    peer: { name: "scripted-peer", version: "1", generation: stable.receipt.generation },
    methods: STABLE_CONFORMANCE_CASES.map((value) => ({
      key: stableCaseKey(value),
      method: value.method,
      direction: value.direction,
      authMode: value.method === "authenticate" ? "cached_token-fixture" : "not-applicable",
      capability: STABLE_METHOD_MANIFEST.members.find(
        (member) => stableCaseKey(member) === stableCaseKey(value),
      )?.requiredCapability,
      proof: "executed-hermetic-fixture",
      fixtureStatus: "executed",
      liveStatus: "not-applicable",
      result: "pass",
    })),
    transport: {
      requestsCompleted: stable.receipt.counters.requestsCompleted,
      reverseRequests: stable.receipt.counters.reverseRequests,
      terminalOutcome: stable.receipt.terminalOutcome,
    },
    faults: faultRows,
    namedPeerLiveStatus: [
      { peer: "grok", result: "not-run", authority: "separate-opt-in-or-#8897" },
      { peer: "cursor", result: "not-run", authority: "separate-opt-in-or-#8897" },
    ],
  };
};

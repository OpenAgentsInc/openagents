import { stripVTControlCharacters } from "node:util";

import { ACP_RELEASE_SCENARIO_IDS } from "./release.ts";

export type AcpLiveReleaseScenarioReceipt = Readonly<{
  id: (typeof ACP_RELEASE_SCENARIO_IDS)[number];
  result: "live-pass" | "not-observed" | "blocked" | "fail";
  safeDetail: string;
}>;

export type AcpLiveReleasePeerReceipt = Readonly<{
  peer: "grok" | "cursor";
  result: "pass" | "partial" | "fail";
  binary: Readonly<{
    reportedVersion: string;
    executableSha256: string;
    installationClosureSha256?: string;
  }>;
  negotiation: Readonly<{
    wireVersion: 1;
    authMethodIds: ReadonlyArray<string>;
    capabilityKeys: ReadonlyArray<string>;
  }>;
  scenarios: ReadonlyArray<AcpLiveReleaseScenarioReceipt>;
  counters: Readonly<{
    updateCount: number;
    updateKinds: ReadonlyArray<string>;
    promptCount: number;
  }>;
}>;

export type AcpLiveReleaseArtifact = Readonly<{
  format: "openagents-acp-live-release-run-v1";
  protocol: "Agent Client Protocol";
  protocolExclusions: ReadonlyArray<"Agent Communication Protocol" | "A2A">;
  proofClass: "candidate-live";
  claimAuthority: "none-release-matrix-only";
  recordedAt: string;
  openAgentsRevision: string;
  schemaRelease: "schema-v1.19.0";
  platform: string;
  peers: ReadonlyArray<AcpLiveReleasePeerReceipt>;
  redaction: Readonly<{
    promptTextRetained: false;
    responseTextRetained: false;
    sessionIdentifiersRetained: false;
    authMaterialRetained: false;
    absolutePathsRetained: false;
  }>;
}>;

export type AcpDesktopReleaseArtifact = Readonly<{
  format: "openagents-acp-desktop-release-run-v1";
  protocol: "Agent Client Protocol";
  protocolExclusions: ReadonlyArray<"Agent Communication Protocol" | "A2A">;
  proofClass: "candidate-packaged-desktop-live";
  claimAuthority: "none-release-matrix-only";
  recordedAt: string;
  openAgentsRevision: string;
  platform: string;
  provider: "grok" | "cursor";
  lane: "acp:grok-cli" | "acp:cursor-agent";
  packaged: true;
  interruption: Readonly<{
    mismatchedWorkspaceRefused: true;
    laneConfigured: true;
    laneAdmitted: true;
    exitedDuringRunningTurn: true;
  }>;
  recovery: Readonly<{
    reusedDesktopState: true;
    explicitlyReenabledSameThread: true;
    recoveredSameThread: boolean;
    freshThreadRetryAfterFailure: boolean;
    additionalProcessRestartAfterFailedRetry?: boolean;
    laneConfigured: true;
    interruptedTurnSettled: true;
    durableCompletedTurn: true;
    disabled: true;
  }>;
  redaction: Readonly<{
    promptTextRetained: false;
    responseTextRetained: false;
    threadIdentifiersRetained: false;
    authMaterialRetained: false;
    absolutePathsRetained: false;
  }>;
}>;

const secretOrPrivate = (value: unknown): boolean => {
  const encoded = JSON.stringify(value);
  return [
    /\/Users\//,
    /\/home\/[^"/]+\//,
    /Bearer\s+[A-Za-z0-9._~-]+/i,
    /(?:token|secret|api[_-]?key)=[^"\s]+/i,
    /(?:sk|xai)-[A-Za-z0-9_-]{12,}/,
    /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/,
  ].some((pattern) => pattern.test(encoded));
};

const safeToken = (value: string, max = 160): string =>
  /^[A-Za-z0-9][A-Za-z0-9._:+()[\],; /-]*$/.test(value) ? value.slice(0, max) : "withheld";

const safeVersion = (value: string): string => safeToken(stripVTControlCharacters(value).trim(), 256);

export const buildAcpLiveReleaseArtifact = (input: Readonly<{
  recordedAt: string;
  openAgentsRevision: string;
  platform: string;
  peers: ReadonlyArray<AcpLiveReleasePeerReceipt>;
}>): AcpLiveReleaseArtifact => ({
  format: "openagents-acp-live-release-run-v1",
  protocol: "Agent Client Protocol",
  protocolExclusions: ["Agent Communication Protocol", "A2A"],
  proofClass: "candidate-live",
  claimAuthority: "none-release-matrix-only",
  recordedAt: input.recordedAt,
  openAgentsRevision: input.openAgentsRevision,
  schemaRelease: "schema-v1.19.0",
  platform: safeToken(input.platform, 80),
  peers: input.peers.map((peer) => ({
    ...peer,
    binary: {
      reportedVersion: safeVersion(peer.binary.reportedVersion),
      executableSha256: peer.binary.executableSha256,
      ...(peer.binary.installationClosureSha256 === undefined
        ? {}
        : { installationClosureSha256: peer.binary.installationClosureSha256 }),
    },
    negotiation: {
      ...peer.negotiation,
      authMethodIds: peer.negotiation.authMethodIds.map((value) => safeToken(value, 120)),
      capabilityKeys: peer.negotiation.capabilityKeys.map((value) => safeToken(value, 120)),
    },
    scenarios: peer.scenarios.map((scenario) => ({
      ...scenario,
      safeDetail: safeToken(scenario.safeDetail, 240),
    })),
    counters: {
      ...peer.counters,
      updateKinds: peer.counters.updateKinds.map((value) => safeToken(value, 120)),
    },
  })),
  redaction: {
    promptTextRetained: false,
    responseTextRetained: false,
    sessionIdentifiersRetained: false,
    authMaterialRetained: false,
    absolutePathsRetained: false,
  },
});

export const validateAcpLiveReleaseArtifact = (
  artifact: AcpLiveReleaseArtifact,
): Readonly<{ valid: boolean; errors: ReadonlyArray<string> }> => {
  const errors: string[] = [];
  if (artifact.format !== "openagents-acp-live-release-run-v1") errors.push("format is invalid");
  if (artifact.protocol !== "Agent Client Protocol") errors.push("protocol is invalid");
  if (
    artifact.protocolExclusions.length !== 2 ||
    !artifact.protocolExclusions.includes("Agent Communication Protocol") ||
    !artifact.protocolExclusions.includes("A2A")
  )
    errors.push("protocol exclusions are invalid");
  if (artifact.proofClass !== "candidate-live") errors.push("proof class is invalid");
  if (artifact.claimAuthority !== "none-release-matrix-only")
    errors.push("claim authority is invalid");
  if (artifact.schemaRelease !== "schema-v1.19.0") errors.push("schema release is invalid");
  if (Object.values(artifact.redaction).some((retained) => retained !== false))
    errors.push("redaction declaration is invalid");
  if (!/^[a-f0-9]{40}$/.test(artifact.openAgentsRevision))
    errors.push("OpenAgents revision must be a full Git SHA");
  if (!Number.isFinite(Date.parse(artifact.recordedAt))) errors.push("recordedAt is invalid");
  if (artifact.peers.length === 0 || artifact.peers.length > 2)
    errors.push("one or two named peer receipts are required");
  if (new Set(artifact.peers.map((peer) => peer.peer)).size !== artifact.peers.length)
    errors.push("peer receipts must be unique");
  const scenarioIds = new Set<string>(ACP_RELEASE_SCENARIO_IDS);
  for (const peer of artifact.peers) {
    if (!/^[a-f0-9]{64}$/.test(peer.binary.executableSha256))
      errors.push(`${peer.peer}: executable SHA-256 is invalid`);
    if (
      peer.binary.installationClosureSha256 !== undefined &&
      !/^[a-f0-9]{64}$/.test(peer.binary.installationClosureSha256)
    )
      errors.push(`${peer.peer}: installation closure SHA-256 is invalid`);
    if (peer.negotiation.wireVersion !== 1) errors.push(`${peer.peer}: wire version is not 1`);
    const ids = peer.scenarios.map((scenario) => scenario.id);
    if (ids.length === 0) errors.push(`${peer.peer}: at least one scenario receipt is required`);
    if (new Set(ids).size !== ids.length) errors.push(`${peer.peer}: duplicate scenario receipt`);
    if (ids.some((id) => !scenarioIds.has(id))) errors.push(`${peer.peer}: unknown scenario receipt`);
    if (peer.result === "pass" && peer.scenarios.some((scenario) => scenario.result === "fail"))
      errors.push(`${peer.peer}: pass contains a failed scenario`);
    if (peer.result === "fail" && peer.scenarios.every((scenario) => scenario.result !== "fail"))
      errors.push(`${peer.peer}: failed peer has no failed scenario`);
    if (
      !Number.isSafeInteger(peer.counters.updateCount) ||
      peer.counters.updateCount < 0 ||
      !Number.isSafeInteger(peer.counters.promptCount) ||
      peer.counters.promptCount < 0
    )
      errors.push(`${peer.peer}: counters are invalid`);
  }
  if (secretOrPrivate(artifact)) errors.push("artifact contains secret-shaped or host-private data");
  return { valid: errors.length === 0, errors };
};

export const validateAcpDesktopReleaseArtifact = (
  artifact: AcpDesktopReleaseArtifact,
): Readonly<{ valid: boolean; errors: ReadonlyArray<string> }> => {
  const errors: string[] = [];
  if (artifact.format !== "openagents-acp-desktop-release-run-v1")
    errors.push("format is invalid");
  if (artifact.protocol !== "Agent Client Protocol") errors.push("protocol is invalid");
  if (
    artifact.protocolExclusions.length !== 2 ||
    !artifact.protocolExclusions.includes("Agent Communication Protocol") ||
    !artifact.protocolExclusions.includes("A2A")
  )
    errors.push("protocol exclusions are invalid");
  if (artifact.proofClass !== "candidate-packaged-desktop-live")
    errors.push("proof class is invalid");
  if (artifact.claimAuthority !== "none-release-matrix-only")
    errors.push("claim authority is invalid");
  if (!Number.isFinite(Date.parse(artifact.recordedAt))) errors.push("recordedAt is invalid");
  if (!/^[a-f0-9]{40}$/.test(artifact.openAgentsRevision))
    errors.push("OpenAgents revision must be a full Git SHA");
  if (
    (artifact.provider !== "cursor" || artifact.lane !== "acp:cursor-agent") &&
    (artifact.provider !== "grok" || artifact.lane !== "acp:grok-cli")
  )
    errors.push("provider lane identity is invalid");
  if (artifact.packaged !== true) errors.push("packaged execution is required");
  if (Object.values(artifact.interruption).some((value) => value !== true))
    errors.push("interruption proof is incomplete");
  if (
    artifact.recovery.reusedDesktopState !== true ||
    artifact.recovery.explicitlyReenabledSameThread !== true ||
    artifact.recovery.laneConfigured !== true ||
    artifact.recovery.interruptedTurnSettled !== true ||
    artifact.recovery.durableCompletedTurn !== true ||
    artifact.recovery.disabled !== true ||
    (artifact.recovery.recoveredSameThread === artifact.recovery.freshThreadRetryAfterFailure)
  )
    errors.push("recovery proof is incomplete or inconsistent");
  if (Object.values(artifact.redaction).some((retained) => retained !== false))
    errors.push("redaction declaration is invalid");
  if (secretOrPrivate(artifact)) errors.push("artifact contains secret-shaped or host-private data");
  return { valid: errors.length === 0, errors };
};

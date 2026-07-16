export const ACP_RELEASE_CLAIM_STATES = [
  "supported",
  "experimental",
  "incompatible",
  "not-installed",
  "auth-required",
  "degraded",
] as const;

export const ACP_RELEASE_SCENARIO_RESULTS = [
  "live-pass",
  "fixture-pass",
  "blocked",
  "not-tested",
  "unsupported",
  "fail",
] as const;

export const ACP_RELEASE_SCENARIO_IDS = [
  "identity-version",
  "incompatible-version-rejection",
  "initialize",
  "auth-primary",
  "auth-secondary",
  "auth-cancel",
  "auth-expiry-failure",
  "auth-logout-reauth",
  "session-new",
  "session-list",
  "session-load",
  "session-resume-close-delete",
  "real-repo-text",
  "real-repo-tool-plan-config-usage",
  "permission-approval",
  "permission-refusal",
  "permission-timeout-stale-policy",
  "fs-terminal-enabled",
  "fs-terminal-disabled",
  "mcp-authorized",
  "mcp-expired-refusal",
  "mcp-no-durable-secret",
  "model-mode-config",
  "stream-cancel",
  "reverse-cancel",
  "crash-kill-restart",
  "malformed-unknown-output",
  "attachment-repair",
  "sequential-turns",
  "multiple-sessions",
  "cleanup-bounds",
  "fragmented-oversized",
  "stderr-update-flood",
  "queue-stall-timeout-race",
  "crash-loop-repeat",
  "secret-scan",
  "trust-controls",
  "grok-wire-launch",
  "grok-rich-stream",
  "grok-load-replay",
  "grok-question-extensions",
  "cursor-wire-launch",
  "cursor-login-lifecycle",
  "cursor-extensions-models",
  "cursor-load-unsupported",
  "desktop-clean-machine",
  "support-bundle",
] as const;

export type AcpReleaseEvidenceClass =
  | "live-peer"
  | "optional-live-peer"
  | "packaged-desktop-live"
  | "hermetic-production"
  | "not-applicable";

const hermeticProductionScenarios = new Set<string>([
  "incompatible-version-rejection",
  "fs-terminal-disabled",
  "mcp-expired-refusal",
  "malformed-unknown-output",
  "fragmented-oversized",
  "stderr-update-flood",
  "queue-stall-timeout-race",
  "permission-timeout-stale-policy",
  "auth-expiry-failure",
  "trust-controls",
]);
const packagedDesktopScenarios = new Set<string>(["desktop-clean-machine", "support-bundle"]);
const grokOptionalScenarios = new Set<string>(["auth-secondary"]);
const grokNotApplicableScenarios = new Set<string>([
  "auth-logout-reauth",
  "session-resume-close-delete",
  "model-mode-config",
  "cursor-wire-launch",
  "cursor-login-lifecycle",
  "cursor-extensions-models",
  "cursor-load-unsupported",
]);
const cursorNotApplicableScenarios = new Set<string>([
  "auth-secondary",
  "auth-logout-reauth",
  "session-resume-close-delete",
  "fs-terminal-enabled",
  "grok-wire-launch",
  "grok-rich-stream",
  "grok-load-replay",
  "grok-question-extensions",
]);

export const acpReleaseEvidenceClass = (
  peer: "grok" | "cursor",
  scenario: string,
): AcpReleaseEvidenceClass => {
  const notApplicable = peer === "grok" ? grokNotApplicableScenarios : cursorNotApplicableScenarios;
  if (notApplicable.has(scenario)) return "not-applicable";
  if (peer === "grok" && grokOptionalScenarios.has(scenario)) return "optional-live-peer";
  if (hermeticProductionScenarios.has(scenario)) return "hermetic-production";
  if (packagedDesktopScenarios.has(scenario)) return "packaged-desktop-live";
  return "live-peer";
};

const evidenceSatisfies = (evidenceClass: AcpReleaseEvidenceClass, result: unknown): boolean =>
  evidenceClass === "optional-live-peer"
    ? true
    : evidenceClass === "not-applicable"
      ? result === "unsupported"
      : evidenceClass === "hermetic-production"
        ? result === "fixture-pass" || result === "live-pass"
        : result === "live-pass";

export type AcpReleaseClaimState = (typeof ACP_RELEASE_CLAIM_STATES)[number];
export type AcpReleaseScenarioResult = (typeof ACP_RELEASE_SCENARIO_RESULTS)[number];

export type AcpReleaseScenario = Readonly<{
  id: string;
  requiredForSupported: boolean;
  result: AcpReleaseScenarioResult;
  evidenceRefs: ReadonlyArray<string>;
  safeDetail: string;
}>;

export type AcpReleasePeer = Readonly<{
  peer: "grok" | "cursor";
  claimState: AcpReleaseClaimState;
  releaseEligible: boolean;
  scenarios: ReadonlyArray<AcpReleaseScenario>;
}>;

export type AcpReleaseMatrix = Readonly<{
  format: "openagents-acp-release-matrix-v1";
  protocol: "Agent Client Protocol";
  protocolExclusions: ReadonlyArray<"Agent Communication Protocol" | "A2A">;
  recordedAt: string;
  freshnessDays: number;
  peers: ReadonlyArray<AcpReleasePeer>;
}>;

export type AcpReleaseMatrixValidation = Readonly<{
  valid: boolean;
  errors: ReadonlyArray<string>;
}>;

const object = (value: unknown): Record<string, unknown> | undefined =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;

const strings = (value: unknown): ReadonlyArray<string> | undefined =>
  Array.isArray(value) && value.every((entry) => typeof entry === "string") ? value : undefined;

const nonEmptyString = (value: unknown): value is string =>
  typeof value === "string" && value.length > 0;
const sha256 = (value: unknown): value is string =>
  typeof value === "string" && /^[a-f0-9]{64}$/.test(value);
const gitRevision = (value: unknown): value is string =>
  typeof value === "string" && /^[a-f0-9]{40}$/.test(value);

const containsExportedSecret = (value: unknown): boolean => {
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

/**
 * Validates a checked ACP release matrix without trusting its claim booleans.
 * A fixture pass is useful evidence, but it can never satisfy a live release
 * requirement or promote a named peer by itself.
 */
export const validateAcpReleaseMatrix = (
  value: unknown,
  options: Readonly<{ now?: Date }> = {},
): AcpReleaseMatrixValidation => {
  const errors: string[] = [];
  const matrix = object(value);
  if (matrix === undefined) return { valid: false, errors: ["matrix must be an object"] };
  if (matrix.format !== "openagents-acp-release-matrix-v1") errors.push("unexpected format");
  if (matrix.protocol !== "Agent Client Protocol")
    errors.push("protocol must be Agent Client Protocol");
  const exclusions = strings(matrix.protocolExclusions);
  if (
    exclusions === undefined ||
    !exclusions.includes("Agent Communication Protocol") ||
    !exclusions.includes("A2A")
  )
    errors.push("protocol exclusions must name Agent Communication Protocol and A2A");
  const recordedAt = typeof matrix.recordedAt === "string" ? Date.parse(matrix.recordedAt) : NaN;
  const freshnessDays =
    typeof matrix.freshnessDays === "number" && Number.isFinite(matrix.freshnessDays)
      ? matrix.freshnessDays
      : NaN;
  if (!Number.isFinite(recordedAt) || !Number.isFinite(freshnessDays) || freshnessDays <= 0)
    errors.push("matrix freshness metadata is invalid");
  else if (recordedAt - (options.now ?? new Date()).getTime() > 300_000)
    errors.push("matrix evidence timestamp is in the future");
  else if ((options.now ?? new Date()).getTime() - recordedAt > freshnessDays * 86_400_000)
    errors.push("matrix evidence is stale");

  const openAgents = object(matrix.openAgents);
  if (openAgents === undefined) errors.push("OpenAgents release identity is required");
  else {
    if (!gitRevision(openAgents.revision))
      errors.push("OpenAgents revision must be a full Git SHA");
    if (!gitRevision(openAgents.desktopIntegrationRevision))
      errors.push("Desktop integration revision must be a full Git SHA");
    if (!nonEmptyString(openAgents.build)) errors.push("OpenAgents build identity is required");
    if (!nonEmptyString(openAgents.protocolPackageRevision))
      errors.push("protocol package revision is required");
    if (openAgents.schemaRelease !== "schema-v1.19.0")
      errors.push("schema release must be schema-v1.19.0");
    if (!sha256(openAgents.schemaSha256)) errors.push("schema SHA-256 is required");
  }

  const platform = object(matrix.platform);
  if (platform === undefined) errors.push("tested platform identity is required");
  else {
    if (!nonEmptyString(platform.tested) || !/^[a-z0-9]+-[a-z0-9_]+$/.test(platform.tested))
      errors.push("tested platform must be an os-architecture identifier");
    if (!nonEmptyString(platform.osVersion)) errors.push("tested OS version is required");
    if (!nonEmptyString(platform.nodeVersion)) errors.push("tested Node version is required");
    const notTested = strings(platform.notTested);
    if (notTested === undefined) errors.push("not-tested platforms must be explicit");
    else if (new Set(notTested).size !== notTested.length)
      errors.push("not-tested platforms must be unique");
    else if (notTested.includes(String(platform.tested)))
      errors.push("tested platform cannot also be not-tested");
  }

  const peers = Array.isArray(matrix.peers) ? matrix.peers : [];
  if (peers.length !== 2) errors.push("matrix must contain exactly Grok and Cursor");
  const peerNames = new Set<string>();
  let scenarioCatalog: ReadonlyArray<string> | undefined;
  for (const [peerIndex, rawPeer] of peers.entries()) {
    const peer = object(rawPeer);
    if (peer === undefined) {
      errors.push(`peer ${peerIndex} must be an object`);
      continue;
    }
    const peerName = typeof peer.peer === "string" ? peer.peer : `peer-${peerIndex}`;
    peerNames.add(peerName);
    if (peerName !== "grok" && peerName !== "cursor") errors.push(`unknown peer ${peerName}`);
    if (!(ACP_RELEASE_CLAIM_STATES as readonly unknown[]).includes(peer.claimState))
      errors.push(`${peerName}: invalid claim state`);
    const expectedProfile = peerName === "grok" ? "grok-cli" : "cursor-agent";
    const profile = object(peer.profile);
    if (
      profile === undefined ||
      profile.id !== expectedProfile ||
      typeof profile.revision !== "number" ||
      !Number.isInteger(profile.revision) ||
      profile.revision < 1
    )
      errors.push(`${peerName}: exact trusted profile identity is required`);
    const binary = object(peer.binary);
    const expectedCommand = peerName === "grok" ? ["grok", "agent", "stdio"] : ["agent", "acp"];
    if (binary === undefined) errors.push(`${peerName}: pinned binary identity is required`);
    else {
      if (JSON.stringify(strings(binary.command)) !== JSON.stringify(expectedCommand))
        errors.push(`${peerName}: launch command does not match the trusted profile`);
      if (!nonEmptyString(binary.reportedVersion))
        errors.push(`${peerName}: reported version is required`);
      if (
        !nonEmptyString(binary.resolvedExecutableName) ||
        binary.resolvedExecutableName.includes("/") ||
        binary.resolvedExecutableName.includes("\\")
      )
        errors.push(`${peerName}: safe resolved executable name is required`);
      if (!sha256(binary.sha256)) errors.push(`${peerName}: executable SHA-256 is required`);
      if (peerName === "cursor" && !sha256(binary.installationClosureSha256))
        errors.push("cursor: installation closure SHA-256 is required");
      if (!nonEmptyString(binary.installationSource))
        errors.push(`${peerName}: installation source is required`);
    }
    const negotiation = object(peer.negotiation);
    if (negotiation === undefined) errors.push(`${peerName}: initialize evidence is required`);
    else {
      if (negotiation.wireVersion !== 1) errors.push(`${peerName}: wire version must be 1`);
      const identity = object(negotiation.peerIdentity);
      if (
        identity === undefined ||
        !nonEmptyString(identity.name) ||
        !nonEmptyString(identity.version)
      )
        errors.push(`${peerName}: peer initialize identity is required`);
      if (strings(negotiation.authMethodIds) === undefined)
        errors.push(`${peerName}: advertised auth methods are required`);
      if (strings(negotiation.advertisedCapabilityKeys) === undefined)
        errors.push(`${peerName}: advertised capabilities are required`);
    }
    const scenarios = Array.isArray(peer.scenarios) ? peer.scenarios : [];
    const ids: string[] = [];
    let allRequiredSatisfied = scenarios.length > 0;
    for (const [scenarioIndex, rawScenario] of scenarios.entries()) {
      const scenario = object(rawScenario);
      if (scenario === undefined) {
        errors.push(`${peerName}: scenario ${scenarioIndex} must be an object`);
        allRequiredSatisfied = false;
        continue;
      }
      const id = typeof scenario.id === "string" ? scenario.id : `scenario-${scenarioIndex}`;
      ids.push(id);
      const evidenceClass =
        peerName === "grok" || peerName === "cursor"
          ? acpReleaseEvidenceClass(peerName, id)
          : "live-peer";
      const expectedRequired =
        evidenceClass !== "not-applicable" && evidenceClass !== "optional-live-peer";
      if (scenario.requiredForSupported !== expectedRequired)
        errors.push(`${peerName}/${id}: requiredness does not match the code-owned catalog`);
      if (!(ACP_RELEASE_SCENARIO_RESULTS as readonly unknown[]).includes(scenario.result))
        errors.push(`${peerName}/${id}: invalid result`);
      if (!evidenceSatisfies(evidenceClass, scenario.result)) allRequiredSatisfied = false;
      const evidenceRefs = strings(scenario.evidenceRefs);
      if (evidenceRefs === undefined)
        errors.push(`${peerName}/${id}: evidenceRefs must be strings`);
      else if (
        evidenceRefs.some(
          (ref) =>
            ref.startsWith("/") ||
            ref.includes("..") ||
            ref.includes("\\") ||
            /^[a-z][a-z0-9+.-]*:/i.test(ref),
        )
      )
        errors.push(`${peerName}/${id}: evidence refs must be repository-relative`);
      else if (
        (scenario.result === "live-pass" || scenario.result === "fixture-pass") &&
        evidenceRefs.length === 0
      )
        errors.push(`${peerName}/${id}: passing evidence class requires evidence`);
      if (typeof scenario.safeDetail !== "string" || scenario.safeDetail.length === 0)
        errors.push(`${peerName}/${id}: safeDetail is required`);
    }
    if (new Set(ids).size !== ids.length) errors.push(`${peerName}: scenario ids must be unique`);
    if (JSON.stringify(ids.toSorted()) !== JSON.stringify([...ACP_RELEASE_SCENARIO_IDS].toSorted()))
      errors.push(`${peerName}: scenario catalog is incomplete or unknown`);
    if (scenarioCatalog === undefined) scenarioCatalog = ids.toSorted();
    else if (JSON.stringify(scenarioCatalog) !== JSON.stringify(ids.toSorted()))
      errors.push("Grok and Cursor must use the same release scenario catalog");
    if (peer.releaseEligible !== allRequiredSatisfied)
      errors.push(`${peerName}: releaseEligible does not match code-owned evidence requirements`);
    if (peer.claimState === "supported" && !allRequiredSatisfied)
      errors.push(`${peerName}: supported claim lacks required evidence`);
  }
  if (!peerNames.has("grok") || !peerNames.has("cursor"))
    errors.push("matrix must independently identify Grok and Cursor");
  if (containsExportedSecret(matrix))
    errors.push("matrix contains secret-shaped or host-private data");
  return { valid: errors.length === 0, errors };
};

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
  Array.isArray(value) && value.every((entry) => typeof entry === "string")
    ? value
    : undefined;

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
  if (matrix.protocol !== "Agent Client Protocol") errors.push("protocol must be Agent Client Protocol");
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
  else if ((options.now ?? new Date()).getTime() - recordedAt > freshnessDays * 86_400_000)
    errors.push("matrix evidence is stale");

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
    const scenarios = Array.isArray(peer.scenarios) ? peer.scenarios : [];
    const ids: string[] = [];
    let allRequiredLive = scenarios.length > 0;
    for (const [scenarioIndex, rawScenario] of scenarios.entries()) {
      const scenario = object(rawScenario);
      if (scenario === undefined) {
        errors.push(`${peerName}: scenario ${scenarioIndex} must be an object`);
        allRequiredLive = false;
        continue;
      }
      const id = typeof scenario.id === "string" ? scenario.id : `scenario-${scenarioIndex}`;
      ids.push(id);
      if (!(ACP_RELEASE_SCENARIO_RESULTS as readonly unknown[]).includes(scenario.result))
        errors.push(`${peerName}/${id}: invalid result`);
      if (scenario.requiredForSupported === true && scenario.result !== "live-pass")
        allRequiredLive = false;
      const evidenceRefs = strings(scenario.evidenceRefs);
      if (evidenceRefs === undefined) errors.push(`${peerName}/${id}: evidenceRefs must be strings`);
      else if (evidenceRefs.some((ref) => ref.startsWith("/") || ref.includes("..")))
        errors.push(`${peerName}/${id}: evidence refs must be repository-relative`);
      if (typeof scenario.safeDetail !== "string" || scenario.safeDetail.length === 0)
        errors.push(`${peerName}/${id}: safeDetail is required`);
    }
    if (new Set(ids).size !== ids.length) errors.push(`${peerName}: scenario ids must be unique`);
    if (scenarioCatalog === undefined) scenarioCatalog = ids.toSorted();
    else if (JSON.stringify(scenarioCatalog) !== JSON.stringify(ids.toSorted()))
      errors.push("Grok and Cursor must use the same release scenario catalog");
    if (peer.releaseEligible !== allRequiredLive)
      errors.push(`${peerName}: releaseEligible does not match required live evidence`);
    if (peer.claimState === "supported" && !allRequiredLive)
      errors.push(`${peerName}: supported claim lacks required live evidence`);
  }
  if (!peerNames.has("grok") || !peerNames.has("cursor"))
    errors.push("matrix must independently identify Grok and Cursor");
  if (containsExportedSecret(matrix)) errors.push("matrix contains secret-shaped or host-private data");
  return { valid: errors.length === 0, errors };
};

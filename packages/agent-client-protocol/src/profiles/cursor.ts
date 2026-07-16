/**
 * Cursor Agent CLI trusted peer profile (ACP-9 #8896; peer implementation
 * ACP-8 #8894). Launch pin: `agent acp`. Extension identity comes only from
 * the Cursor vendor-extension module — no Grok code is shared here.
 *
 * Version pins are deliberately conservative: no version is marked
 * `supported` until the pinned live compatibility matrix (#8897) lands its
 * evidence, so admission derives at most `experimental` for current builds.
 */

import { CURSOR_ACP_PROFILE } from "../extensions/cursor.ts";

export const CURSOR_TRUSTED_PEER_PROFILE = {
  contractVersion: 1,
  protocol: "Agent Client Protocol",
  schemaRelease: "schema-v1.19.0",
  wireVersion: 1,
  profileId: "cursor-agent",
  providerId: "cursor",
  profileRevision: 1,
  display: {
    name: "Cursor Agent CLI",
    description:
      "Cursor coding agent hosted over the Agent Client Protocol stdio surface (agent acp).",
  },
  provenance: {
    source: "openagents-trusted",
    auditRef: "docs/teardowns/2026-07-16-t3-code-agent-client-protocol-implementation-teardown.md",
  },
  versions: {
    supported: [],
    // Cursor Agent CLI publishes date-shaped numeric versions (e.g. 2025.9.12).
    experimental: [{ kind: "bounded", fromInclusive: "2024.1.0", toExclusive: "2028.1.0" }],
    denied: [],
  },
  launch: {
    strategy: "trusted-path-lookup",
    executable: "agent",
    args: ["acp"],
    versionProbeArgs: ["--version"],
  },
  environment: {
    allowedKeys: ["CURSOR_API_KEY"],
    secretRefs: [
      {
        key: "CURSOR_API_KEY",
        requirement: "optional",
        secretRef: "secret-manager:cursor-api-key",
      },
    ],
  },
  identity: {
    expectedExecutableBasename: "agent",
    expectedAgentName: { kind: "prefix", value: "cursor" },
    versionExtraction: "leading-semver",
  },
  auth: {
    policy: "advertised-methods-only",
    methods: [
      { id: "cursor-login", kind: "interactive-login", interaction: "external-browser" },
      {
        id: "api-key",
        kind: "api-key-secret",
        interaction: "none",
        secretRefKey: "CURSOR_API_KEY",
      },
    ],
  },
  capabilities: [
    { capability: "prompt.text", state: "supported" },
    { capability: "sessionUpdates.streaming", state: "supported" },
    { capability: "modes", state: "experimental" },
    { capability: "configuration.models", state: "experimental" },
    { capability: "fs.readTextFile", state: "unsupported" },
    { capability: "fs.writeTextFile", state: "unsupported" },
    { capability: "terminal", state: "unsupported" },
    { capability: "network", state: "unsupported" },
  ],
  deviations: [
    {
      id: "capability-truth-pending-live-pin",
      description:
        "Mode, model-discovery, and capability truth-values are pinned finally by the live matrix (#8894/#8897); until then every state above experimental is withheld.",
    },
  ],
  configuration: { modes: [], modelConfigOptionIds: [] },
  extensions: CURSOR_ACP_PROFILE.methods.map((member) => ({
    method: member.method,
    direction: member.direction,
    kind: member.kind,
    extensionProfileVersion: CURSOR_ACP_PROFILE.profileVersion,
  })),
  sessionPolicy: {
    ownership: "single-root-session",
    restore: "unsupported",
    cancellation: "session-cancel",
    shutdown: "dispose-process",
  },
  evidence: {
    fixtureSuites: ["acp-wire-v1-conformance"],
    liveMatrixRequired: true,
    maxEvidenceAgeDays: 30,
  },
  platforms: [
    { os: "darwin", arch: "arm64" },
    { os: "darwin", arch: "x64" },
    { os: "linux", arch: "arm64" },
    { os: "linux", arch: "x64" },
  ],
  install: {
    kind: "external",
    guidance:
      "Install the Cursor Agent CLI through Cursor's official distribution (provides the agent executable). Admission trusts only the probe-verified executable identity, never the installer.",
  },
  redaction: { additionalSensitiveKeys: ["CURSOR_API_KEY", "login_state"] },
} as const;

/**
 * Cursor Agent CLI trusted peer profile (ACP-9 #8896; peer implementation
 * ACP-7 #8894). Launch pin: `agent acp`. Extension identity comes only from
 * the Cursor vendor-extension module — no Grok code is shared here.
 *
 * Version pins are deliberately conservative: the probe-verified build remains
 * experimental until #8897 installs the complete compatibility matrix.
 */

import { CURSOR_ACP_PROFILE } from "../extensions/cursor.ts";

export const CURSOR_TRUSTED_PEER_PROFILE = {
  contractVersion: 1,
  protocol: "Agent Client Protocol",
  schemaRelease: "schema-v1.19.0",
  wireVersion: 1,
  profileId: "cursor-agent",
  providerId: "cursor",
  profileRevision: 2,
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
    // Diagnostic live pin from the official Cursor Agent CLI distribution on
    // Darwin arm64. Full support remains gated by the #8897 compatibility
    // matrix; unknown builds are not admitted by this profile.
    experimental: [{ kind: "exact", version: "2026.6.24" }],
    denied: [],
  },
  launch: {
    strategy: "trusted-path-lookup",
    executable: "agent",
    args: ["acp"],
    versionProbeArgs: ["--version"],
  },
  environment: { allowedKeys: ["HOME"], secretRefs: [] },
  identity: {
    expectedExecutableBasename: "cursor-agent",
    expectedAgentName: { kind: "prefix", value: "cursor" },
    versionExtraction: "leading-semver",
  },
  auth: {
    policy: "advertised-methods-only",
    methods: [{ id: "cursor_login", kind: "interactive-login", interaction: "external-browser" }],
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
    fixtureSuites: ["acp-wire-v1-conformance", "cursor-t3-bde0a4c0"],
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
  redaction: { additionalSensitiveKeys: ["login_state", "device_code", "verification_uri"] },
} as const;

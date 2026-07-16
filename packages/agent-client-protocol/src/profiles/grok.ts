/**
 * Grok CLI trusted peer profile (ACP-9 #8896; peer implementation ACP-7
 * #8893). Launch pin: `grok agent stdio`. Extension identity comes only from
 * the Grok vendor-extension module — no Cursor code is shared here.
 *
 * Version pins are deliberately conservative: no version is marked
 * `supported` until the pinned live compatibility matrix (#8897) lands its
 * evidence, so admission derives at most `experimental` for current builds.
 */

import { GROK_ACP_PROFILE } from "../extensions/grok.ts";

export const GROK_TRUSTED_PEER_PROFILE = {
  contractVersion: 1,
  protocol: "Agent Client Protocol",
  schemaRelease: "schema-v1.19.0",
  wireVersion: 1,
  profileId: "grok-cli",
  providerId: "x-ai",
  profileRevision: 1,
  display: {
    name: "Grok CLI",
    description:
      "xAI Grok coding agent hosted over the Agent Client Protocol stdio surface (grok agent stdio).",
  },
  provenance: {
    source: "openagents-trusted",
    auditRef: "docs/teardowns/2026-07-16-t3-code-agent-client-protocol-implementation-teardown.md",
  },
  versions: {
    supported: [],
    experimental: [{ kind: "bounded", fromInclusive: "0.0.1", toExclusive: "2.0.0" }],
    denied: [],
  },
  launch: {
    strategy: "trusted-path-lookup",
    executable: "grok",
    args: ["agent", "stdio"],
    versionProbeArgs: ["version"],
  },
  environment: {
    allowedKeys: ["XAI_API_KEY"],
    secretRefs: [
      {
        key: "XAI_API_KEY",
        requirement: "optional",
        secretRef: "secret-manager:xai-api-key",
      },
    ],
  },
  identity: {
    expectedExecutableBasename: "grok",
    expectedAgentName: { kind: "prefix", value: "grok" },
    versionExtraction: "leading-semver",
  },
  auth: {
    policy: "advertised-methods-only",
    methods: [
      { id: "cached-token", kind: "cached-token", interaction: "none" },
      {
        id: "api-key",
        kind: "api-key-secret",
        interaction: "none",
        secretRefKey: "XAI_API_KEY",
      },
    ],
  },
  capabilities: [
    { capability: "prompt.text", state: "supported" },
    { capability: "sessionUpdates.streaming", state: "supported" },
    { capability: "fs.readTextFile", state: "unsupported" },
    { capability: "fs.writeTextFile", state: "unsupported" },
    { capability: "terminal", state: "unsupported" },
    { capability: "network", state: "unsupported" },
    { capability: "session.load", state: "experimental" },
  ],
  deviations: [
    {
      id: "auth-method-ids-pending-live-pin",
      description:
        "Advertised auth method identifiers and capability truth-values are pinned finally by the live matrix (#8893/#8897); until then every state above experimental is withheld.",
    },
    {
      id: "underscore-extension-compatibility",
      description:
        "Grok has shipped both x.ai/ask_user_question and the underscore-prefixed compatibility variant; both are declared so neither bypasses the allowlist.",
    },
  ],
  configuration: { modes: [], modelConfigOptionIds: [] },
  extensions: GROK_ACP_PROFILE.methods.map((member) => ({
    method: member.method,
    direction: member.direction,
    kind: member.kind,
    extensionProfileVersion: GROK_ACP_PROFILE.profileVersion,
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
      "Install the Grok CLI through xAI's official distribution. Admission trusts only the probe-verified executable identity, never the installer.",
  },
  redaction: { additionalSensitiveKeys: ["XAI_API_KEY", "login_state"] },
} as const;

/**
 * Grok CLI trusted peer profile (ACP-9 #8896; peer implementation ACP-7
 * #8893). Launch pin: `grok agent stdio`. Extension identity comes only from
 * the Grok vendor-extension module — no Cursor code is shared here.
 *
 * Version 0.2.101 is the first exact release candidate. Admission still
 * derives `experimental` unless fresh fixture and digest-bound live evidence
 * is supplied; the final release claim remains gated by #8897.
 */

import { GROK_ACP_PROFILE } from "../extensions/grok.ts";

export const GROK_ACP_VERSION_COMPATIBILITY = Object.freeze({
  "0.2.101": Object.freeze({
    unstableSetModel: false,
    privatePromptCompletionFallback: false,
  }),
});

export const grokAcpCompatibilityForVersion = (
  version: string,
): Readonly<{ unstableSetModel: boolean; privatePromptCompletionFallback: boolean }> =>
  GROK_ACP_VERSION_COMPATIBILITY[version as keyof typeof GROK_ACP_VERSION_COMPATIBILITY] ??
  Object.freeze({ unstableSetModel: false, privatePromptCompletionFallback: false });

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
    supported: [{ kind: "exact", version: "0.2.101" }],
    experimental: [
      { kind: "bounded", fromInclusive: "0.2.0", toExclusive: "0.2.101" },
      { kind: "bounded", fromInclusive: "0.2.102", toExclusive: "0.3.0" },
    ],
    denied: [],
  },
  launch: {
    strategy: "trusted-path-lookup",
    executable: "grok",
    args: ["agent", "stdio"],
    versionProbeArgs: ["version"],
  },
  environment: {
    allowedKeys: ["HOME", "XAI_API_KEY"],
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
      { id: "cached_token", kind: "cached-token", interaction: "none" },
      {
        id: "xai.api_key",
        kind: "api-key-secret",
        interaction: "none",
        secretRefKey: "XAI_API_KEY",
      },
      { id: "grok.com", kind: "interactive-login", interaction: "external-browser" },
      { id: "oidc", kind: "interactive-login", interaction: "external-browser" },
    ],
  },
  capabilities: [
    { capability: "prompt.text", state: "supported" },
    { capability: "sessionUpdates.streaming", state: "supported" },
    { capability: "fs.readTextFile", state: "supported" },
    { capability: "fs.writeTextFile", state: "supported" },
    { capability: "terminal", state: "supported" },
    { capability: "network", state: "unsupported" },
    { capability: "session.load", state: "supported" },
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
    restore: "session-load",
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

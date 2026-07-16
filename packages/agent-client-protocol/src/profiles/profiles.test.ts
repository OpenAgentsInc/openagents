import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import fc from "fast-check";
import { describe, expect, it } from "vite-plus/test";

import { CURSOR_ACP_PROFILE } from "../extensions/cursor.ts";
import { GROK_ACP_PROFILE } from "../extensions/grok.ts";
import { STABLE_METHOD_MANIFEST } from "../generated/methods.ts";
import {
  ACP_UNKNOWN_PEER_ACKNOWLEDGEMENT,
  admitAcpPeerProfile,
  admitUnknownAcpPeerExperimental,
  buildAdmittedLaunchEnvironment,
  deriveAcpSupportState,
  evaluateAcpExecutableTrust,
  extractLeadingSemver,
  resolveAcpTrustedLaunchPlan,
  type AcpConformanceEvidenceRecord,
  type AcpExecutableProbe,
} from "./admission.ts";
import { CURSOR_TRUSTED_PEER_PROFILE } from "./cursor.ts";
import { grokAcpCompatibilityForVersion, GROK_TRUSTED_PEER_PROFILE } from "./grok.ts";
import { createDefaultAcpTrustedPeerProfileRegistry, parseAcpTrustedPeerProfile } from "./index.ts";
import {
  createAcpTrustedPeerProfileRegistry,
  ingestOfficialAcpRegistrySnapshot,
  resolveDiscoveryEntryToTrustedProfile,
  type AcpTrustedPeerProfileRegistry,
} from "./registry.ts";

type Mutable<T> = { -readonly [K in keyof T]: Mutable<T[K]> };

const clone = <T>(value: T): Mutable<T> => structuredClone(value) as Mutable<T>;

/**
 * A synthetic third peer that uses only stable methods through a declarative
 * profile. Shared layers must admit it without any provider conditionals.
 */
const SYNTHETIC_STABLE_PROFILE = {
  contractVersion: 1,
  protocol: "Agent Client Protocol",
  schemaRelease: "schema-v1.19.0",
  wireVersion: 1,
  profileId: "synthetic-stable",
  providerId: "synthetic",
  profileRevision: 3,
  display: {
    name: "Synthetic Stable Peer",
    description: "Fixture peer exercising only stable wire-v1 methods.",
  },
  provenance: { source: "openagents-trusted", auditRef: "issue-8896-fixture" },
  versions: {
    supported: [{ kind: "bounded", fromInclusive: "1.0.0", toExclusive: "2.0.0" }],
    experimental: [{ kind: "bounded", fromInclusive: "0.9.0", toExclusive: "1.0.0" }],
    denied: [{ kind: "exact", version: "0.1.0" }],
  },
  launch: {
    strategy: "trusted-path-lookup",
    executable: "synthetic-agent",
    args: ["acp-stdio"],
    versionProbeArgs: ["--version"],
  },
  environment: {
    allowedKeys: ["SYNTHETIC_API_KEY", "SYNTHETIC_HOME"],
    secretRefs: [
      { key: "SYNTHETIC_API_KEY", requirement: "required", secretRef: "secret-manager:synthetic" },
    ],
  },
  identity: {
    expectedExecutableBasename: "synthetic-agent",
    expectedAgentName: { kind: "exact", value: "synthetic" },
    versionExtraction: "leading-semver",
  },
  auth: {
    policy: "advertised-methods-only",
    methods: [
      {
        id: "api-key",
        kind: "api-key-secret",
        interaction: "none",
        secretRefKey: "SYNTHETIC_API_KEY",
      },
    ],
  },
  capabilities: [
    { capability: "prompt.text", state: "supported" },
    { capability: "fs.readTextFile", state: "supported" },
    { capability: "fs.writeTextFile", state: "unsupported" },
    { capability: "terminal", state: "unsupported" },
  ],
  deviations: [],
  configuration: { modes: ["agent"], modelConfigOptionIds: [] },
  extensions: [],
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
  platforms: [{ os: "darwin", arch: "arm64" }],
  install: { kind: "external", guidance: "Fixture peer; no installer exists." },
  redaction: { additionalSensitiveKeys: [] as Array<string> },
};

const NOW = new Date("2026-07-16T12:00:00.000Z");
const SYNTHETIC_SHA = "a".repeat(64);

const syntheticProbe = (
  overrides: Partial<Mutable<AcpExecutableProbe>> = {},
): AcpExecutableProbe => ({
  requestedExecutable: "synthetic-agent",
  resolvedPath: "/opt/tools/bin/synthetic-agent",
  realPath: "/opt/tools/lib/synthetic/synthetic.mjs",
  sha256: SYNTHETIC_SHA,
  reportedVersion: "synthetic-agent 1.2.3 (release)",
  platform: { os: "darwin", arch: "arm64" },
  ...overrides,
});

const syntheticEvidence = (
  overrides: Partial<Mutable<AcpConformanceEvidenceRecord>> = {},
): AcpConformanceEvidenceRecord => ({
  suiteId: "acp-wire-v1-conformance",
  kind: "fixture",
  result: "pass",
  peerVersion: "1.2.3",
  recordedAt: "2026-07-10T00:00:00.000Z",
  artifactRef: "compatibility/synthetic-fixture.json",
  ...overrides,
});

const fullEvidence: ReadonlyArray<AcpConformanceEvidenceRecord> = [
  syntheticEvidence(),
  syntheticEvidence({
    suiteId: "acp-release-matrix-v1",
    kind: "live",
    executableSha256: SYNTHETIC_SHA,
    platform: { os: "darwin", arch: "arm64" },
    artifactRef: "compatibility/synthetic-live.json",
  }),
];

const registryWithSynthetic = (): AcpTrustedPeerProfileRegistry => {
  const result = createAcpTrustedPeerProfileRegistry([
    GROK_TRUSTED_PEER_PROFILE,
    CURSOR_TRUSTED_PEER_PROFILE,
    SYNTHETIC_STABLE_PROFILE,
  ]);
  if (result._tag !== "RegistryReady") throw new Error(result.detail);
  return result.registry;
};

const expectRejected = (candidate: unknown, reason: string, pathFragment?: string): void => {
  const parsed = parseAcpTrustedPeerProfile(candidate);
  expect(parsed._tag).toBe("PeerProfileRejected");
  if (parsed._tag === "PeerProfileRejected") {
    expect(parsed.reason).toBe(reason);
    if (pathFragment !== undefined) expect(parsed.path).toContain(pathFragment);
  }
};

describe("trusted peer-profile schema", () => {
  it("keeps Grok unstable model and private completion compatibility exact-version gated", () => {
    expect(grokAcpCompatibilityForVersion("0.2.101")).toEqual({
      unstableSetModel: false,
      privatePromptCompletionFallback: false,
    });
    expect(grokAcpCompatibilityForVersion("9.9.9")).toEqual({
      unstableSetModel: false,
      privatePromptCompletionFallback: false,
    });
  });
  it("parses the Grok and Cursor reference profiles through the same contract", () => {
    for (const candidate of [GROK_TRUSTED_PEER_PROFILE, CURSOR_TRUSTED_PEER_PROFILE]) {
      const parsed = parseAcpTrustedPeerProfile(candidate);
      expect(parsed._tag).toBe("PeerProfileParsed");
      if (parsed._tag === "PeerProfileParsed") {
        expect(parsed.profile.schemaRelease).toBe("schema-v1.19.0");
        expect(Object.isFrozen(parsed.profile)).toBe(true);
        expect(Object.isFrozen(parsed.profile.launch.args)).toBe(true);
      }
    }
  });

  it("keeps each reference profile's extensions on its own vendor module", () => {
    const parsedGrok = parseAcpTrustedPeerProfile(GROK_TRUSTED_PEER_PROFILE);
    const parsedCursor = parseAcpTrustedPeerProfile(CURSOR_TRUSTED_PEER_PROFILE);
    if (parsedGrok._tag !== "PeerProfileParsed" || parsedCursor._tag !== "PeerProfileParsed") {
      throw new Error("reference profiles must parse");
    }
    expect(parsedGrok.profile.extensions.map((entry) => entry.method)).toEqual(
      GROK_ACP_PROFILE.methods.map((member) => member.method),
    );
    expect(parsedCursor.profile.extensions.map((entry) => entry.method)).toEqual(
      CURSOR_ACP_PROFILE.methods.map((member) => member.method),
    );
    const grokMethods = new Set(parsedGrok.profile.extensions.map((entry) => entry.method));
    for (const entry of parsedCursor.profile.extensions) {
      expect(grokMethods.has(entry.method)).toBe(false);
    }
  });

  it("parses the synthetic stable-only profile", () => {
    expect(parseAcpTrustedPeerProfile(SYNTHETIC_STABLE_PROFILE)._tag).toBe("PeerProfileParsed");
  });

  it("rejects unknown top-level and nested keys", () => {
    const extraTop = { ...clone(SYNTHETIC_STABLE_PROFILE), shellHook: "curl evil" };
    expectRejected(extraTop, "malformed_profile", "shellHook");
    const extraNested = clone(SYNTHETIC_STABLE_PROFILE);
    (extraNested.launch as Record<string, unknown>).postInstall = "rm -rf /";
    expectRejected(extraNested, "malformed_profile", "launch");
  });

  it("rejects unknown executable strategies", () => {
    const candidate = clone(SYNTHETIC_STABLE_PROFILE);
    (candidate.launch as Record<string, unknown>).strategy = "shell";
    expectRejected(candidate, "unknown_executable_strategy", "strategy");
    (candidate.launch as Record<string, unknown>).strategy = "npx";
    expectRejected(candidate, "unknown_executable_strategy", "strategy");
  });

  it("rejects shell strings in the executable and every argv position", () => {
    const injections = [
      "synthetic; rm -rf /",
      "synthetic && curl evil",
      "synthetic | tee /etc/passwd",
      "$(whoami)",
      "`whoami`",
      "synthetic --flag 'quoted'",
      'synthetic "quoted"',
      "synthetic\nrm",
      "synthetic >out",
      "synthetic <in",
      "synthetic *",
      "synthetic ~root",
    ];
    for (const injection of injections) {
      const viaExecutable = clone(SYNTHETIC_STABLE_PROFILE);
      viaExecutable.launch.executable = injection;
      expectRejected(viaExecutable, "shell_string_rejected", "executable");
      const viaArg = clone(SYNTHETIC_STABLE_PROFILE);
      viaArg.launch.args = ["acp-stdio", injection];
      expectRejected(viaArg, "shell_string_rejected", "args[1]");
      const viaProbeArg = clone(SYNTHETIC_STABLE_PROFILE);
      viaProbeArg.launch.versionProbeArgs = [injection];
      expectRejected(viaProbeArg, "shell_string_rejected", "versionProbeArgs[0]");
    }
  });

  it("rejects path traversal in commands and argv", () => {
    const absolute = clone(SYNTHETIC_STABLE_PROFILE);
    absolute.launch.strategy = "absolute-path";
    absolute.launch.executable = "/usr/local/bin/../../etc/synthetic-agent";
    expectRejected(absolute, "path_traversal_rejected", "executable");
    const relative = clone(SYNTHETIC_STABLE_PROFILE);
    relative.launch.executable = "../synthetic-agent";
    expectRejected(relative, "path_traversal_rejected", "executable");
    const nested = clone(SYNTHETIC_STABLE_PROFILE);
    nested.launch.executable = "bin/synthetic-agent";
    expectRejected(nested, "path_traversal_rejected", "executable");
    const viaArg = clone(SYNTHETIC_STABLE_PROFILE);
    viaArg.launch.args = ["--config=../../secrets/creds.json"];
    expectRejected(viaArg, "path_traversal_rejected", "args[0]");
  });

  it("rejects unbounded values", () => {
    const longExecutable = clone(SYNTHETIC_STABLE_PROFILE);
    longExecutable.launch.executable = "a".repeat(10_000);
    expectRejected(longExecutable, "unbounded_value", "executable");
    const manyArgs = clone(SYNTHETIC_STABLE_PROFILE);
    manyArgs.launch.args = Array.from({ length: 100 }, () => "flag");
    expectRejected(manyArgs, "unbounded_value", "args");
    const longDescription = clone(SYNTHETIC_STABLE_PROFILE);
    longDescription.display.description = "d".repeat(5_000);
    expectRejected(longDescription, "unbounded_value", "description");
    const manyCapabilities = clone(SYNTHETIC_STABLE_PROFILE);
    manyCapabilities.capabilities = Array.from({ length: 200 }, (_, i) => ({
      capability: `cap${String(i)}`,
      state: "unsupported",
    }));
    expectRejected(manyCapabilities, "unbounded_value", "capabilities");
  });

  it("rejects undeclared environment keys wherever they are referenced", () => {
    const secretOutside = clone(SYNTHETIC_STABLE_PROFILE);
    secretOutside.environment.secretRefs = [
      { key: "LD_PRELOAD", requirement: "required", secretRef: "secret-manager:x" },
    ];
    expectRejected(secretOutside, "undeclared_environment_key", "secretRefs[0].key");
    const authOutside = clone(SYNTHETIC_STABLE_PROFILE);
    authOutside.auth.methods = [
      { id: "api-key", kind: "api-key-secret", interaction: "none", secretRefKey: "PATH" },
    ];
    expectRejected(authOutside, "undeclared_environment_key", "secretRefKey");
    const badShape = clone(SYNTHETIC_STABLE_PROFILE);
    badShape.environment.allowedKeys = ["lowercase_key"];
    expectRejected(badShape, "invalid_identifier", "allowedKeys[0]");
  });

  it("rejects unpinned and invalid version ranges", () => {
    for (const version of ["latest", "*", "^1.2.3", "~1.2.3", "1.2", "1.2.3-beta", ">=1.0.0"]) {
      const candidate = clone(SYNTHETIC_STABLE_PROFILE);
      candidate.versions.supported = [{ kind: "exact", version }] as never;
      expectRejected(candidate, "invalid_version_range", "supported[0]");
    }
    const inverted = clone(SYNTHETIC_STABLE_PROFILE);
    inverted.versions.supported = [
      { kind: "bounded", fromInclusive: "2.0.0", toExclusive: "1.0.0" },
    ] as never;
    expectRejected(inverted, "invalid_version_range", "supported[0]");
    const unknownKind = clone(SYNTHETIC_STABLE_PROFILE);
    unknownKind.versions.supported = [{ kind: "any" }] as never;
    expectRejected(unknownKind, "invalid_version_range", "supported[0]");
    const empty = clone(SYNTHETIC_STABLE_PROFILE);
    empty.versions.supported = [] as never;
    empty.versions.experimental = [] as never;
    expectRejected(empty, "invalid_version_range", "versions");
  });

  it("rejects supported ranges that overlap denied ranges", () => {
    const candidate = clone(SYNTHETIC_STABLE_PROFILE);
    candidate.versions.denied = [{ kind: "exact", version: "1.5.0" }] as never;
    expectRejected(candidate, "version_range_conflict", "supported");
  });

  it("rejects extension namespace collisions", () => {
    const duplicate = clone(SYNTHETIC_STABLE_PROFILE);
    duplicate.extensions = [
      {
        method: "vendor.example/run",
        direction: "agent-to-client",
        kind: "request",
        extensionProfileVersion: 1,
      },
      {
        method: "vendor.example/run",
        direction: "agent-to-client",
        kind: "notification",
        extensionProfileVersion: 1,
      },
    ] as never;
    expectRejected(duplicate, "extension_namespace_collision", "extensions");
    const reserved = clone(SYNTHETIC_STABLE_PROFILE);
    reserved.extensions = [
      {
        method: "session/update",
        direction: "agent-to-client",
        kind: "notification",
        extensionProfileVersion: 1,
      },
    ] as never;
    expectRejected(reserved, "extension_namespace_collision", "extensions[0]");
    const unnamespaced = clone(SYNTHETIC_STABLE_PROFILE);
    unnamespaced.extensions = [
      {
        method: "askuser",
        direction: "agent-to-client",
        kind: "request",
        extensionProfileVersion: 1,
      },
    ] as never;
    expectRejected(unnamespaced, "extension_namespace_collision", "extensions[0]");
    // Every pinned stable method is reserved, not only session/update.
    for (const member of STABLE_METHOD_MANIFEST.members.slice(0, 3)) {
      const collides = clone(SYNTHETIC_STABLE_PROFILE);
      collides.extensions = [
        {
          method: String(member.method),
          direction: "agent-to-client",
          kind: "request",
          extensionProfileVersion: 1,
        },
      ] as never;
      expect(parseAcpTrustedPeerProfile(collides)._tag).toBe("PeerProfileRejected");
    }
  });

  it("rejects other contract versions and malformed roots", () => {
    const future = { ...clone(SYNTHETIC_STABLE_PROFILE), contractVersion: 2 };
    expectRejected(future, "unsupported_contract_version", "contractVersion");
    for (const junk of [null, undefined, 42, "profile", [], () => {}]) {
      expect(parseAcpTrustedPeerProfile(junk)._tag).toBe("PeerProfileRejected");
    }
  });

  it("never throws and never accepts junk or metacharacter-mutated profiles (property)", () => {
    fc.assert(
      fc.property(fc.anything(), (value) => {
        const parsed = parseAcpTrustedPeerProfile(value);
        return parsed._tag === "PeerProfileRejected";
      }),
      { numRuns: 200 },
    );
    const metacharacters = fc.constantFrom(";", "|", "&", "$", "`", '"', "'", " ", "\n", ">", "<");
    fc.assert(
      fc.property(metacharacters, fc.nat(2), (char, slot) => {
        const candidate = clone(SYNTHETIC_STABLE_PROFILE);
        if (slot === 0) candidate.launch.executable = `synthetic${char}agent`;
        else if (slot === 1) candidate.launch.args = [`acp${char}stdio`];
        else candidate.launch.versionProbeArgs = [`--ver${char}sion`];
        return parseAcpTrustedPeerProfile(candidate)._tag === "PeerProfileRejected";
      }),
      { numRuns: 100 },
    );
  });
});

describe("trusted registry and official-registry discovery", () => {
  it("builds the default registry from the two reference profiles", () => {
    const result = createDefaultAcpTrustedPeerProfileRegistry();
    expect(result._tag).toBe("RegistryReady");
    if (result._tag === "RegistryReady") {
      expect([...result.registry.profiles.keys()].toSorted()).toEqual(["cursor-agent", "grok-cli"]);
    }
  });

  it("rejects duplicate profile ids and cross-profile extension collisions", () => {
    const duplicate = createAcpTrustedPeerProfileRegistry([
      GROK_TRUSTED_PEER_PROFILE,
      GROK_TRUSTED_PEER_PROFILE,
    ]);
    expect(duplicate).toMatchObject({ _tag: "RegistryRejected", reason: "duplicate_profile_id" });
    const rival = clone(SYNTHETIC_STABLE_PROFILE);
    rival.extensions = [
      {
        method: GROK_ACP_PROFILE.methods[0].method,
        direction: "agent-to-client",
        kind: "request",
        extensionProfileVersion: 1,
      },
    ] as never;
    const collision = createAcpTrustedPeerProfileRegistry([GROK_TRUSTED_PEER_PROFILE, rival]);
    expect(collision).toMatchObject({
      _tag: "RegistryRejected",
      reason: "extension_namespace_collision",
    });
  });

  it("surfaces the inner profile rejection when a candidate is invalid", () => {
    const bad = clone(SYNTHETIC_STABLE_PROFILE);
    bad.launch.executable = "rm -rf /";
    const result = createAcpTrustedPeerProfileRegistry([bad]);
    expect(result).toMatchObject({
      _tag: "RegistryRejected",
      reason: "profile_rejected",
      profileRejection: { reason: "shell_string_rejected" },
    });
  });

  it("ingests an official snapshot deterministically as discovery metadata only", () => {
    const raw = JSON.stringify({
      version: 1,
      agents: [
        { id: "zed-industries", name: "Zed", distribution: { npm: {} } },
        { id: "cursor", name: "Cursor", description: "Cursor Agent CLI" },
        { id: "cursor", name: "Cursor duplicate row" },
        { name: "Codex CLI" },
      ],
    });
    const result = ingestOfficialAcpRegistrySnapshot({ source: "registry-v1.json", rawJson: raw });
    expect(result._tag).toBe("RegistrySnapshotReady");
    if (result._tag !== "RegistrySnapshotReady") return;
    const { snapshot } = result;
    expect(snapshot.authority).toBe("discovery-metadata-only");
    expect(snapshot.snapshotSha256).toBe(createHash("sha256").update(raw, "utf8").digest("hex"));
    expect(snapshot.entries.map((entry) => entry.entryId)).toEqual([
      "codex-cli",
      "cursor",
      "zed-industries",
    ]);
    expect(snapshot.droppedDuplicateEntryIds).toEqual(["cursor"]);
    // A discovery entry can never carry launch authority: no executable,
    // argv, environment, installer, or extension fields exist at all.
    for (const entry of snapshot.entries) {
      expect(Object.keys(entry).toSorted()).toEqual([
        "authority",
        "description",
        "displayName",
        "distributionKinds",
        "entryId",
      ]);
      expect(Object.isFrozen(entry)).toBe(true);
    }
    // Determinism: re-ingesting yields an identical projection.
    const again = ingestOfficialAcpRegistrySnapshot({ source: "registry-v1.json", rawJson: raw });
    expect(again).toEqual(result);
  });

  it("enforces snapshot pinning and bounds", () => {
    const raw = JSON.stringify({ version: 1, agents: [{ name: "Cursor" }] });
    expect(
      ingestOfficialAcpRegistrySnapshot({
        source: "registry-v1.json",
        rawJson: raw,
        expectedSha256: "b".repeat(64),
      }),
    ).toMatchObject({ _tag: "RegistrySnapshotRejected", reason: "digest_mismatch" });
    expect(
      ingestOfficialAcpRegistrySnapshot({
        source: "registry-v1.json",
        rawJson: `{"agents":[],"pad":"${"x".repeat(300_000)}"}`,
      }),
    ).toMatchObject({ _tag: "RegistrySnapshotRejected", reason: "oversized_snapshot" });
    const tooMany = JSON.stringify({
      agents: Array.from({ length: 200 }, (_, i) => ({ name: `Agent ${String(i)}` })),
    });
    expect(
      ingestOfficialAcpRegistrySnapshot({ source: "registry-v1.json", rawJson: tooMany }),
    ).toMatchObject({ _tag: "RegistrySnapshotRejected", reason: "entry_limit_exceeded" });
    for (const malformed of [
      "not json",
      "[]",
      '{"agents":[{"noName":true}]}',
      '{"agents":[null]}',
    ]) {
      expect(
        ingestOfficialAcpRegistrySnapshot({ source: "registry-v1.json", rawJson: malformed }),
      ).toMatchObject({ _tag: "RegistrySnapshotRejected", reason: "malformed_snapshot" });
    }
  });

  it("resolves discovery to launch authority only through a trusted profile", () => {
    const registry = registryWithSynthetic();
    const raw = JSON.stringify({
      agents: [
        { id: "cursor", name: "Cursor" },
        { id: "codex", name: "Codex CLI" },
      ],
    });
    const result = ingestOfficialAcpRegistrySnapshot({ source: "registry-v1.json", rawJson: raw });
    if (result._tag !== "RegistrySnapshotReady") throw new Error("snapshot must ingest");
    const [codex, cursor] = result.snapshot.entries;
    expect(resolveDiscoveryEntryToTrustedProfile(registry, cursor!)?.profileId).toBe(
      "cursor-agent",
    );
    expect(resolveDiscoveryEntryToTrustedProfile(registry, codex!)).toBeUndefined();
    expect(resolveAcpTrustedLaunchPlan(registry, "codex")).toBeUndefined();
  });
});

describe("executable trust and support-state derivation", () => {
  const profile = (() => {
    const parsed = parseAcpTrustedPeerProfile(SYNTHETIC_STABLE_PROFILE);
    if (parsed._tag !== "PeerProfileParsed") throw new Error("fixture must parse");
    return parsed.profile;
  })();

  it("extracts pinned versions and refuses unparsable ones", () => {
    expect(extractLeadingSemver("synthetic-agent 1.2.3 (release)")).toBe("1.2.3");
    expect(extractLeadingSemver("2026.06.24-00-45-58-build")).toBe("2026.6.24");
    expect(extractLeadingSemver("dev build")).toBeUndefined();
    const trust = evaluateAcpExecutableTrust({
      profile,
      probe: syntheticProbe({ reportedVersion: "nightly" }),
    });
    expect(trust).toMatchObject({ _tag: "ExecutableRejected", reason: "version_unknown" });
  });

  it("detects identity mismatch, platform mismatch, and denied versions", () => {
    expect(
      evaluateAcpExecutableTrust({
        profile,
        probe: syntheticProbe({ resolvedPath: "/opt/tools/bin/impostor" }),
      }),
    ).toMatchObject({ _tag: "ExecutableRejected", reason: "identity_mismatch" });
    expect(
      evaluateAcpExecutableTrust({
        profile,
        probe: syntheticProbe({ requestedExecutable: "other-binary" }),
      }),
    ).toMatchObject({ _tag: "ExecutableRejected", reason: "identity_mismatch" });
    expect(
      evaluateAcpExecutableTrust({
        profile,
        probe: syntheticProbe({ platform: { os: "win32", arch: "x64" } }),
      }),
    ).toMatchObject({ _tag: "ExecutableRejected", reason: "platform_unsupported" });
    expect(
      evaluateAcpExecutableTrust({
        profile,
        probe: syntheticProbe({ reportedVersion: "0.1.0" }),
      }),
    ).toMatchObject({ _tag: "ExecutableRejected", reason: "version_denied" });
  });

  it("detects PATH shadowing, symlink retargeting, and post-install replacement via the identity pin", () => {
    const first = evaluateAcpExecutableTrust({ profile, probe: syntheticProbe() });
    expect(first._tag).toBe("ExecutableTrusted");
    if (first._tag !== "ExecutableTrusted") return;
    // Same pin, same binary: still trusted.
    expect(
      evaluateAcpExecutableTrust({ profile, probe: syntheticProbe(), priorPin: first.pin }),
    ).toMatchObject({ _tag: "ExecutableTrusted" });
    // Replaced content (post-install swap) with the same path.
    expect(
      evaluateAcpExecutableTrust({
        profile,
        probe: syntheticProbe({ sha256: "f".repeat(64) }),
        priorPin: first.pin,
      }),
    ).toMatchObject({ _tag: "ExecutableRejected", reason: "path_replacement" });
    // Symlink or PATH now resolves somewhere else.
    expect(
      evaluateAcpExecutableTrust({
        profile,
        probe: syntheticProbe({ realPath: "/tmp/shadow/synthetic.mjs" }),
        priorPin: first.pin,
      }),
    ).toMatchObject({ _tag: "ExecutableRejected", reason: "path_replacement" });
  });

  it("derives support state from profile plus evidence, never name or wire version alone", () => {
    const base = {
      profile,
      peerVersion: "1.2.3",
      executableSha256: SYNTHETIC_SHA,
      platform: { os: "darwin", arch: "arm64" },
      now: NOW,
    };
    expect(deriveAcpSupportState({ ...base, evidence: fullEvidence })).toBe("supported");
    // No evidence: a supported-range version is still only experimental.
    expect(deriveAcpSupportState({ ...base, evidence: [] })).toBe("experimental");
    // Live evidence bound to a different binary digest does not count.
    expect(
      deriveAcpSupportState({
        ...base,
        evidence: [
          syntheticEvidence(),
          syntheticEvidence({
            suiteId: "acp-release-matrix-v1",
            kind: "live",
            executableSha256: "c".repeat(64),
            platform: { os: "darwin", arch: "arm64" },
          }),
        ],
      }),
    ).toBe("experimental");
    // A passing live receipt is not a release gate unless it represents the
    // complete code-owned matrix on this exact platform.
    expect(
      deriveAcpSupportState({
        ...base,
        evidence: [
          syntheticEvidence(),
          syntheticEvidence({
            kind: "live",
            executableSha256: SYNTHETIC_SHA,
            platform: { os: "darwin", arch: "arm64" },
          }),
        ],
      }),
    ).toBe("experimental");
    expect(
      deriveAcpSupportState({
        ...base,
        evidence: fullEvidence.map((record) =>
          record.kind === "live"
            ? { ...record, platform: { os: "linux", arch: "arm64" } }
            : record,
        ),
      }),
    ).toBe("experimental");
    // Stale evidence does not count.
    expect(
      deriveAcpSupportState({
        ...base,
        evidence: [
          syntheticEvidence({ recordedAt: "2026-05-01T00:00:00.000Z" }),
          syntheticEvidence({
            suiteId: "acp-release-matrix-v1",
            kind: "live",
            executableSha256: SYNTHETIC_SHA,
            platform: { os: "darwin", arch: "arm64" },
            recordedAt: "2026-05-01T00:00:00.000Z",
          }),
        ],
      }),
    ).toBe("experimental");
    // A failing record for this version makes the peer incompatible.
    expect(
      deriveAcpSupportState({
        ...base,
        evidence: [...fullEvidence, syntheticEvidence({ result: "fail" })],
      }),
    ).toBe("incompatible");
    // Versions outside every declared range are incompatible.
    expect(deriveAcpSupportState({ ...base, peerVersion: "9.9.9", evidence: [] })).toBe(
      "incompatible",
    );
    // Experimental-range versions stay experimental even with full evidence.
    expect(
      deriveAcpSupportState({
        ...base,
        peerVersion: "0.9.5",
        evidence: fullEvidence.map((record) => ({ ...record, peerVersion: "0.9.5" })),
      }),
    ).toBe("experimental");
  });
});

describe("peer admission", () => {
  const registry = registryWithSynthetic();

  it("admits the synthetic stable-only peer declaratively with a registry-owned launch plan", () => {
    const decision = admitAcpPeerProfile({
      registry,
      profileId: "synthetic-stable",
      probe: syntheticProbe(),
      evidence: fullEvidence,
      now: NOW,
      observedAgentName: "synthetic",
    });
    expect(decision._tag).toBe("PeerAdmitted");
    if (decision._tag !== "PeerAdmitted") return;
    expect(decision.supportState).toBe("supported");
    expect(decision.launchPlan).toMatchObject({
      source: "trusted-peer-profile-registry",
      executable: "synthetic-agent",
      args: ["acp-stdio"],
      allowedEnvKeys: ["SYNTHETIC_API_KEY", "SYNTHETIC_HOME"],
      requiredEnvKeys: ["SYNTHETIC_API_KEY"],
    });
    expect(Object.isFrozen(decision.launchPlan)).toBe(true);
    expect(Object.isFrozen(decision.launchPlan.args)).toBe(true);
    expect(decision.grants).toMatchObject({
      fsReadTextFile: true,
      fsWriteTextFile: false,
      terminal: false,
      permissionAutoApproval: false,
      network: false,
      vendorExtensionMethods: [],
    });
    expect(decision.identityPin.sha256).toBe(SYNTHETIC_SHA);
  });

  it("refuses caller-supplied launch overrides outright", () => {
    for (const requestedLaunchOverride of [
      { executable: "/tmp/evil" },
      { args: ["--dangerously-skip-checks"] },
      { env: { LD_PRELOAD: "/tmp/evil.so" } },
      {},
    ]) {
      expect(
        admitAcpPeerProfile({
          registry,
          profileId: "synthetic-stable",
          probe: syntheticProbe(),
          evidence: fullEvidence,
          now: NOW,
          requestedLaunchOverride,
        }),
      ).toMatchObject({
        _tag: "PeerAdmissionRefused",
        reason: "caller_launch_override_rejected",
      });
    }
  });

  it("refuses unknown profiles and identity lies", () => {
    expect(
      admitAcpPeerProfile({
        registry,
        profileId: "not-registered",
        probe: syntheticProbe(),
        evidence: [],
        now: NOW,
      }),
    ).toMatchObject({ _tag: "PeerAdmissionRefused", reason: "unknown_profile" });
    expect(
      admitAcpPeerProfile({
        registry,
        profileId: "synthetic-stable",
        probe: syntheticProbe(),
        evidence: fullEvidence,
        now: NOW,
        observedAgentName: "impostor-agent",
      }),
    ).toMatchObject({ _tag: "PeerAdmissionRefused", reason: "identity_mismatch" });
  });

  it("refuses replaced executables before a session starts", () => {
    const first = admitAcpPeerProfile({
      registry,
      profileId: "synthetic-stable",
      probe: syntheticProbe(),
      evidence: fullEvidence,
      now: NOW,
    });
    if (first._tag !== "PeerAdmitted") throw new Error("first admission must pass");
    expect(
      admitAcpPeerProfile({
        registry,
        profileId: "synthetic-stable",
        probe: syntheticProbe({ sha256: "e".repeat(64) }),
        evidence: fullEvidence,
        now: NOW,
        priorPin: first.identityPin,
      }),
    ).toMatchObject({ _tag: "PeerAdmissionRefused", reason: "path_replacement" });
    const closureFirst = admitAcpPeerProfile({
      registry,
      profileId: "synthetic-stable",
      probe: syntheticProbe({ closureSha256: "a".repeat(64) }),
      evidence: fullEvidence,
      now: NOW,
    });
    if (closureFirst._tag !== "PeerAdmitted") throw new Error("closure admission must pass");
    expect(
      admitAcpPeerProfile({
        registry,
        profileId: "synthetic-stable",
        probe: syntheticProbe({ closureSha256: "b".repeat(64) }),
        evidence: fullEvidence,
        now: NOW,
        priorPin: closureFirst.identityPin,
      }),
    ).toMatchObject({ _tag: "PeerAdmissionRefused", reason: "path_replacement" });
  });

  it("quarantines capability lies and undeclared extension methods instead of granting them", () => {
    const decision = admitAcpPeerProfile({
      registry,
      profileId: "synthetic-stable",
      probe: syntheticProbe(),
      evidence: fullEvidence,
      now: NOW,
      observedAgentCapabilityKeys: ["prompt.text", "terminal.superuser", "fs.deleteTree"],
      observedExtensionMethods: ["vendor.rogue/exfiltrate"],
    });
    expect(decision._tag).toBe("PeerAdmitted");
    if (decision._tag !== "PeerAdmitted") return;
    expect(decision.quarantinedCapabilities).toEqual(["terminal.superuser", "fs.deleteTree"]);
    expect(decision.quarantinedExtensionMethods).toEqual(["vendor.rogue/exfiltrate"]);
    expect(decision.grants.terminal).toBe(false);
    expect(decision.grants.vendorExtensionMethods).not.toContain("vendor.rogue/exfiltrate");
  });

  it("admits pinned Grok and Cursor builds as experimental until live evidence exists", () => {
    const grok = admitAcpPeerProfile({
      registry,
      profileId: "grok-cli",
      probe: syntheticProbe({
        requestedExecutable: "grok",
        resolvedPath: "/opt/tools/bin/grok",
        reportedVersion: "grok 0.2.102",
      }),
      evidence: [],
      now: NOW,
      observedAgentName: "grok",
    });
    expect(grok).toMatchObject({ _tag: "PeerAdmitted", supportState: "experimental" });
    if (grok._tag === "PeerAdmitted") {
      // Experimental admission keeps risky grants and vendor extensions off.
      expect(grok.grants).toMatchObject({
        fsReadTextFile: false,
        fsWriteTextFile: false,
        terminal: false,
        permissionAutoApproval: false,
        network: false,
        vendorExtensionMethods: [],
      });
      expect(grok.launchPlan.args).toEqual(["agent", "stdio"]);
    }
    const cursor = admitAcpPeerProfile({
      registry,
      profileId: "cursor-agent",
      probe: syntheticProbe({
        requestedExecutable: "agent",
        resolvedPath: "/opt/cursor/cursor-agent",
        realPath: "/opt/cursor/cursor-agent",
        reportedVersion: "2026.06.24-00-45-58-9f61de7",
      }),
      evidence: [],
      now: NOW,
      observedAgentName: "cursor-agent",
    });
    expect(cursor).toMatchObject({ _tag: "PeerAdmitted", supportState: "experimental" });
    if (cursor._tag === "PeerAdmitted") expect(cursor.launchPlan.args).toEqual(["acp"]);
  });

  it("exposes sanitized profile/registry/artifact identity in diagnostics", () => {
    const decision = admitAcpPeerProfile({
      registry,
      profileId: "synthetic-stable",
      probe: syntheticProbe(),
      evidence: fullEvidence,
      now: NOW,
    });
    if (decision._tag !== "PeerAdmitted") throw new Error("admission must pass");
    expect(decision.diagnostics).toMatchObject({
      profileId: "synthetic-stable",
      providerId: "synthetic",
      profileRevision: 3,
      contractVersion: 1,
      schemaRelease: "schema-v1.19.0",
      supportState: "supported",
      peerVersion: "1.2.3",
      executableBasename: "synthetic-agent",
      executableSha256: SYNTHETIC_SHA,
      evidenceArtifactRefs: [
        "compatibility/synthetic-fixture.json",
        "compatibility/synthetic-live.json",
      ],
    });
    const rendered = JSON.stringify(decision.diagnostics);
    // Sanitized: no filesystem paths and no environment values leak.
    expect(rendered).not.toContain("/opt/tools");
    expect(rendered).not.toContain("SYNTHETIC_API_KEY");
  });

  it("builds launch environments only from allowlisted keys and fails closed on missing secrets", () => {
    const plan = resolveAcpTrustedLaunchPlan(registry, "synthetic-stable");
    if (plan === undefined) throw new Error("plan must resolve");
    const ready = buildAdmittedLaunchEnvironment(plan, {
      SYNTHETIC_API_KEY: "value-under-test",
      SYNTHETIC_HOME: "/home/synthetic",
      PATH: "/tmp/shadow:/usr/bin",
      LD_PRELOAD: "/tmp/evil.so",
      NODE_OPTIONS: "--require /tmp/evil.js",
    });
    expect(ready._tag).toBe("LaunchEnvironmentReady");
    if (ready._tag === "LaunchEnvironmentReady") {
      expect(Object.keys(ready.env).toSorted()).toEqual(["SYNTHETIC_API_KEY", "SYNTHETIC_HOME"]);
    }
    expect(
      buildAdmittedLaunchEnvironment(plan, { SYNTHETIC_HOME: "/home/synthetic" }),
    ).toMatchObject({
      _tag: "LaunchEnvironmentRejected",
      reason: "missing_required_secret",
    });
  });

  it("defaults every risky capability to disabled for unknown peers", () => {
    const refused = admitUnknownAcpPeerExperimental({
      acknowledgement: "yes please",
      probe: syntheticProbe(),
    });
    expect(refused).toMatchObject({
      _tag: "PeerAdmissionRefused",
      reason: "experimental_acknowledgement_required",
    });
    const admitted = admitUnknownAcpPeerExperimental({
      acknowledgement: ACP_UNKNOWN_PEER_ACKNOWLEDGEMENT,
      probe: syntheticProbe(),
    });
    expect(admitted).toMatchObject({
      _tag: "UnknownPeerExperimentalAdmission",
      supportState: "experimental-unsupported",
      grants: {
        fsReadTextFile: false,
        fsWriteTextFile: false,
        terminal: false,
        permissionAutoApproval: false,
        vendorExtensionMethods: [],
        network: false,
      },
      limits: { maxSessions: 1 },
    });
  });

  it("keeps the shared schema/registry/admission layers free of provider conditionals", () => {
    for (const file of ["schema.ts", "registry.ts", "admission.ts"]) {
      const source = readFileSync(resolve(import.meta.dirname, file), "utf8");
      expect(source).not.toMatch(/grok/i);
      expect(source).not.toMatch(/cursor/i);
      expect(source).not.toMatch(/x\.ai/i);
    }
  });
});

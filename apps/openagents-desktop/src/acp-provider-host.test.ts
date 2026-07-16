import { describe, expect, test } from "vite-plus/test";

import { createAcpProviderHost } from "./acp-provider-host.ts";

const probe = (provider: "grok" | "cursor") =>
  ({
    requestedExecutable: provider === "grok" ? "grok" : "agent",
    resolvedPath: provider === "grok" ? "/secret/bin/grok" : "/secret/cursor/cursor-agent",
    realPath: provider === "grok" ? "/secret/bin/grok" : "/secret/cursor/cursor-agent",
    sha256: "a".repeat(64),
    reportedVersion: provider === "grok" ? "0.2.101" : "2026.6.24",
    platform: { os: "darwin", arch: "arm64" },
  }) as any;

const admission = (provider: "grok" | "cursor") =>
  ({
    _tag: "PeerAdmitted",
    profileId: provider === "grok" ? "grok-cli" : "cursor-agent",
    supportState: provider === "grok" ? "supported" : "experimental",
    peerVersion: provider === "grok" ? "0.2.101" : "2026.6.24",
    launchPlan: {},
    grants: {},
    identityPin: {},
    quarantinedCapabilities: [],
    quarantinedExtensionMethods: [],
    diagnostics: {
      evidenceArtifactRefs: [`acp.${provider}.matrix`],
      schemaRelease: "schema-v1.19.0",
    },
  }) as any;

const fakeRuntime = (provider: "grok" | "cursor") => {
  const sessions: any[] = [];
  const receipts: any[] = [];
  const evidence = {
    schemaRelease: "schema-v1.19.0",
    wireVersion: 1,
    runtimeGeneration: 1,
    connectionRef: `connection.${provider}.1`,
    profile: provider,
    peer: { name: provider, version: provider === "grok" ? "0.2.101" : "2026.6.24" },
    capabilities: {
      load: provider === "grok",
      list: provider === "grok",
      delete: false,
      resume: false,
      close: false,
      logout: false,
      fork: false,
    },
    authMethodIds: provider === "grok" ? ["cached_token", "xai.api_key"] : ["cursor_login"],
    extensionMethods: [],
  };
  return {
    evidence: () => evidence,
    receipts: () => receipts,
    sessions: () => sessions,
    start: async () => {
      receipts.push({ method: "initialize" });
      return { ok: true, value: evidence, receipt: { evidenceRefs: [] } };
    },
    newSession: async ({ canonicalThreadSeed }: any) => {
      const session = {
        threadId: canonicalThreadSeed,
        peerSessionId: `peer.${provider}.1`,
        runtimeGeneration: 1,
        sessionGeneration: 1,
        phase: "live",
        configOptions: [],
        promptActive: false,
      };
      sessions.push(session);
      receipts.push({ method: "session/new" });
      return { ok: true, value: session, receipt: { evidenceRefs: [] } };
    },
    logout: async () => ({ ok: false, reason: "unsupported", receipt: { evidenceRefs: [] } }),
    cancel: async () => ({ ok: true, value: undefined, receipt: { evidenceRefs: [] } }),
    recover: async () => ({ ok: false, reason: "missing_session", receipt: { evidenceRefs: [] } }),
    shutdown: async () => undefined,
  } as any;
};

describe("main-owned ACP provider host", () => {
  test("drives distinct clean-machine probe, auth, and session flows without renderer authority", async () => {
    const host = createAcpProviderHost({
      cwd: async () => "/workspace",
      now: () => new Date("2026-07-16T12:00:00.000Z"),
      probeGrok: async () => probe("grok"),
      probeCursor: async () => probe("cursor"),
      admitGrok: async () => admission("grok"),
      admitCursor: async () => admission("cursor"),
      createGrok: async () => fakeRuntime("grok"),
      createCursor: async (_cwd, _admission, onAuth) => {
        onAuth("pending");
        return fakeRuntime("cursor");
      },
    });
    const detected = await host.initialize();
    expect(
      detected.providers.map((provider) => [
        provider.provider,
        provider.install,
        provider.profileState,
      ]),
    ).toEqual([
      ["grok", "detected", "supported"],
      ["cursor", "detected", "experimental"],
    ]);
    const authenticated = await host.action("cursor", "authenticate");
    expect(authenticated.providers[1]?.auth).toMatchObject({
      state: "authenticated",
      advertisedMethods: ["cursor_login"],
    });
    const started = await host.action("cursor", "new_session");
    expect(started.providers[1]?.session).toMatchObject({
      state: "running",
      processRef: "connection.cursor.1",
    });
  });

  test("reports missing binaries honestly and exports only the closed main-owned support artifact", async () => {
    const host = createAcpProviderHost({
      cwd: async () => "/workspace",
      probeGrok: async () => {
        throw Object.assign(new Error("missing"), { kind: "missing_executable" });
      },
      probeCursor: async () => probe("cursor"),
      admitCursor: async () => admission("cursor"),
    });
    const status = await host.initialize();
    expect(status.providers[0]).toMatchObject({
      install: "not_installed",
      probe: { state: "failed", code: "missing_executable" },
    });
    const serialized = JSON.stringify(host.supportBundle());
    expect(serialized).toContain("schema-v1.19.0");
    expect(serialized).not.toContain("/secret/");
    expect(serialized).not.toContain("cached_token");
    expect(serialized).not.toContain("XAI_API_KEY");
    expect(serialized).not.toContain("cursor_login");
  });
});

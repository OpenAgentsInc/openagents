import { describe, expect, test } from "vite-plus/test";

import { createAcpProviderHost, defaultGrokDesktopAuthOptions } from "./acp-provider-host.ts";

const probe = (provider: "grok" | "cursor", overrides: Record<string, unknown> = {}) =>
  ({
    requestedExecutable: provider === "grok" ? "grok" : "agent",
    resolvedPath: provider === "grok" ? "/secret/bin/grok" : "/secret/cursor/cursor-agent",
    realPath: provider === "grok" ? "/secret/bin/grok" : "/secret/cursor/cursor-agent",
    sha256:
      provider === "grok"
        ? "8431538dbd99379240f558b48b779c651d668b06d793c87311ad532c4395a4e2"
        : "b7babf47d8b1eee28ac27a74affa02a559bb38103a6e71fbb1f120805d51fedf",
    ...(provider === "cursor"
      ? {
          closureSha256: "69d078daa4db8cbb4163ce2f010207553efb06d652c1e1ea421d739795532faa",
        }
      : {}),
    reportedVersion:
      provider === "grok" ? "grok 0.2.101 (5bc4b5dfadcf)" : "2026.06.24-00-45-58-9f61de7",
    platform: { os: "darwin", arch: "arm64" },
    ...overrides,
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

const fakeRuntime = (provider: "grok" | "cursor", calls: string[] = []) => {
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
    prompt: async (_sessionRef: string, blocks: ReadonlyArray<{ text?: string }>) => {
      calls.push(`prompt:${blocks[0]?.text ?? ""}`);
      return {
        ok: true,
        value: { stopReason: "end_turn", terminal: "completed" },
        receipt: { evidenceRefs: [] },
      };
    },
    logout: async () => ({ ok: false, reason: "unsupported", receipt: { evidenceRefs: [] } }),
    cancel: async () => ({ ok: true, value: undefined, receipt: { evidenceRefs: [] } }),
    recover: async () => ({ ok: false, reason: "missing_session", receipt: { evidenceRefs: [] } }),
    shutdown: async () => undefined,
  } as any;
};

describe("main-owned ACP provider host", () => {
  test("uses existing Grok login without promoting an ambient API key to configuration", () => {
    expect(
      defaultGrokDesktopAuthOptions({
        HOME: "/ordinary/home",
        XAI_API_KEY: "ambient-must-not-select-api-key",
      }),
    ).toEqual({ environment: { HOME: "/ordinary/home" }, apiKeyConfigured: false });
  });

  test("binds the complete checked release matrix into exact shipped peer admission", async () => {
    const host = createAcpProviderHost({
      cwd: async () => "/workspace",
      now: () => new Date("2026-07-17T12:00:00.000Z"),
      probeGrok: async () => probe("grok"),
      probeCursor: async () => probe("cursor"),
    });
    const status = await host.initialize();
    expect(status.providers.map((provider) => [provider.provider, provider.profileState])).toEqual([
      ["grok", "supported"],
      ["cursor", "supported"],
    ]);
    expect(
      status.providers.every((provider) =>
        provider.conformanceRef?.endsWith("release-matrix.json"),
      ),
    ).toBe(true);
  });

  test("keeps a substituted executable experimental even when the checked matrix is complete", async () => {
    const host = createAcpProviderHost({
      cwd: async () => "/workspace",
      now: () => new Date("2026-07-17T12:00:00.000Z"),
      probeGrok: async () => probe("grok", { sha256: "f".repeat(64) }),
      probeCursor: async () => probe("cursor"),
    });
    const status = await host.initialize();
    expect(status.providers.map((provider) => [provider.provider, provider.profileState])).toEqual([
      ["grok", "experimental"],
      ["cursor", "supported"],
    ]);
  });

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

  test("drives a canonical ProviderLane turn through the admitted peer runtime", async () => {
    const calls: string[] = [];
    const host = createAcpProviderHost({
      cwd: async () => "/workspace",
      now: () => new Date("2026-07-16T12:00:00.000Z"),
      probeGrok: async () => probe("grok"),
      probeCursor: async () => probe("cursor"),
      admitGrok: async () => admission("grok"),
      admitCursor: async () => admission("cursor"),
      createGrok: async () => fakeRuntime("grok", calls),
      createCursor: async () => fakeRuntime("cursor"),
    });
    const events: string[] = [];
    const result = await host.driver("grok").runTurn({
      threadRef: "thread-1",
      turnRef: "turn-1",
      model: "grok-default",
      history: [],
      message: "continue",
      background: true,
      emit: (event) => events.push(event.kind),
    });
    expect(result).toMatchObject({ ok: true, providerSessionRef: "peer.grok.1" });
    expect(events).toEqual(["turn.started", "raw.sidecar_ref", "turn.finished"]);
    expect(calls).toEqual(["prompt:continue"]);
  });
});

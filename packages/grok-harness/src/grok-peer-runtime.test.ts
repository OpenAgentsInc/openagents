import { describe, expect, it } from "vite-plus/test";

import type { AgentStdioReverseHandler } from "@openagentsinc/agent-stdio-transport";

import { createGrokAcpPeerRuntime, type GrokAcpTransport } from "./grok-peer-runtime.ts";

type NotificationHandler = (params: unknown) => void;

class FakeGrokTransport implements GrokAcpTransport {
  readonly generation = 7;
  state = "running";
  readonly requests: Array<{ method: string; params: unknown }> = [];
  readonly notifications: Array<{ method: string; params: unknown }> = [];
  readonly responders = new Map<string, unknown[]>();
  readonly notificationHandlers = new Map<string, Set<NotificationHandler>>();
  readonly reverseHandlers = new Map<string, AgentStdioReverseHandler>();
  onNotify: ((method: string, params: unknown) => void) | undefined;
  #exit!: () => void;
  readonly exited = new Promise<void>((resolve) => {
    this.#exit = resolve;
  });

  respond(method: string, value: unknown): void {
    const queue = this.responders.get(method) ?? [];
    queue.push(value);
    this.responders.set(method, queue);
  }
  async request(method: string, params?: unknown): Promise<unknown> {
    this.requests.push({ method, params: structuredClone(params) });
    const value = this.responders.get(method)?.shift();
    if (typeof value === "function") return value();
    if (value === undefined) throw new Error(`missing fake response for ${method}`);
    return structuredClone(value);
  }
  notify(method: string, params?: unknown): void {
    this.notifications.push({ method, params: structuredClone(params) });
    this.onNotify?.(method, params);
  }
  onNotification(method: string, handler: NotificationHandler): () => void {
    const handlers = this.notificationHandlers.get(method) ?? new Set();
    handlers.add(handler);
    this.notificationHandlers.set(method, handlers);
    return () => handlers.delete(handler);
  }
  emit(method: string, params: unknown): void {
    for (const handler of this.notificationHandlers.get(method) ?? []) handler(params);
  }
  registerReverseHandler(method: string, handler: AgentStdioReverseHandler): () => void {
    this.reverseHandlers.set(method, handler);
    return () => this.reverseHandlers.delete(method);
  }
  cancelReverseRequests(): number {
    return 0;
  }
  async drainAcceptedInbound(): Promise<void> {
    await Promise.resolve();
  }
  waitForExit(): Promise<void> {
    return this.exited;
  }
  async shutdown(): Promise<void> {
    this.state = "exited";
    this.#exit();
  }
  async dispose(): Promise<void> {
    this.state = "disposed";
    this.#exit();
  }
}

const digest = "8".repeat(64);
const waitForPromptCount = async (transport: FakeGrokTransport, count: number): Promise<void> => {
  const deadline = Date.now() + 1_000;
  while (
    transport.requests.filter((request) => request.method === "session/prompt").length < count
  ) {
    if (Date.now() >= deadline) throw new Error("prompt did not start");
    await new Promise((resolve) => setTimeout(resolve, 1));
  }
};
const evidence = [
  {
    suiteId: "acp-wire-v1-conformance",
    kind: "fixture" as const,
    result: "pass" as const,
    peerVersion: "0.2.101",
    recordedAt: "2026-07-16T12:00:00.000Z",
    artifactRef: "fixtures/grok-0.2.101.json",
  },
  {
    suiteId: "grok-live-profile",
    kind: "live" as const,
    result: "pass" as const,
    peerVersion: "0.2.101",
    executableSha256: digest,
    recordedAt: "2026-07-16T12:00:00.000Z",
    artifactRef: "live/grok-0.2.101.json",
  },
  ...["grok-api-key-auth", "grok-authority-reverse", "grok-question-extensions"].map((suiteId) => ({
    suiteId,
    kind: "live" as const,
    result: "pass" as const,
    peerVersion: "0.2.101",
    executableSha256: digest,
    recordedAt: "2026-07-16T12:00:00.000Z",
    artifactRef: `live/${suiteId}.json`,
  })),
];

describe("Grok admitted ACP peer runtime", () => {
  it.each(["grok.com", "oidc"] as const)(
    "fails closed when the owner cancels %s interactive login",
    async (methodId) => {
      const transport = new FakeGrokTransport();
      transport.respond("initialize", {
        protocolVersion: 1,
        agentInfo: { name: "grok", version: "0.2.101" },
        agentCapabilities: { loadSession: true, sessionCapabilities: {} },
        authMethods: [{ id: methodId, name: methodId === "oidc" ? "Company SSO" : "Grok" }],
      });
      let decisions = 0;
      const peer = await createGrokAcpPeerRuntime({
        cwd: process.cwd(),
        environment: { HOME: "/tmp/grok-home" },
        evidence,
        now: new Date("2026-07-16T12:00:00.000Z"),
        probe: {
          requestedExecutable: "grok",
          resolvedPath: "/opt/bin/grok",
          realPath: "/opt/grok/grok-0.2.101",
          sha256: digest,
          reportedVersion: "grok 0.2.101 (fixture)",
          platform: { os: process.platform, arch: process.arch },
        },
        authorizeLogin: async (interaction) => {
          decisions += 1;
          expect(interaction).toMatchObject({
            methodId,
            kind: "external-browser",
            state: "login-required",
          });
          return "cancel";
        },
        createTransport: async () => transport,
      });

      await expect(peer.start()).resolves.toMatchObject({ ok: false, reason: "auth_required" });
      expect(decisions).toBe(1);
      expect(transport.requests.map((request) => request.method)).toEqual(["initialize"]);
      expect(transport.state).toBe("disposed");
    },
  );

  it("authenticates advertised grok.com only after explicit owner continuation", async () => {
    const transport = new FakeGrokTransport();
    transport.respond("initialize", {
      protocolVersion: 1,
      agentInfo: { name: "grok", version: "0.2.101" },
      agentCapabilities: { loadSession: true, sessionCapabilities: {} },
      authMethods: [{ id: "grok.com", name: "Grok" }],
    });
    transport.respond("authenticate", {});
    const peer = await createGrokAcpPeerRuntime({
      cwd: process.cwd(),
      environment: { HOME: "/tmp/grok-home" },
      evidence,
      now: new Date("2026-07-16T12:00:00.000Z"),
      probe: {
        requestedExecutable: "grok",
        resolvedPath: "/opt/bin/grok",
        realPath: "/opt/grok/grok-0.2.101",
        sha256: digest,
        reportedVersion: "grok 0.2.101 (fixture)",
        platform: { os: process.platform, arch: process.arch },
      },
      authorizeLogin: async () => "continue",
      createTransport: async () => transport,
    });

    await expect(peer.start()).resolves.toMatchObject({ ok: true });
    expect(transport.requests[1]).toEqual({
      method: "authenticate",
      params: { methodId: "grok.com", _meta: { headless: true } },
    });
    await peer.shutdown();
  });

  it.each([
    ["cached token", { XAI_API_KEY: "ambient-must-not-select" }, "cached_token"],
    ["intentional API key", { XAI_API_KEY: "not-a-real-secret" }, "xai.api_key"],
  ])(
    "negotiates %s and composes shared session/update/cancel mechanics",
    async (_name, env, authId) => {
      const transport = new FakeGrokTransport();
      transport.respond("initialize", {
        protocolVersion: 1,
        agentInfo: { name: "grok", version: "0.2.101" },
        agentCapabilities: { loadSession: true, sessionCapabilities: {} },
        authMethods: [
          { id: "cached_token", name: "Cached" },
          { id: "xai.api_key", name: "API key" },
        ],
      });
      transport.respond("authenticate", {});
      transport.respond("session/new", { sessionId: "grok-session-1" });
      transport.respond("session/prompt", () => {
        transport.emit("session/update", {
          sessionId: "grok-session-1",
          update: {
            sessionUpdate: "agent_message_chunk",
            content: { type: "text", text: "hello" },
          },
        });
        return { stopReason: "end_turn" };
      });
      const updates: unknown[] = [];
      let authorityInstalled = 0;
      let authorityDisposed = 0;
      const peer = await createGrokAcpPeerRuntime({
        cwd: process.cwd(),
        environment: { HOME: "/tmp/grok-home", ...env },
        apiKeyConfigured: authId === "xai.api_key",
        evidence,
        now: new Date("2026-07-16T12:00:00.000Z"),
        probe: {
          requestedExecutable: "grok",
          resolvedPath: "/opt/bin/grok",
          realPath: "/opt/grok/grok-0.2.101",
          sha256: digest,
          reportedVersion: "grok 0.2.101 (fixture)",
          platform: { os: process.platform, arch: process.arch },
        },
        authority: {
          install: (host) => {
            authorityInstalled += 1;
            const methods = [
              "session/request_permission",
              "fs/read_text_file",
              "fs/write_text_file",
              "terminal/create",
              "terminal/output",
              "terminal/release",
              "terminal/wait_for_exit",
              "terminal/kill",
            ];
            const dispose = methods.map((method) =>
              host.registerReverseHandler(method, async () => ({})),
            );
            return () => {
              for (const unregister of dispose) unregister();
              authorityDisposed += 1;
            };
          },
        },
        installVendorHandlers: (host) => {
          const methods = ["x.ai/ask_user_question", "_x.ai/ask_user_question"];
          const dispose = methods.map((method) =>
            host.registerReverseHandler(method, async () => ({})),
          );
          return () => {
            for (const unregister of dispose) unregister();
          };
        },
        createTransport: async () => transport,
        cancelGraceMs: 5,
        onUpdate: (record) => {
          updates.push(record);
        },
      });
      expect(peer.admission.supportState).toBe("supported");
      expect((await peer.start()).ok).toBe(true);
      const attached = await peer.newSession({ cwd: process.cwd(), canonicalThreadSeed: "thread" });
      expect(attached).toMatchObject({ ok: true, value: { peerSessionId: "grok-session-1" } });
      await expect(
        peer.prompt("grok-session-1", [{ type: "text", text: "Say hello" }]),
      ).resolves.toMatchObject({ ok: true, value: { stopReason: "end_turn" } });
      expect(authorityInstalled).toBe(1);
      expect(updates).toHaveLength(1);
      expect(transport.requests[0]).toMatchObject({
        method: "initialize",
        params: {
          protocolVersion: 1,
          clientCapabilities: {
            fs: { readTextFile: true, writeTextFile: true },
            terminal: true,
          },
        },
      });
      expect(transport.requests[1]).toMatchObject({
        method: "authenticate",
        params: { methodId: authId, _meta: { headless: true } },
      });
      expect(peer.evidence()).toMatchObject({
        peer: { name: "grok", version: "0.2.101" },
        capabilities: { load: true },
        extensionMethods: ["x.ai/ask_user_question", "_x.ai/ask_user_question"],
      });

      let releaseCooperative!: () => void;
      transport.respond(
        "session/prompt",
        () =>
          new Promise((resolve) => {
            releaseCooperative = () => resolve({ stopReason: "cancelled" });
          }),
      );
      transport.onNotify = (method) => {
        if (method === "session/cancel") releaseCooperative();
      };
      const cooperative = peer.prompt("grok-session-1", [{ type: "text", text: "wait" }]);
      await waitForPromptCount(transport, 2);
      expect((await peer.cancel("grok-session-1", "user")).ok).toBe(true);
      await expect(cooperative).resolves.toMatchObject({
        ok: true,
        value: { terminal: "cancelled" },
      });
      expect(transport.state).toBe("running");

      let releaseIgnored!: () => void;
      transport.respond(
        "session/prompt",
        () =>
          new Promise((resolve) => {
            releaseIgnored = () => resolve({ stopReason: "end_turn" });
          }),
      );
      transport.onNotify = (method) => {
        if (method === "session/cancel") releaseIgnored();
      };
      void peer.prompt("grok-session-1", [{ type: "text", text: "ignore cancel" }]);
      await waitForPromptCount(transport, 3);
      expect((await peer.cancel("grok-session-1", "user")).ok).toBe(true);
      expect(transport.state).toBe("disposed");
      expect(authorityDisposed).toBe(1);
    },
  );

  it("rejects a relative workspace before any session request", async () => {
    const transport = new FakeGrokTransport();
    const peer = await createGrokAcpPeerRuntime({
      cwd: process.cwd(),
      admission: {
        _tag: "PeerAdmitted",
        profileId: "grok-cli",
        supportState: "experimental",
        peerVersion: "0.2.102",
        launchPlan: {
          _tag: "AcpTrustedLaunchPlan",
          source: "trusted-peer-profile-registry",
          profileId: "grok-cli",
          profileRevision: 1,
          strategy: "trusted-path-lookup",
          executable: "grok",
          args: ["agent", "stdio"],
          versionProbeArgs: ["version"],
          allowedEnvKeys: ["HOME", "XAI_API_KEY"],
          requiredEnvKeys: [],
        },
        grants: {
          fsReadTextFile: false,
          fsWriteTextFile: false,
          terminal: false,
          permissionAutoApproval: false,
          vendorExtensionMethods: [],
          network: false,
        },
        identityPin: { realPath: "/opt/grok", sha256: digest },
        quarantinedCapabilities: [],
        quarantinedExtensionMethods: [],
        diagnostics: {
          profileId: "grok-cli",
          providerId: "x-ai",
          profileRevision: 1,
          contractVersion: 1,
          schemaRelease: "schema-v1.19.0",
          supportState: "experimental",
          peerVersion: "0.2.102",
          executableBasename: "grok",
          executableSha256: digest,
          evidenceArtifactRefs: [],
        },
      },
      createTransport: async () => transport,
    });
    await expect(
      peer.newSession({ cwd: "relative", canonicalThreadSeed: "thread" }),
    ).resolves.toMatchObject({ ok: false, reason: "invalid_value" });
    expect(transport.requests).toEqual([]);
  });
});

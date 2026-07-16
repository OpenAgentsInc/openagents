import { describe, expect, it } from "vite-plus/test";

import { CURSOR_ACP_PROFILE } from "@openagentsinc/agent-client-protocol/extensions/cursor";
import {
  AgentStdioTransportError,
  type AgentStdioReverseHandler,
} from "@openagentsinc/agent-stdio-transport";

import {
  admitCursorAcpPeer,
  createCursorAcpPeerRuntime,
  probeCursorAcpExecutable,
  type CursorAcpTransport,
} from "./cursor-peer-runtime.ts";

type NotificationHandler = (params: unknown) => void;

class FakeCursorTransport implements CursorAcpTransport {
  readonly generation = 11;
  state = "running";
  readonly requests: Array<{ method: string; params: unknown }> = [];
  readonly responders = new Map<string, unknown[]>();
  readonly notifications: Array<{ method: string; params: unknown }> = [];
  readonly notificationHandlers = new Map<string, Set<NotificationHandler>>();
  readonly reverseHandlers = new Map<string, AgentStdioReverseHandler>();
  onShutdown: (() => void) | undefined;
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
  }
  onNotification(method: string, handler: NotificationHandler): () => void {
    const handlers = this.notificationHandlers.get(method) ?? new Set();
    handlers.add(handler);
    this.notificationHandlers.set(method, handlers);
    return () => handlers.delete(handler);
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
    this.onShutdown?.();
    this.state = "exited";
    this.#exit();
  }
  async dispose(): Promise<void> {
    this.state = "disposed";
    this.#exit();
  }
}

const digest = "c".repeat(64);
const version = "2026.6.24";
const at = "2026-07-16T12:00:00.000Z";
const evidence = [
  ...["acp-wire-v1-conformance", "cursor-t3-bde0a4c0"].map((suiteId) => ({
    suiteId,
    kind: "fixture" as const,
    result: "pass" as const,
    peerVersion: version,
    recordedAt: at,
    artifactRef: `fixtures/${suiteId}.json`,
  })),
  ...[
    "cursor-live-profile",
    "cursor-authority-reverse",
    "cursor-vendor-extensions",
    "cursor-model-discovery",
    "cursor-parameterized-model-picker",
  ].map((suiteId) => ({
    suiteId,
    kind: "live" as const,
    result: "pass" as const,
    peerVersion: version,
    executableSha256: digest,
    installationClosureSha256: "d".repeat(64),
    recordedAt: at,
    artifactRef: `live/${suiteId}.json`,
  })),
];

const testProbe = {
  requestedExecutable: "agent",
  resolvedPath: "/opt/cursor/cursor-agent",
  realPath: "/opt/cursor/cursor-agent",
  sha256: digest,
  closureSha256: "d".repeat(64),
  reportedVersion: "2026.06.24-00-45-58-9f61de7",
  platform: { os: process.platform, arch: process.arch },
} as const;

const setup = async (transport: FakeCursorTransport, authorize = true) => {
  const experimental = await admitCursorAcpPeer({
    evidence,
    now: new Date(at),
    probe: testProbe,
  });
  return createCursorAcpPeerRuntime({
    cwd: process.cwd(),
    environment: { HOME: "/tmp/cursor-home" },
    evidence,
    now: new Date(at),
    authorizeLogin: async () => (authorize ? "continue" : "cancel"),
    admission: {
      ...experimental,
      supportState: "supported",
      grants: {
        ...experimental.grants,
        vendorExtensionMethods: CURSOR_ACP_PROFILE.methods.map((entry) => entry.method),
      },
    },
    authority: {
      install: (host) =>
        host.registerReverseHandler("session/request_permission", async () => ({
          outcome: { outcome: "cancelled" },
        })),
    },
    installVendorHandlers: (host) => {
      const disposers = [
        host.registerReverseHandler("cursor/ask_question", async () => ({ answers: {} })),
        host.registerReverseHandler("cursor/create_plan", async () => ({ accepted: false })),
        host.onNotification("cursor/update_todos", () => undefined),
      ];
      return () => disposers.forEach((dispose) => dispose());
    },
    createTransport: async () => transport,
    requestTimeoutMs: 100,
    cancelGraceMs: 5,
  });
};

describe("Cursor admitted ACP peer runtime", () => {
  it("skips another vendor's agent shim and probes only a real cursor-agent target", async () => {
    const root = await mkdtemp(join(tmpdir(), "cursor-acp-probe-"));
    try {
      const other = join(root, "other");
      const cursor = join(root, "cursor");
      await Promise.all([mkdir(other), mkdir(cursor)]);
      const otherTarget = join(other, "grok-agent");
      const cursorTarget = join(cursor, "cursor-agent");
      await Promise.all([
        writeFile(otherTarget, "#!/bin/sh\necho 0.0.1\n", { mode: 0o755 }),
        writeFile(cursorTarget, "#!/bin/sh\necho 2026.06.24-00-45-58-test\n", { mode: 0o755 }),
        writeFile(join(cursor, "index.js"), "export const build = 1;\n"),
      ]);
      await Promise.all([chmod(otherTarget, 0o755), chmod(cursorTarget, 0o755)]);
      await Promise.all([
        symlink(otherTarget, join(other, "agent")),
        symlink(cursorTarget, join(cursor, "agent")),
      ]);
      const probe = await probeCursorAcpExecutable({ PATH: `${other}:${cursor}` });
      expect(probe).toMatchObject({
        requestedExecutable: "agent",
        reportedVersion: "2026.06.24-00-45-58-test",
      });
      expect(probe.realPath.endsWith("/cursor/cursor-agent")).toBe(true);
      expect(probe.resolvedPath).toBe(probe.realPath);
      await writeFile(join(cursor, "index.js"), "export const build = 2;\n");
      const changed = await probeCursorAcpExecutable({ PATH: `${other}:${cursor}` });
      expect(changed.sha256).toBe(probe.sha256);
      expect(changed.closureSha256).not.toBe(probe.closureSha256);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  it("negotiates cursor_login and composes stable sessions, modes, config, and model discovery", async () => {
    const transport = new FakeCursorTransport();
    transport.respond("initialize", {
      protocolVersion: 1,
      agentCapabilities: { loadSession: true, sessionCapabilities: { list: {} } },
      authMethods: [{ id: "cursor_login", name: "Log in to Cursor" }],
    });
    transport.respond("authenticate", {});
    transport.respond("session/new", {
      sessionId: "cursor-session-1",
      modes: {
        currentModeId: "agent",
        availableModes: [
          { id: "agent", name: "Agent" },
          { id: "plan", name: "Plan" },
        ],
      },
      configOptions: [
        {
          id: "model",
          name: "Model",
          category: "model",
          type: "select",
          currentValue: "auto",
          options: [{ value: "auto", name: "Auto" }],
        },
      ],
    });
    transport.respond("cursor/list_available_models", {
      models: [
        {
          value: "cursor-auto",
          name: "Auto",
          configOptions: [
            {
              id: "thinking",
              name: "Thinking",
              type: "boolean",
              currentValue: true,
            },
          ],
        },
      ],
    });
    transport.respond("session/set_mode", {});
    transport.respond("session/set_config_option", {});
    const peer = await setup(transport);

    expect(peer.admission.supportState).toBe("supported");
    expect((await peer.start()).ok).toBe(true);
    expect(transport.requests[0]).toMatchObject({
      method: "initialize",
      params: {
        clientCapabilities: {
          fs: { readTextFile: false, writeTextFile: false },
          terminal: false,
          _meta: { parameterizedModelPicker: true },
        },
      },
    });
    expect(transport.requests[1]).toEqual({
      method: "authenticate",
      params: { methodId: "cursor_login" },
    });
    const session = await peer.newSession({
      cwd: process.cwd(),
      canonicalThreadSeed: "cursor-thread",
    });
    expect(session).toMatchObject({ ok: true, value: { peerSessionId: "cursor-session-1" } });
    expect((await peer.setMode("cursor-session-1", "plan")).ok).toBe(true);
    expect((await peer.setConfigOption("cursor-session-1", "model", "auto")).ok).toBe(true);
    await expect(peer.listAvailableModels()).resolves.toMatchObject({
      ok: true,
      value: {
        provenance: "cursor/list_available_models",
        profileVersion: 1,
        response: { models: [{ value: "cursor-auto", name: "Auto" }] },
        models: [
          { value: "auto", name: "Auto", sources: ["stable-config"] },
          {
            value: "cursor-auto",
            name: "Auto",
            sources: ["cursor/list_available_models"],
          },
        ],
      },
    });
    expect(peer.evidence()?.extensionMethods).toEqual([
      "cursor/ask_question",
      "cursor/create_plan",
      "cursor/update_todos",
      "cursor/list_available_models",
    ]);
    await peer.shutdown();
  });

  it("returns typed model timeout and unconfirmed cancellation failures", async () => {
    const transport = new FakeCursorTransport();
    transport.respond("initialize", {
      protocolVersion: 1,
      agentCapabilities: {},
      authMethods: [],
    });
    transport.respond("session/new", { sessionId: "cursor-session-timeout" });
    transport.respond("cursor/list_available_models", () => {
      throw new AgentStdioTransportError("timeout", "model request timed out");
    });
    let releasePrompt!: () => void;
    transport.respond(
      "session/prompt",
      () =>
        new Promise((resolve) => {
          releasePrompt = () => resolve({ stopReason: "end_turn" });
        }),
    );
    transport.onShutdown = () => releasePrompt?.();
    const peer = await setup(transport);
    expect((await peer.start()).ok).toBe(true);
    const session = await peer.newSession({
      cwd: process.cwd(),
      canonicalThreadSeed: "timeout-thread",
    });
    if (!session.ok) throw new Error("fixture session failed");
    await expect(peer.listAvailableModels()).resolves.toMatchObject({
      ok: false,
      reason: "timed_out",
      safeDetail: "Cursor model discovery timed out",
    });
    const prompt = peer.prompt(session.value.peerSessionId, [{ type: "text", text: "wait" }]);
    while (!transport.requests.some((request) => request.method === "session/prompt"))
      await new Promise((resolve) => setTimeout(resolve, 1));
    await expect(peer.cancel(session.value.peerSessionId, "user")).resolves.toMatchObject({
      ok: false,
      reason: "protocol_failure",
    });
    await prompt;
  });

  it("does not authenticate when the owner cancels the typed browser interaction", async () => {
    const transport = new FakeCursorTransport();
    transport.respond("initialize", {
      protocolVersion: 1,
      agentCapabilities: {},
      authMethods: [{ id: "cursor_login", name: "Log in" }],
    });
    const peer = await setup(transport, false);
    await expect(peer.start()).resolves.toMatchObject({ ok: false, reason: "auth_required" });
    expect(transport.requests.map((request) => request.method)).toEqual(["initialize"]);
  });

  it("rejects malformed model extension output without exposing the native payload", async () => {
    const transport = new FakeCursorTransport();
    transport.respond("initialize", {
      protocolVersion: 1,
      agentCapabilities: {},
      authMethods: [],
    });
    transport.respond("cursor/list_available_models", {
      models: [
        { value: "duplicate", name: "A" },
        { value: "duplicate", name: "B" },
      ],
      secret: "must-not-surface",
    });
    const peer = await setup(transport);
    expect((await peer.start()).ok).toBe(true);
    await expect(peer.listAvailableModels()).resolves.toMatchObject({
      ok: false,
      reason: "invalid_value",
      safeDetail: "Cursor returned an invalid bounded model response",
    });
    await peer.shutdown();
  });
});
import { chmod, mkdtemp, mkdir, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describe, expect, it } from "vite-plus/test";
import type { AcpSessionTransportPort } from "./session-runtime.ts";
import { AcpSessionRuntime } from "./session-runtime.ts";

type Handler = (params: unknown) => void;
type Responder = (params: unknown, signal?: AbortSignal) => unknown | Promise<unknown>;

class FakeError extends Error {
  constructor(
    readonly kind: string,
    readonly code?: number | string,
  ) {
    super(kind);
  }
}

class FakeTransport implements AcpSessionTransportPort {
  state = "running";
  readonly requests: Array<{ method: string; params: unknown }> = [];
  readonly notifications: Array<{ method: string; params: unknown }> = [];
  readonly handlers = new Map<string, Set<Handler>>();
  readonly queuedDrain: Array<() => void> = [];
  readonly responders = new Map<string, Responder[]>();
  disposed = 0;
  shutdowns = 0;
  drains = 0;
  drainHook: (() => Promise<void>) | undefined;
  readonly reverseCancels: Array<string | undefined> = [];
  #exit!: () => void;
  readonly exit = new Promise<void>((resolve) => {
    this.#exit = resolve;
  });

  constructor(readonly generation = 1) {}

  respond(method: string, value: unknown | Responder): void {
    const values = this.responders.get(method) ?? [];
    values.push(typeof value === "function" ? (value as Responder) : () => structuredClone(value));
    this.responders.set(method, values);
  }
  async request(
    method: string,
    params: unknown,
    options?: { signal?: AbortSignal },
  ): Promise<unknown> {
    this.requests.push({ method, params: structuredClone(params) });
    const responder = this.responders.get(method)?.shift();
    if (responder === undefined) throw new Error(`missing fake response for ${method}`);
    return responder(params, options?.signal);
  }
  notify(method: string, params: unknown): void {
    this.notifications.push({ method, params: structuredClone(params) });
  }
  onNotification(method: string, handler: Handler): () => void {
    const handlers = this.handlers.get(method) ?? new Set();
    handlers.add(handler);
    this.handlers.set(method, handlers);
    return () => handlers.delete(handler);
  }
  cancelReverseRequests(sessionId?: string): number {
    this.reverseCancels.push(sessionId);
    return 1;
  }
  emit(method: string, params: unknown): void {
    for (const handler of this.handlers.get(method) ?? []) handler(params);
  }
  async drainAcceptedInbound(): Promise<void> {
    this.drains += 1;
    for (const action of this.queuedDrain.splice(0)) action();
    await this.drainHook?.();
    await Promise.resolve();
  }
  waitForExit(): Promise<void> {
    return this.exit;
  }
  crash(): void {
    this.state = "exited";
    this.#exit();
  }
  async shutdown(): Promise<void> {
    this.shutdowns += 1;
    this.state = "exited";
    this.#exit();
  }
  async dispose(): Promise<void> {
    this.disposed += 1;
    this.state = "disposed";
    this.#exit();
  }
}

const initialize = (extra: Record<string, unknown> = {}) => ({
  protocolVersion: 1,
  agentInfo: { name: "fixture-agent", version: "1.2.3" },
  agentCapabilities: {},
  authMethods: [],
  ...extra,
});
const make = (
  transport: FakeTransport,
  overrides: Partial<ConstructorParameters<typeof AcpSessionRuntime>[0]> = {},
) =>
  new AcpSessionRuntime({
    profile: "standard",
    createTransport: async () => transport,
    clientCapabilities: { fs: { readTextFile: false, writeTextFile: false }, terminal: false },
    ...overrides,
  });
const prepareSession = async (
  runtime: AcpSessionRuntime,
  transport: FakeTransport,
  response: Record<string, unknown> = { sessionId: "peer-1" },
) => {
  transport.respond("initialize", initialize());
  transport.respond("session/new", response);
  expect((await runtime.start()).ok).toBe(true);
  const session = await runtime.newSession({ cwd: "/workspace", canonicalThreadSeed: "thread" });
  expect(session.ok).toBe(true);
};

describe("AcpSessionRuntime startup and turn races", () => {
  it("single-flights concurrent initialize, auth, and bootstrap", async () => {
    const transport = new FakeTransport(7);
    let creates = 0;
    transport.respond(
      "initialize",
      initialize({ authMethods: [{ id: "cached", name: "Cached" }] }),
    );
    transport.respond("authenticate", {});
    transport.respond("session/new", { sessionId: "peer-1" });
    const runtime = make(transport, {
      createTransport: async () => {
        creates += 1;
        return transport;
      },
      selectAuthMethod: async () => "cached",
      bootstrap: { cwd: "/workspace", canonicalThreadSeed: "root" },
    });
    const results = await Promise.all(Array.from({ length: 32 }, () => runtime.start()));
    expect(results.every((result) => result.ok)).toBe(true);
    expect(creates).toBe(1);
    expect(transport.requests.map((request) => request.method)).toEqual([
      "initialize",
      "authenticate",
      "session/new",
    ]);
    expect(runtime.sessions()).toHaveLength(1);
    expect(runtime.evidence?.runtimeGeneration).toBe(7);
  });

  it("fails closed for unauthorized auth and protocol drift", async () => {
    const authTransport = new FakeTransport();
    authTransport.respond(
      "initialize",
      initialize({ authMethods: [{ id: "owner-login", name: "Owner login" }] }),
    );
    expect(await make(authTransport).start()).toMatchObject({ ok: false, reason: "auth_required" });
    expect(authTransport.disposed).toBe(1);
    const driftTransport = new FakeTransport();
    driftTransport.respond("initialize", { protocolVersion: 2 });
    expect(await make(driftTransport).start()).toMatchObject({
      ok: false,
      reason: "incompatible_version",
    });
  });

  it("serializes prompts and drains updates before terminal settlement", async () => {
    const transport = new FakeTransport();
    const updates: Array<Record<string, unknown>> = [];
    const runtime = make(transport, {
      onUpdate: (update) => {
        updates.push(update as unknown as Record<string, unknown>);
      },
    });
    await prepareSession(runtime, transport);
    let release!: () => void;
    transport.respond("session/prompt", async () => {
      transport.emit("session/update", {
        sessionId: "peer-1",
        update: { sessionUpdate: "agent_message_chunk", content: { type: "text", text: "one" } },
      });
      await new Promise<void>((resolve) => {
        release = resolve;
      });
      return { stopReason: "end_turn" };
    });
    transport.respond("session/prompt", { stopReason: "end_turn" });
    const first = runtime.prompt("peer-1", [{ type: "text", text: "first" }]);
    const second = runtime.prompt("peer-1", [{ type: "text", text: "second" }]);
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    expect(transport.requests.filter((entry) => entry.method === "session/prompt")).toHaveLength(1);
    release();
    expect((await Promise.all([first, second])).every((result) => result.ok)).toBe(true);
    expect(updates[0]).toMatchObject({ disposition: "applied", turnGeneration: 1 });
  });

  it("accepts response-before-final-read updates and quarantines post-barrier updates", async () => {
    const transport = new FakeTransport();
    const updates: Array<Record<string, unknown>> = [];
    const runtime = make(transport, {
      onUpdate: (update) => {
        updates.push(update as unknown as Record<string, unknown>);
      },
    });
    await prepareSession(runtime, transport);
    transport.respond("session/prompt", () => {
      transport.queuedDrain.push(() =>
        transport.emit("session/update", {
          sessionId: "peer-1",
          update: { sessionUpdate: "agent_message_chunk", content: { type: "text", text: "tail" } },
        }),
      );
      return { stopReason: "end_turn" };
    });
    const result = await runtime.prompt("peer-1", [{ type: "text", text: "hello" }]);
    expect(result).toMatchObject({
      ok: true,
      value: { terminal: "completed" },
    });
    expect(result.receipt.stopReasonRef).toMatch(/^stop_reason\./);
    expect(runtime.receipts()).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ method: "start", outcome: "started" }),
        expect.objectContaining({ method: "session/prompt", outcome: "started" }),
      ]),
    );
    expect(updates[0]).toMatchObject({ disposition: "applied", turnGeneration: 1 });
    transport.emit("session/update", {
      sessionId: "peer-1",
      update: { sessionUpdate: "agent_message_chunk", content: { type: "text", text: "late" } },
    });
    await Promise.resolve();
    expect(updates.at(-1)).toMatchObject({
      disposition: "quarantined",
      safeReason: "late-after-turn",
    });
  });

  it("preserves identical update frames without a provider update ID", async () => {
    const transport = new FakeTransport();
    const updates: Array<Record<string, unknown>> = [];
    const runtime = make(transport, {
      onUpdate: (update) => {
        updates.push(update as unknown as Record<string, unknown>);
      },
    });
    await prepareSession(runtime, transport);
    transport.respond("session/prompt", () => {
      const frame = {
        sessionId: "peer-1",
        update: {
          sessionUpdate: "agent_message_chunk",
          content: { type: "text", text: "same" },
        },
      };
      transport.emit("session/update", frame);
      transport.emit("session/update", frame);
      return { stopReason: "end_turn" };
    });
    await runtime.prompt("peer-1", [{ type: "text", text: "hello" }]);
    expect(updates.map((update) => update.sequence)).toEqual([1, 2]);
  });

  it("does not settle a turn before asynchronous update projection commits", async () => {
    const transport = new FakeTransport();
    let releaseProjection!: () => void;
    let projected = false;
    const runtime = make(transport, {
      onUpdate: async () => {
        await new Promise<void>((resolve) => {
          releaseProjection = resolve;
        });
        projected = true;
      },
    });
    await prepareSession(runtime, transport);
    transport.respond("session/prompt", () => {
      transport.emit("session/update", {
        sessionId: "peer-1",
        update: {
          sessionUpdate: "agent_message_chunk",
          content: { type: "text", text: "pending projection" },
        },
      });
      return { stopReason: "end_turn" };
    });
    let settled = false;
    const prompt = runtime.prompt("peer-1", [{ type: "text", text: "hello" }]).then((value) => {
      settled = true;
      return value;
    });
    await Promise.resolve();
    await Promise.resolve();
    expect(settled).toBe(false);
    releaseProjection();
    expect((await prompt).ok).toBe(true);
    expect(projected).toBe(true);
  });

  it("single-flights settlement when process exit races a completed prompt", async () => {
    const transport = new FakeTransport();
    let releaseSettlement!: () => void;
    let settlementCalls = 0;
    let settlementStarted!: () => void;
    const started = new Promise<void>((resolve) => {
      settlementStarted = resolve;
    });
    const runtime = make(transport, {
      settleTurn: async () => {
        settlementCalls += 1;
        settlementStarted();
        await new Promise<void>((resolve) => {
          releaseSettlement = resolve;
        });
      },
    });
    await prepareSession(runtime, transport);
    transport.respond("session/prompt", { stopReason: "end_turn" });
    let promptResolved = false;
    const prompt = runtime.prompt("peer-1", [{ type: "text", text: "hello" }]).then((value) => {
      promptResolved = true;
      return value;
    });
    await started;
    transport.crash();
    await Promise.resolve();
    expect(settlementCalls).toBe(1);
    expect(promptResolved).toBe(false);
    releaseSettlement();
    expect(await prompt).toMatchObject({
      ok: true,
      value: { terminal: "completed", stopReason: "end_turn" },
    });
    expect(settlementCalls).toBe(1);
  });

  it("reports process exit when it wins while a completed response is still draining", async () => {
    const transport = new FakeTransport();
    const settlements: Array<Record<string, unknown>> = [];
    let drainStarted!: () => void;
    const started = new Promise<void>((resolve) => {
      drainStarted = resolve;
    });
    let releaseDrain!: () => void;
    const runtime = make(transport, {
      settleTurn: (value) => {
        settlements.push(value as unknown as Record<string, unknown>);
      },
    });
    await prepareSession(runtime, transport);
    transport.drainHook = () =>
      new Promise<void>((resolve) => {
        releaseDrain = resolve;
        drainStarted();
      });
    transport.respond("session/prompt", { stopReason: "end_turn" });
    const prompt = runtime.prompt("peer-1", [{ type: "text", text: "hello" }]);
    await started;
    transport.crash();
    await Promise.resolve();
    releaseDrain();
    expect(await prompt).toMatchObject({ ok: false, reason: "process_exit" });
    expect(settlements).toMatchObject([{ terminal: "process_exit", stopReason: "process_exit" }]);
  });

  it("returns a typed protocol failure when projection settlement rejects", async () => {
    const transport = new FakeTransport();
    let settlementCalls = 0;
    const runtime = make(transport, {
      settleTurn: () => {
        settlementCalls += 1;
        throw new Error("projection unavailable");
      },
    });
    await prepareSession(runtime, transport);
    transport.respond("session/prompt", { stopReason: "end_turn" });
    await expect(
      runtime.prompt("peer-1", [{ type: "text", text: "hello" }]),
    ).resolves.toMatchObject({ ok: false, reason: "protocol_failure" });
    expect(settlementCalls).toBe(1);
    expect(runtime.sessions()[0]?.promptActive).toBe(false);
  });

  it("makes cancellation idempotent and prevents double finalization", async () => {
    const transport = new FakeTransport();
    const runtime = make(transport);
    await prepareSession(runtime, transport);
    transport.respond(
      "session/prompt",
      (_params: unknown, signal?: AbortSignal) =>
        new Promise((_resolve, reject) => {
          signal?.addEventListener("abort", () => reject(new FakeError("cancelled")), {
            once: true,
          });
        }),
    );
    const prompt = runtime.prompt("peer-1", [{ type: "text", text: "hello" }]);
    for (let turn = 0; turn < 8; turn += 1) {
      if (transport.requests.some((entry) => entry.method === "session/prompt")) break;
      await Promise.resolve();
    }
    expect((await runtime.cancel("peer-1", "user")).ok).toBe(true);
    expect((await runtime.cancel("peer-1", "user")).ok).toBe(true);
    expect(await prompt).toMatchObject({ ok: false, reason: "cancelled" });
    expect(
      transport.notifications.filter((entry) => entry.method === "session/cancel"),
    ).toHaveLength(1);
    expect(transport.reverseCancels).toEqual(["peer-1"]);
    expect(runtime.sessions()[0]?.promptActive).toBe(false);
  });

  it("cancels a queued prompt before any protocol request is written", async () => {
    const transport = new FakeTransport();
    const settlements: Array<Record<string, unknown>> = [];
    const runtime = make(transport, {
      settleTurn: (value) => {
        settlements.push(value as unknown as Record<string, unknown>);
      },
    });
    await prepareSession(runtime, transport);
    const prompt = runtime.prompt("peer-1", [{ type: "text", text: "never sent" }]);
    expect((await runtime.cancel("peer-1", "user")).ok).toBe(true);
    expect(await prompt).toMatchObject({ ok: false, reason: "cancelled" });
    expect(transport.requests.some((entry) => entry.method === "session/prompt")).toBe(false);
    expect(transport.notifications.some((entry) => entry.method === "session/cancel")).toBe(false);
    expect(transport.reverseCancels).toEqual([]);
    expect(settlements).toMatchObject([{ terminal: "cancelled", stopReason: "cancelled" }]);
  });

  it("fences shutdown during start and refuses restart after stopped", async () => {
    const transport = new FakeTransport();
    let release!: () => void;
    const runtime = make(transport, {
      createTransport: () =>
        new Promise((resolve) => {
          release = () => resolve(transport);
        }),
    });
    const start = runtime.start();
    const shutdown = runtime.shutdown();
    release();
    expect(await start).toMatchObject({ ok: false, reason: "cancelled" });
    await shutdown;
    expect(runtime.state).toBe("stopped");
    expect(transport.disposed).toBe(1);
    expect(await runtime.start()).toMatchObject({ ok: false, reason: "invalid_state" });
  });

  it("permits only one concurrent root-session attach", async () => {
    const transport = new FakeTransport();
    transport.respond("initialize", initialize());
    let release!: () => void;
    transport.respond(
      "session/new",
      () =>
        new Promise((resolve) => {
          release = () => resolve({ sessionId: "peer-1" });
        }),
    );
    const runtime = make(transport);
    await runtime.start();
    const first = runtime.newSession({ cwd: "/w", canonicalThreadSeed: "one" });
    const second = runtime.newSession({ cwd: "/w", canonicalThreadSeed: "two" });
    expect(await second).toMatchObject({ ok: false, reason: "invalid_state" });
    release();
    expect((await first).ok).toBe(true);
    expect(transport.requests.filter((entry) => entry.method === "session/new")).toHaveLength(1);
  });
});

describe("AcpSessionRuntime lifecycle, replay, authority, and recovery", () => {
  it("capability-gates optional stable lifecycle operations", async () => {
    const transport = new FakeTransport();
    const runtime = make(transport);
    await prepareSession(runtime, transport);
    expect(
      await runtime.loadSession({ cwd: "/w", canonicalThreadSeed: "x", peerSessionId: "s" }),
    ).toMatchObject({ ok: false, reason: "unsupported" });
    expect(
      await runtime.resumeSession({ cwd: "/w", canonicalThreadSeed: "x", peerSessionId: "s" }),
    ).toMatchObject({ ok: false, reason: "unsupported" });
    expect(await runtime.listSessions()).toMatchObject({ ok: false, reason: "unsupported" });
    expect(await runtime.deleteSession("peer-1")).toMatchObject({
      ok: false,
      reason: "unsupported",
    });
    expect(await runtime.closeSession("peer-1")).toMatchObject({
      ok: false,
      reason: "unsupported",
    });
    expect(await runtime.logout()).toMatchObject({ ok: false, reason: "unsupported" });
  });

  it("treats explicit loadSession false and absent fork as unsupported", async () => {
    const transport = new FakeTransport();
    transport.respond("initialize", initialize({ agentCapabilities: { loadSession: false } }));
    const runtime = make(transport, {
      unstableFork: { enabled: true, peerVersion: "1.2.3" },
    });
    await runtime.start();
    expect(
      await runtime.loadSession({
        cwd: "/w",
        canonicalThreadSeed: "x",
        peerSessionId: "peer-1",
      }),
    ).toMatchObject({ ok: false, reason: "unsupported" });
    expect(await runtime.forkSession({ peerSessionId: "peer-1", cwd: "/w" })).toMatchObject({
      ok: false,
      reason: "unsupported",
    });
  });

  it("uses advertised list, delete, and logout capabilities", async () => {
    const transport = new FakeTransport();
    transport.respond(
      "initialize",
      initialize({
        agentCapabilities: {
          sessionCapabilities: { list: {}, delete: {}, close: {} },
          auth: { logout: {} },
        },
      }),
    );
    transport.respond("session/new", { sessionId: "peer-1" });
    transport.respond("session/list", { sessions: [] });
    transport.respond("session/delete", {});
    transport.respond("logout", {});
    const runtime = make(transport);
    expect((await runtime.start()).ok).toBe(true);
    expect((await runtime.newSession({ cwd: "/w", canonicalThreadSeed: "x" })).ok).toBe(true);
    expect((await runtime.listSessions()).ok).toBe(true);
    expect((await runtime.deleteSession("unknown")).ok).toBe(true);
    expect((await runtime.logout()).ok).toBe(true);
  });

  const optionalMethods = [
    "session/load",
    "session/resume",
    "session/list",
    "session/delete",
    "session/close",
    "logout",
  ] as const;
  for (const method of optionalMethods) {
    it.each([
      ["remote_error", "refused"],
      ["timeout", "timed_out"],
      ["process_exit", "process_exit"],
    ] as const)(`${method} returns a typed %s outcome`, async (kind, reason) => {
      const transport = new FakeTransport();
      transport.respond(
        "initialize",
        initialize({
          agentCapabilities: {
            loadSession: true,
            sessionCapabilities: { list: {}, delete: {}, resume: {}, close: {} },
            auth: { logout: {} },
          },
        }),
      );
      transport.respond(method, () => {
        throw new FakeError(kind);
      });
      const runtime = make(transport);
      await runtime.start();
      const result =
        method === "session/load"
          ? await runtime.loadSession({
              cwd: "/w",
              canonicalThreadSeed: "x",
              peerSessionId: "peer-1",
            })
          : method === "session/resume"
            ? await runtime.resumeSession({
                cwd: "/w",
                canonicalThreadSeed: "x",
                peerSessionId: "peer-1",
              })
            : method === "session/list"
              ? await runtime.listSessions()
              : method === "session/delete"
                ? await runtime.deleteSession("peer-1")
                : method === "session/close"
                  ? await runtime.closeSession("peer-1")
                  : await runtime.logout();
      expect(result).toMatchObject({ ok: false, reason });
    });
  }

  it("exposes unstable fork only for an exact enabled peer version", async () => {
    const disabledTransport = new FakeTransport();
    const disabled = make(disabledTransport);
    await prepareSession(disabled, disabledTransport);
    expect(await disabled.forkSession({ peerSessionId: "peer-1", cwd: "/w" })).toMatchObject({
      ok: false,
      reason: "unsupported",
    });

    const enabledTransport = new FakeTransport();
    enabledTransport.respond(
      "initialize",
      initialize({ agentCapabilities: { sessionCapabilities: { fork: {} } } }),
    );
    enabledTransport.respond("session/fork", { sessionId: "fork-1" });
    const enabled = make(enabledTransport, {
      unstableFork: { enabled: true, peerVersion: "1.2.3" },
    });
    await enabled.start();
    expect(await enabled.forkSession({ peerSessionId: "peer-1", cwd: "/w" })).toMatchObject({
      ok: true,
      value: { sessionId: "fork-1" },
    });
  });

  for (const method of [
    "session/new",
    "session/set_mode",
    "session/set_config_option",
    "session/fork",
  ] as const) {
    it.each([
      ["remote_error", "refused"],
      ["timeout", "timed_out"],
      ["process_exit", "process_exit"],
    ] as const)(`${method} returns a typed %s outcome`, async (kind, reason) => {
      const transport = new FakeTransport();
      transport.respond(
        "initialize",
        initialize({ agentCapabilities: { sessionCapabilities: { fork: {} } } }),
      );
      if (method === "session/set_mode" || method === "session/set_config_option")
        transport.respond("session/new", {
          sessionId: "peer-1",
          modes: {
            currentModeId: "ask",
            availableModes: [
              { id: "ask", name: "Ask" },
              { id: "agent", name: "Agent" },
            ],
          },
          configOptions: [
            {
              id: "flag",
              name: "Flag",
              type: "boolean",
              currentValue: false,
            },
          ],
        });
      transport.respond(method, () => {
        throw new FakeError(kind);
      });
      const runtime = make(transport, {
        unstableFork: { enabled: true, peerVersion: "1.2.3" },
      });
      await runtime.start();
      if (method === "session/set_mode" || method === "session/set_config_option")
        await runtime.newSession({ cwd: "/w", canonicalThreadSeed: "x" });
      const result =
        method === "session/new"
          ? await runtime.newSession({ cwd: "/w", canonicalThreadSeed: "x" })
          : method === "session/set_mode"
            ? await runtime.setMode("peer-1", "agent")
            : method === "session/set_config_option"
              ? await runtime.setConfigOption("peer-1", "flag", true)
              : await runtime.forkSession({ peerSessionId: "peer-1", cwd: "/w" });
      expect(result).toMatchObject({ ok: false, reason });
    });
  }

  it("validates and no-op suppresses modes and configuration", async () => {
    const transport = new FakeTransport();
    const runtime = make(transport);
    await prepareSession(runtime, transport, {
      sessionId: "peer-1",
      modes: {
        currentModeId: "ask",
        availableModes: [
          { id: "ask", name: "Ask" },
          { id: "agent", name: "Agent" },
        ],
      },
      configOptions: [
        {
          id: "model",
          name: "Model",
          type: "select",
          currentValue: "a",
          options: [
            { value: "a", name: "A" },
            { value: "b", name: "B" },
          ],
        },
      ],
    });
    expect((await runtime.setMode("peer-1", "ask")).ok).toBe(true);
    expect((await runtime.setConfigOption("peer-1", "model", "a")).ok).toBe(true);
    expect(
      transport.requests.filter((entry) => entry.method.startsWith("session/set_")),
    ).toHaveLength(0);
    expect(await runtime.setMode("peer-1", "invalid")).toMatchObject({
      ok: false,
      reason: "invalid_value",
    });
    expect(await runtime.setConfigOption("peer-1", "model", "invalid")).toMatchObject({
      ok: false,
      reason: "invalid_value",
    });
    transport.respond("session/set_mode", {});
    transport.respond("session/set_config_option", {
      configOptions: [
        {
          id: "model",
          name: "Model",
          type: "select",
          currentValue: "b",
          options: [
            { value: "a", name: "A" },
            { value: "b", name: "B" },
          ],
        },
      ],
    });
    expect((await runtime.setMode("peer-1", "agent")).ok).toBe(true);
    expect((await runtime.setConfigOption("peer-1", "model", "b")).ok).toBe(true);
    expect(runtime.sessions()[0]?.modes?.currentModeId).toBe("agent");
  });

  it("keeps replay behind a one-way gate before live prompts", async () => {
    const transport = new FakeTransport();
    const updates: Array<Record<string, unknown>> = [];
    transport.respond("initialize", initialize({ agentCapabilities: { loadSession: true } }));
    transport.respond("session/load", () => {
      transport.emit("session/update", {
        sessionId: "existing",
        update: {
          sessionUpdate: "agent_message_chunk",
          content: { type: "text", text: "history" },
        },
      });
      return {};
    });
    const runtime = make(transport, {
      onUpdate: (update) => {
        updates.push(update as unknown as Record<string, unknown>);
      },
    });
    expect((await runtime.start()).ok).toBe(true);
    expect(
      await runtime.loadSession({
        cwd: "/w",
        canonicalThreadSeed: "canonical",
        peerSessionId: "existing",
      }),
    ).toMatchObject({ ok: true, value: { phase: "live" } });
    expect(updates[0]).toMatchObject({ phase: "replay", disposition: "applied" });
  });

  it("materializes MCP refs for one generation and drops material afterward", async () => {
    const transport = new FakeTransport(12);
    transport.respond("initialize", initialize());
    transport.respond("session/new", { sessionId: "peer-1" });
    let disposed = 0;
    const runtime = make(transport, {
      now: () => new Date("2026-07-16T12:00:00Z"),
      materializeMcp: async (_refs, context) => {
        expect(context).toMatchObject({
          runtimeGeneration: 12,
          sessionGeneration: 1,
          method: "session/new",
        });
        return {
          servers: [{ name: "fixture", command: "fixture", args: [], env: [] }],
          resolvedRefs: [{ serverRef: "mcp.fixture", transport: "stdio" }],
          receiptRefs: ["receipt.mcp.fixture"],
          dispose: () => {
            disposed += 1;
          },
        };
      },
    });
    await runtime.start();
    const result = await runtime.newSession({
      cwd: "/w",
      canonicalThreadSeed: "x",
      mcpRefs: [
        {
          serverRef: "mcp.fixture",
          transport: "stdio",
          expiresAt: "2026-07-17T00:00:00Z",
        },
      ],
    });
    expect(result.ok).toBe(true);
    expect(result.receipt.evidenceRefs).toEqual(["receipt.mcp.fixture"]);
    expect(disposed).toBe(1);
    expect(JSON.stringify(runtime.sessions())).not.toContain("command");
  });

  it("refuses expired MCP references before session/new", async () => {
    const transport = new FakeTransport();
    transport.respond("initialize", initialize());
    const runtime = make(transport, { now: () => new Date("2026-07-16T12:00:00Z") });
    await runtime.start();
    expect(
      await runtime.newSession({
        cwd: "/w",
        canonicalThreadSeed: "x",
        mcpRefs: [{ serverRef: "expired", transport: "http", expiresAt: "2026-07-15T00:00:00Z" }],
      }),
    ).toMatchObject({ ok: false, reason: "protocol_failure" });
    expect(transport.requests.some((entry) => entry.method === "session/new")).toBe(false);
  });

  it("refuses out-of-scope and mismatched MCP material before peer attach", async () => {
    const outOfScopeTransport = new FakeTransport();
    outOfScopeTransport.respond("initialize", initialize());
    const outOfScope = make(outOfScopeTransport, {
      materializeMcp: async () => {
        throw new Error("must not materialize");
      },
    });
    await outOfScope.start();
    expect(
      await outOfScope.newSession({
        cwd: "/w",
        canonicalThreadSeed: "x",
        scopeRef: "scope.allowed",
        mcpRefs: [
          {
            serverRef: "mcp.fixture",
            transport: "stdio",
            expiresAt: "2099-01-01T00:00:00Z",
            scopeRef: "scope.denied",
          },
        ],
      }),
    ).toMatchObject({ ok: false, reason: "protocol_failure" });

    const mismatchTransport = new FakeTransport();
    mismatchTransport.respond("initialize", initialize());
    let mismatchDisposed = 0;
    const mismatch = make(mismatchTransport, {
      materializeMcp: async () => ({
        servers: [{ name: "fixture", command: "fixture", args: [], env: [] }],
        resolvedRefs: [{ serverRef: "different", transport: "stdio" }],
        receiptRefs: [],
        dispose: () => {
          mismatchDisposed += 1;
        },
      }),
    });
    await mismatch.start();
    expect(
      await mismatch.newSession({
        cwd: "/w",
        canonicalThreadSeed: "x",
        mcpRefs: [
          {
            serverRef: "mcp.fixture",
            transport: "stdio",
            expiresAt: "2099-01-01T00:00:00Z",
          },
        ],
      }),
    ).toMatchObject({ ok: false, reason: "protocol_failure" });
    expect(mismatchTransport.requests.some((entry) => entry.method === "session/new")).toBe(false);
    expect(mismatchDisposed).toBe(1);
  });

  it("observes idle process exit and never reports the session as live", async () => {
    const transport = new FakeTransport();
    const runtime = make(transport);
    await prepareSession(runtime, transport);
    transport.crash();
    await Promise.resolve();
    expect(runtime.state).toBe("failed");
    expect(runtime.sessions()[0]).toMatchObject({ phase: "closed", promptActive: false });
    expect(runtime.receipts().at(-1)).toMatchObject({
      method: "process/exit",
      outcome: "process_exit",
    });
  });

  it("recovers through advertised resume on a new runtime generation", async () => {
    const first = new FakeTransport(1);
    const second = new FakeTransport(2);
    first.respond(
      "initialize",
      initialize({ agentCapabilities: { sessionCapabilities: { resume: {} } } }),
    );
    first.respond("session/new", { sessionId: "peer-1" });
    second.respond(
      "initialize",
      initialize({ agentCapabilities: { sessionCapabilities: { resume: {} } } }),
    );
    second.respond("session/resume", {});
    const queue = [first, second];
    const runtime = make(first, { createTransport: async () => queue.shift()! });
    await runtime.start();
    const original = await runtime.newSession({ cwd: "/w", canonicalThreadSeed: "x" });
    const recoveryInput = {
      cwd: "/w",
      canonicalThreadSeed: "x",
      peerSessionId: "peer-1",
    } as const;
    const recovery = runtime.recover(recoveryInput);
    expect(runtime.recover(recoveryInput)).toBe(recovery);
    const recovered = await recovery;
    expect(recovered).toMatchObject({ ok: true, value: { runtimeGeneration: 2 } });
    if (original.ok && recovered.ok) expect(recovered.value.threadId).toBe(original.value.threadId);
    expect(recovered.receipt.recoveryDecision).toBe("reattached");
  });

  it.each([
    ["missing-binary", "missing-binary"],
    ["incompatible", "incompatible-version"],
    ["protocol-drift", "protocol-drift"],
  ] as const)("records terminal %s recovery", async (fault, decision) => {
    const first = new FakeTransport(1);
    first.respond(
      "initialize",
      initialize({ agentCapabilities: { sessionCapabilities: { resume: {} } } }),
    );
    first.respond("session/new", { sessionId: "peer-1" });
    const runtime = make(first, {
      createTransport: async () => {
        if (runtime.evidence === undefined) return first;
        if (fault === "missing-binary") throw new FakeError("missing_executable");
        const next = new FakeTransport(2);
        next.respond(
          "initialize",
          fault === "incompatible"
            ? { protocolVersion: 2 }
            : () => {
                throw new Error("drift");
              },
        );
        return next;
      },
    });
    await runtime.start();
    await runtime.newSession({ cwd: "/w", canonicalThreadSeed: "x" });
    const result = await runtime.recover({
      cwd: "/w",
      canonicalThreadSeed: "x",
      peerSessionId: "peer-1",
    });
    expect(result.ok).toBe(false);
    expect(result.receipt.recoveryDecision).toBe(decision);
  });

  it("ends a consecutive failed-recovery loop at the configured budget", async () => {
    const first = new FakeTransport(1);
    first.respond(
      "initialize",
      initialize({ agentCapabilities: { sessionCapabilities: { resume: {} } } }),
    );
    first.respond("session/new", { sessionId: "peer-1" });
    let starts = 0;
    const runtime = make(first, {
      restart: { maxAttempts: 1, baseBackoffMs: 1, maxBackoffMs: 1 },
      createTransport: async () => {
        starts += 1;
        if (starts === 1) return first;
        throw new FakeError("missing_executable");
      },
    });
    await runtime.start();
    await runtime.newSession({ cwd: "/w", canonicalThreadSeed: "x" });
    await runtime.recover({ cwd: "/w", canonicalThreadSeed: "x", peerSessionId: "peer-1" });
    const exhausted = await runtime.recover({
      cwd: "/w",
      canonicalThreadSeed: "x",
      peerSessionId: "peer-1",
    });
    expect(exhausted).toMatchObject({ ok: false, reason: "restart_budget_exhausted" });
    expect(exhausted.receipt.recoveryDecision).toBe("crash-loop");
  });

  it("records auth loss, missing session, and user-cancelled recovery distinctly", async () => {
    const initial = () => {
      const transport = new FakeTransport(1);
      transport.respond(
        "initialize",
        initialize({ agentCapabilities: { sessionCapabilities: { resume: {} } } }),
      );
      transport.respond("session/new", { sessionId: "peer-1" });
      return transport;
    };

    const firstAuth = initial();
    const auth = new FakeTransport(2);
    auth.respond(
      "initialize",
      initialize({
        authMethods: [{ id: "cached", name: "Cached" }],
        agentCapabilities: { sessionCapabilities: { resume: {} } },
      }),
    );
    auth.respond("authenticate", () => {
      throw new FakeError("remote_error");
    });
    const authQueue = [firstAuth, auth];
    const authRuntime = make(firstAuth, {
      createTransport: async () => authQueue.shift()!,
      selectAuthMethod: async () => "cached",
    });
    await authRuntime.start();
    await authRuntime.newSession({ cwd: "/w", canonicalThreadSeed: "x" });
    const authResult = await authRuntime.recover({
      cwd: "/w",
      canonicalThreadSeed: "x",
      peerSessionId: "peer-1",
    });
    expect(authResult.receipt.recoveryDecision).toBe("auth-lost");

    const firstMissing = initial();
    const missing = new FakeTransport(2);
    missing.respond(
      "initialize",
      initialize({ agentCapabilities: { sessionCapabilities: { resume: {} } } }),
    );
    missing.respond("session/resume", () => {
      throw new FakeError("remote_error", -32002);
    });
    const missingQueue = [firstMissing, missing];
    const missingRuntime = make(firstMissing, {
      createTransport: async () => missingQueue.shift()!,
    });
    await missingRuntime.start();
    await missingRuntime.newSession({ cwd: "/w", canonicalThreadSeed: "x" });
    const missingResult = await missingRuntime.recover({
      cwd: "/w",
      canonicalThreadSeed: "x",
      peerSessionId: "peer-1",
    });
    expect(missingResult.receipt.recoveryDecision).toBe("missing-session");

    const cancelled = initial();
    const cancelledRuntime = make(cancelled);
    await cancelledRuntime.start();
    await cancelledRuntime.newSession({ cwd: "/w", canonicalThreadSeed: "x" });
    const controller = new AbortController();
    controller.abort();
    const cancelledResult = await cancelledRuntime.recover(
      { cwd: "/w", canonicalThreadSeed: "x", peerSessionId: "peer-1" },
      { signal: controller.signal },
    );
    expect(cancelledResult.receipt.recoveryDecision).toBe("cancelled");
  });

  it("propagates cancellation into an in-flight recovery initialize request", async () => {
    const first = new FakeTransport(1);
    const second = new FakeTransport(2);
    first.respond(
      "initialize",
      initialize({ agentCapabilities: { sessionCapabilities: { resume: {} } } }),
    );
    first.respond("session/new", { sessionId: "peer-1" });
    let initializeStarted!: () => void;
    const started = new Promise<void>((resolve) => {
      initializeStarted = resolve;
    });
    second.respond(
      "initialize",
      (_params: unknown, signal?: AbortSignal) =>
        new Promise((_resolve, reject) => {
          initializeStarted();
          signal?.addEventListener("abort", () => reject(new FakeError("cancelled")), {
            once: true,
          });
        }),
    );
    const queue = [first, second];
    const runtime = make(first, { createTransport: async () => queue.shift()! });
    await runtime.start();
    await runtime.newSession({ cwd: "/w", canonicalThreadSeed: "x" });
    const controller = new AbortController();
    const recovery = runtime.recover(
      { cwd: "/w", canonicalThreadSeed: "x", peerSessionId: "peer-1" },
      { signal: controller.signal },
    );
    await started;
    controller.abort();
    const result = await recovery;
    expect(result).toMatchObject({ ok: false, reason: "cancelled" });
    expect(result.receipt.recoveryDecision).toBe("cancelled");
    expect(second.disposed).toBe(1);
  });

  it("classifies cancellation during recovery authentication as cancelled", async () => {
    const first = new FakeTransport(1);
    const second = new FakeTransport(2);
    first.respond(
      "initialize",
      initialize({ agentCapabilities: { sessionCapabilities: { resume: {} } } }),
    );
    first.respond("session/new", { sessionId: "peer-1" });
    second.respond(
      "initialize",
      initialize({
        authMethods: [{ id: "cached", name: "Cached login" }],
        agentCapabilities: { sessionCapabilities: { resume: {} } },
      }),
    );
    let authenticateStarted!: () => void;
    const started = new Promise<void>((resolve) => {
      authenticateStarted = resolve;
    });
    second.respond(
      "authenticate",
      (_params: unknown, signal?: AbortSignal) =>
        new Promise((_resolve, reject) => {
          authenticateStarted();
          signal?.addEventListener("abort", () => reject(new FakeError("cancelled")), {
            once: true,
          });
        }),
    );
    const queue = [first, second];
    const runtime = make(first, {
      createTransport: async () => queue.shift()!,
      selectAuthMethod: async () => "cached",
    });
    await runtime.start();
    await runtime.newSession({ cwd: "/w", canonicalThreadSeed: "x" });
    const controller = new AbortController();
    const recovery = runtime.recover(
      { cwd: "/w", canonicalThreadSeed: "x", peerSessionId: "peer-1" },
      { signal: controller.signal },
    );
    await started;
    controller.abort();
    const result = await recovery;
    expect(result).toMatchObject({ ok: false, reason: "cancelled" });
    expect(result.receipt.recoveryDecision).toBe("cancelled");
    expect(second.disposed).toBe(1);
  });

  it("suppresses configured bootstrap while repairing an existing session", async () => {
    const first = new FakeTransport(1);
    const second = new FakeTransport(2);
    first.respond(
      "initialize",
      initialize({ agentCapabilities: { sessionCapabilities: { resume: {} } } }),
    );
    first.respond("session/new", { sessionId: "peer-1" });
    second.respond(
      "initialize",
      initialize({ agentCapabilities: { sessionCapabilities: { resume: {} } } }),
    );
    second.respond("session/resume", {});
    const queue = [first, second];
    const runtime = make(first, {
      createTransport: async () => queue.shift()!,
      bootstrap: { cwd: "/w", canonicalThreadSeed: "x" },
    });
    await runtime.start();
    expect(
      await runtime.recover({
        cwd: "/w",
        canonicalThreadSeed: "x",
        peerSessionId: "peer-1",
      }),
    ).toMatchObject({ ok: true });
    expect(second.requests.map((entry) => entry.method)).toEqual(["initialize", "session/resume"]);
  });

  it("re-resolves and disposes MCP material for the recovered generation", async () => {
    const first = new FakeTransport(1);
    const second = new FakeTransport(2);
    first.respond(
      "initialize",
      initialize({ agentCapabilities: { sessionCapabilities: { resume: {} } } }),
    );
    first.respond("session/new", { sessionId: "peer-1" });
    second.respond(
      "initialize",
      initialize({ agentCapabilities: { sessionCapabilities: { resume: {} } } }),
    );
    second.respond("session/resume", {});
    const queue = [first, second];
    const contexts: Array<Record<string, unknown>> = [];
    let disposed = 0;
    const runtime = make(first, {
      createTransport: async () => queue.shift()!,
      materializeMcp: async (refs, context) => {
        contexts.push(context as unknown as Record<string, unknown>);
        return {
          servers: [{ name: "fixture", command: "fixture", args: [], env: [] }],
          resolvedRefs: refs.map(({ serverRef, transport }) => ({ serverRef, transport })),
          receiptRefs: ["receipt.mcp.recovered"],
          dispose: () => {
            disposed += 1;
          },
        };
      },
    });
    await runtime.start();
    await runtime.newSession({ cwd: "/w", canonicalThreadSeed: "x" });
    const recovered = await runtime.recover({
      cwd: "/w",
      canonicalThreadSeed: "x",
      peerSessionId: "peer-1",
      scopeRef: "scope.x",
      mcpRefs: [
        {
          serverRef: "mcp.fixture",
          transport: "stdio",
          expiresAt: "2099-01-01T00:00:00Z",
          scopeRef: "scope.x",
        },
      ],
    });
    expect(recovered.ok).toBe(true);
    expect(contexts).toMatchObject([
      { runtimeGeneration: 2, method: "session/resume", scopeRef: "scope.x" },
    ]);
    expect(disposed).toBe(1);
  });

  it("shuts down idempotently with a distinct cancellation source", async () => {
    const transport = new FakeTransport();
    const runtime = make(transport);
    await prepareSession(runtime, transport);
    await runtime.shutdown();
    await runtime.shutdown();
    expect(runtime.state).toBe("stopped");
    expect(transport.shutdowns).toBe(1);
    expect(transport.disposed).toBe(1);
  });
});

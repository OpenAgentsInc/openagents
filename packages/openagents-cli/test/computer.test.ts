import * as NodeServices from "@effect/platform-node/NodeServices";
import { Effect, Layer, Option, Redacted } from "effect";
import { mkdtemp, rm, stat } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";

import { runCliWith } from "../src/cli.js";
import {
  ComputerChannel,
  computerChannelNodeLayer,
  ComputerSocketTransport,
  type ComputerChannelHandlers,
  type ComputerSocket,
} from "../src/computer-channel.js";
import {
  ComputerConfiguration,
  computerConfigurationLayer,
  computerPaths,
  type ComputerPaths,
} from "../src/computer-config.js";
import {
  ComputerJournal,
  computerJournalLayer,
  journalMaxBytes,
  journalReadTailBytes,
} from "../src/computer-journal.js";
import {
  curatedAllowlist,
  decide,
  resolveRoots,
  tierAllows,
  type PolicyConfig,
  withinRoot,
} from "../src/computer-policy.js";
import {
  boundedVersion,
  codingAgentCatalog,
  computerProbeLayer,
  ComputerProbe,
  toolchainCatalog,
} from "../src/computer-probe.js";
import { executeComputerCommand } from "../src/computer-executor.js";
import { ComputerAgentProcess, type AgentProcess } from "../src/computer-agents.js";
import { ComputerClient, type ComputerStatus } from "../src/computer-client.js";
import { ComputerUp, computerUpLayer } from "../src/computer-up.js";
import { environmentLayerFromValues } from "../src/environment.js";
import { CredentialStore, credentialStoreTestFileLayer } from "../src/credential-store.js";
import { pendingDeviceAuthorizationStoreTestLayer } from "../src/device-authorization-store.js";
import { persistedConfigurationTestLayer } from "../src/persisted-configuration.js";
import { outputTestLayer, type OutputDocument, type OutputMode } from "../src/output.js";

const computerConfigurationTestLayer = (
  values: Partial<PolicyConfig> & { readonly paths?: ComputerPaths } = {},
): Layer.Layer<ComputerConfiguration> =>
  Layer.succeed(
    ComputerConfiguration,
    ComputerConfiguration.of({
      tier: values.tier ?? "probe",
      roots: resolveRoots(values.roots ?? []),
      preApproved: values.preApproved ?? [],
      agents: values.agents ?? [],
      registryAgents: values.registryAgents ?? false,
      curatedExecute: values.curatedExecute ?? [],
      paths: values.paths ?? computerPaths(),
    }),
  );

const computerJournalTestLayer = (path: string): Layer.Layer<ComputerJournal> =>
  computerJournalLayer.pipe(
    Layer.provide(
      computerConfigurationTestLayer({
        paths: { ...computerPaths(path), journal: path },
      }),
    ),
  );

class StubSocket implements ComputerSocket {
  readyState = 0;
  readonly sent: string[] = [];
  private readonly listeners = new Map<string, Array<(...args: ReadonlyArray<unknown>) => void>>();

  on(event: string, listener: (...args: ReadonlyArray<unknown>) => void): void {
    this.listeners.set(event, [...(this.listeners.get(event) ?? []), listener]);
  }

  send(data: string): void {
    this.sent.push(data);
  }

  close(): void {
    this.readyState = 3;
    this.emit("close");
  }

  open(): void {
    this.readyState = 1;
    this.emit("open");
  }

  message(value: unknown): void {
    this.emit("message", JSON.stringify(value));
  }

  emit(event: string, ...args: ReadonlyArray<unknown>): void {
    for (const listener of this.listeners.get(event) ?? []) listener(...args);
  }
}

const sentFrames = (socket: StubSocket): Array<ReadonlyArray<unknown>> =>
  socket.sent.map((value) => JSON.parse(value) as ReadonlyArray<unknown>);

const computerChannelTestLayer = (transport: {
  readonly connect: (url: string) => ComputerSocket;
}): Layer.Layer<ComputerChannel> =>
  computerChannelNodeLayer.pipe(Layer.provide(Layer.succeed(ComputerSocketTransport, transport)));

afterEach(() => {
  vi.useRealTimers();
});

describe("local Computer policy", () => {
  const root = "/workspace/project";
  const config = { tier: "probe" as const, roots: [root], preApproved: [] };

  it("defaults to probe with no reachable roots", async () => {
    const directory = await mkdtemp(join("/tmp", "openagents-cli-computer-"));
    try {
      const layer = computerConfigurationLayer.pipe(
        Layer.provide(environmentLayerFromValues({ configPath: join(directory, "config.json") })),
      );
      const value = await Effect.runPromise(ComputerConfiguration.pipe(Effect.provide(layer)));
      expect(value.tier).toBe("probe");
      expect(value.roots).toEqual([]);
      expect(value.paths.config).toBe(join(directory, "computer.json"));
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  it("resolves roots and rejects textual-prefix siblings", () => {
    expect(resolveRoots(["~/work", "~/work/"])).toEqual([join(homedir(), "work")]);
    expect(withinRoot("/workspace/project/file.txt", root)).toBe(true);
    expect(withinRoot("/workspace/project-two", root)).toBe(false);
    if (process.platform === "win32") {
      expect(withinRoot("C:\\workspace\\project\\file.txt", "C:\\workspace\\project")).toBe(true);
      expect(withinRoot("C:\\workspace\\project-two", "C:\\workspace\\project")).toBe(false);
    }
  });

  it("keeps the tier as a ceiling and applies universal denials first", () => {
    expect(tierAllows("probe", "shell")).toBe(false);
    expect(tierAllows("shell", "curated")).toBe(true);
    expect(decide({ argv: ["git", "status"], cwd: root }, config)).toEqual({
      _tag: "Refused",
      reason: "tier_insufficient",
      detail: "probe tier permits fixed discovery only",
    });
    expect(
      decide({ argv: ["sudo", "git", "status"], cwd: root }, { ...config, tier: "shell" }),
    ).toMatchObject({ _tag: "Refused", reason: "denied_command" });
    expect(
      decide({ argv: ["cat", "/etc/passwd"], cwd: root }, { ...config, tier: "shell" }),
    ).toMatchObject({ _tag: "Refused", reason: "denied_argument" });
    expect(
      decide(
        { argv: ["cat", "/workspace/project/file.txt"], cwd: root },
        { ...config, tier: "curated" },
      ),
    ).toMatchObject({ _tag: "Allowed" });
    expect(
      decide({ argv: ["cat", "/tmp/outside"], cwd: root }, { ...config, tier: "curated" }),
    ).toMatchObject({ _tag: "Refused", reason: "denied_argument" });
    expect(decide({ argv: ["git", "status"], cwd: root }, { ...config, roots: [] })).toMatchObject({
      _tag: "Refused",
      reason: "root_not_declared",
    });
    expect(
      decide({ argv: ["echo", "hello;whoami"], cwd: root }, { ...config, tier: "shell" }),
    ).toMatchObject({ _tag: "Refused", reason: "shell_metacharacter" });
    expect(
      decide(
        { argv: ["env", "FOO=1", "bash", "-c", "id"], cwd: root },
        { ...config, tier: "curated" },
      ),
    ).toMatchObject({ _tag: "Refused", reason: "not_allowlisted" });
    expect(
      decide({ argv: ["npm", "run", "build"], cwd: root }, { ...config, tier: "curated" }),
    ).toMatchObject({ _tag: "Refused", reason: "not_allowlisted" });
    expect(
      decide(
        { argv: ["find", ".", "-exec", "sh", "-c", "id"], cwd: root },
        { ...config, tier: "curated" },
      ),
    ).toMatchObject({ _tag: "Refused", reason: "not_allowlisted" });
    expect(
      decide({ argv: ["find", ".", "-delete"], cwd: root }, { ...config, tier: "curated" }),
    ).toMatchObject({ _tag: "Refused", reason: "not_allowlisted" });
    expect(
      decide(
        { argv: ["rg", "--pre", "bash", "pattern"], cwd: root },
        { ...config, tier: "curated" },
      ),
    ).toMatchObject({ _tag: "Refused", reason: "not_allowlisted" });
    expect(
      decide({ argv: ["git", "branch", "-D", "main"], cwd: root }, { ...config, tier: "curated" }),
    ).toMatchObject({ _tag: "Refused", reason: "not_allowlisted" });
    expect(
      decide(
        { argv: ["git", "remote", "set-url", "origin", "evil"], cwd: root },
        { ...config, tier: "curated" },
      ),
    ).toMatchObject({ _tag: "Refused", reason: "not_allowlisted" });
    expect(
      decide({ argv: ["./opencode.exe", "acp"], cwd: root }, { ...config, tier: "shell" }),
    ).toEqual({ _tag: "Allowed", needsConfirmation: true });
    expect(
      decide({ argv: ["gh", "issue", "list"], cwd: root }, { ...config, tier: "curated" }),
    ).toMatchObject({ _tag: "Allowed" });
    expect(
      decide({ argv: ["gh", "issue", "close"], cwd: root }, { ...config, tier: "curated" }),
    ).toMatchObject({ _tag: "Refused", reason: "not_allowlisted" });
    expect(
      decide({ argv: ["date", "+%s"], cwd: root }, { ...config, tier: "curated" }),
    ).toMatchObject({ _tag: "Refused", reason: "not_allowlisted" });
    expect(
      decide({ argv: ["ps", "aux"], cwd: root }, { ...config, tier: "curated" }),
    ).toMatchObject({ _tag: "Refused", reason: "not_allowlisted" });
  });

  it("requires confirmation for non-pre-approved shell commands", () => {
    expect(decide({ argv: ["echo", "hello"], cwd: root }, { ...config, tier: "shell" })).toEqual({
      _tag: "Allowed",
      needsConfirmation: true,
    });
    expect(
      decide(
        { argv: ["echo", "hello"], cwd: root },
        { ...config, tier: "shell", preApproved: ["echo"] },
      ),
    ).toEqual({ _tag: "Allowed", needsConfirmation: false });
  });

  it("exposes a versioned read-only allowlist", () => {
    expect(curatedAllowlist.git).toContain("status");
    expect(curatedAllowlist.sudo).toBeUndefined();
  });
});

describe("local Computer probe", () => {
  it("keeps known catalogs fixed and versions bounded", () => {
    expect(codingAgentCatalog.map((entry) => entry.name)).toContain("claude");
    expect(toolchainCatalog.find((entry) => entry.name === "go")?.versionArgv).toEqual(["version"]);
    expect(boundedVersion("x".repeat(200))).toHaveLength(120);
  });

  it("returns a complete report with missing tools as data", async () => {
    const layer = computerProbeLayer.pipe(
      Layer.provide(Layer.merge(NodeServices.layer, computerConfigurationTestLayer({ roots: [] }))),
    );
    const report = await Effect.runPromise(
      Effect.gen(function* () {
        const probe = yield* ComputerProbe;
        return yield* probe.probe();
      }).pipe(Effect.provide(layer)),
    );
    expect(report.schema).toBe("openagents.computer_probe.v1");
    expect(report.roots).toEqual([]);
    expect(report.host.platform).toBe(process.platform);
    expect(report.codingAgents.every((entry) => typeof entry.present === "boolean")).toBe(true);
    expect(report.acp_agents).toBeDefined();
    expect(report.toolchains.every((entry) => typeof entry.version === "string")).toBe(true);
    expect(report.worktrees).toEqual([]);
  });
});

describe("local Computer journal", () => {
  it("reads no entries when its file is absent and redacts credential-shaped values", async () => {
    const directory = await mkdtemp(join("/tmp", "openagents-cli-journal-"));
    const path = join(directory, "journal.ndjson");
    try {
      const layer = computerJournalTestLayer(path);
      const entries = await Effect.runPromise(
        Effect.gen(function* () {
          const journal = yield* ComputerJournal;
          expect(yield* journal.read(20)).toEqual([]);
          yield* journal.append({
            requestId: "request-1",
            argv: [
              "git",
              "status",
              "oa_pat_secret",
              "oa_agent_secret",
              "oa_assignment_secret",
              "smct_secret",
            ],
            cwd: "/workspace/project",
            decision: "refused",
            outcome: "refused",
            detail: "oa_assignment_secret smct_secret",
          });
          return yield* journal.read(20);
        }).pipe(Effect.provide(layer)),
      );
      expect(entries[0]).toMatchObject({
        requestId: "request-1",
        argv: ["git", "status", "[REDACTED]", "[REDACTED]", "[REDACTED]", "[REDACTED]"],
        decision: "refused",
        detail: "[REDACTED] [REDACTED]",
      });
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  it("bounds journal growth while retaining the newest entries", async () => {
    const directory = await mkdtemp(join("/tmp", "openagents-cli-journal-limit-"));
    const path = join(directory, "journal.ndjson");
    try {
      await Effect.runPromise(
        Effect.gen(function* () {
          const journal = yield* ComputerJournal;
          for (let index = 0; index < 800; index += 1) {
            yield* journal.append({
              requestId: `request-${index}`,
              argv: ["git", "status"],
              cwd: "/workspace/project",
              decision: "allowed",
              outcome: "completed",
              detail: "x".repeat(512),
            });
          }
        }).pipe(Effect.provide(computerJournalTestLayer(path))),
      );
      expect((await stat(path)).size).toBeLessThanOrEqual(journalMaxBytes);
      const entries = await Effect.runPromise(
        Effect.gen(function* () {
          const journal = yield* ComputerJournal;
          return yield* journal.read(20);
        }).pipe(Effect.provide(computerJournalTestLayer(path))),
      );
      expect(entries).toHaveLength(20);
      expect(entries.at(-1)?.requestId).toBe("request-799");
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });
});

describe("Computer channel", () => {
  const options = {
    origin: "https://openagents.example",
    token: Redacted.make("smct_test-secret"),
    machineId: "machine-1",
    hello: {
      agent_version: "0.2.1",
      tier: "curated",
      roots: ["/workspace"],
      probe: { schema: "openagents.computer_probe.v1" },
    },
    heartbeatMillis: 30_000,
    maximumReconnectAttempts: 0,
  };

  it("joins, sends hello, answers probes, and correlates run responses", async () => {
    const socket = new StubSocket();
    const events: string[] = [];
    const cancelled: string[] = [];
    let runResponder:
      | {
          readonly chunk: (text: string) => void;
          readonly exit: (payload: Record<string, unknown>) => void;
          readonly refused: (reason: string, detail: string) => void;
        }
      | undefined;
    const channelRun = Effect.runPromise(
      Effect.gen(function* () {
        const channel = yield* ComputerChannel;
        return yield* channel.serve(options, {
          onProbe: async () => ({ schema: "openagents.computer_probe.v1", roots: ["/workspace"] }),
          onRun: (_requestId, _payload, responder) => {
            runResponder = responder;
          },
          onCancel: (requestId) => cancelled.push(requestId),
          onJoined: () => events.push("joined"),
          onEvent: (event) => events.push(event),
          onClosed: (reason) => events.push(`closed:${reason}`),
        });
      }).pipe(Effect.provide(computerChannelTestLayer({ connect: () => socket }))),
    );
    await Promise.resolve();
    socket.open();
    expect(sentFrames(socket)[0]).toEqual(["1", "1", "computer:machine-1", "phx_join", {}]);
    socket.message(["1", "1", "computer:machine-1", "phx_reply", { status: "ok", response: {} }]);
    expect(sentFrames(socket)[1]).toEqual(["1", "2", "computer:machine-1", "hello", options.hello]);

    socket.message(["1", null, "computer:machine-1", "probe", { request_id: "probe-1" }]);
    await Promise.resolve();
    expect(sentFrames(socket)).toContainEqual([
      "1",
      "3",
      "computer:machine-1",
      "probe_result",
      {
        request_id: "probe-1",
        probe: { schema: "openagents.computer_probe.v1", roots: ["/workspace"] },
      },
    ]);

    socket.message([
      "1",
      null,
      "computer:machine-1",
      "run",
      { request_id: "run-1", argv: ["echo", "hello"], cwd: "/workspace" },
    ]);
    runResponder?.chunk("hello\n");
    runResponder?.exit({ status: "completed", exit_code: 0 });
    socket.message(["1", null, "computer:machine-1", "cancel", { request_id: "run-1" }]);
    expect(sentFrames(socket)).toContainEqual([
      "1",
      "4",
      "computer:machine-1",
      "chunk",
      { request_id: "run-1", text: "hello\n" },
    ]);
    expect(sentFrames(socket)).toContainEqual([
      "1",
      "5",
      "computer:machine-1",
      "exit",
      { request_id: "run-1", status: "completed", exit_code: 0 },
    ]);
    expect(cancelled).toEqual(["run-1"]);
    expect(events).toContain("joined");
    expect(JSON.stringify(sentFrames(socket))).not.toContain("smct_test-secret");
    socket.message(["1", null, "computer:machine-1", "phx_close", {}]);
    await expect(channelRun).resolves.toBe("phx_close");
  });

  it("accepts agent frames and sends the ACP session before terminal output", async () => {
    const socket = new StubSocket();
    let agentResponder:
      | {
          readonly session: (sessionId: string) => void;
          readonly chunk: (text: string) => void;
          readonly exit: (payload: Record<string, unknown>) => void;
          readonly refused: (reason: string, detail: string) => void;
        }
      | undefined;
    const run = Effect.runPromise(
      Effect.gen(function* () {
        const channel = yield* ComputerChannel;
        return yield* channel.serve(options, {
          onProbe: async () => ({}),
          onRun: () => undefined,
          onAgent: (_requestId, _payload, responder) => {
            agentResponder = responder;
          },
          onCancel: () => undefined,
          onJoined: () => undefined,
          onEvent: () => undefined,
          onClosed: () => undefined,
        });
      }).pipe(Effect.provide(computerChannelTestLayer({ connect: () => socket }))),
    );
    await Promise.resolve();
    socket.open();
    socket.message(["1", "1", "computer:machine-1", "phx_reply", { status: "ok", response: {} }]);
    socket.message([
      "1",
      null,
      "computer:machine-1",
      "agent",
      { request_id: "agent-1", agent_id: "opencode", prompt: "work", cwd: "/workspace" },
    ]);
    agentResponder?.session("acp-session-1");
    agentResponder?.chunk("done");
    agentResponder?.exit({ status: "completed" });
    expect(sentFrames(socket)).toContainEqual([
      "1",
      "3",
      "computer:machine-1",
      "session",
      { request_id: "agent-1", session_id: "acp-session-1" },
    ]);
    expect(sentFrames(socket)).toContainEqual([
      "1",
      "4",
      "computer:machine-1",
      "chunk",
      { request_id: "agent-1", text: "done" },
    ]);
    expect(sentFrames(socket)).toContainEqual([
      "1",
      "5",
      "computer:machine-1",
      "exit",
      { request_id: "agent-1", status: "completed" },
    ]);
    socket.message(["1", null, "computer:machine-1", "phx_close", {}]);
    await expect(run).resolves.toBe("phx_close");
  });

  it("retries machine_reconnecting and stops on authorization refusals", async () => {
    vi.useFakeTimers();
    const reconnectingSockets: StubSocket[] = [];
    const reconnectingEvents: string[] = [];
    const reconnectingRun = Effect.runPromise(
      Effect.gen(function* () {
        const channel = yield* ComputerChannel;
        return yield* channel.serve(
          { ...options, reconnectBackoffMillis: 0, maximumReconnectAttempts: 1 },
          {
            onProbe: async () => ({}),
            onRun: () => undefined,
            onCancel: () => undefined,
            onJoined: () => undefined,
            onEvent: (event) => reconnectingEvents.push(event),
            onClosed: () => undefined,
          },
        );
      }).pipe(
        Effect.provide(
          computerChannelTestLayer({
            connect: () => {
              const socket = new StubSocket();
              reconnectingSockets.push(socket);
              return socket;
            },
          }),
        ),
      ),
    );
    await Promise.resolve();
    reconnectingSockets[0]?.open();
    reconnectingSockets[0]?.message([
      "1",
      "1",
      "computer:machine-1",
      "phx_reply",
      { status: "error", response: { reason: "machine_reconnecting" } },
    ]);
    await vi.advanceTimersByTimeAsync(0);
    reconnectingSockets[1]?.open();
    reconnectingSockets[1]?.message([
      "1",
      "1",
      "computer:machine-1",
      "phx_reply",
      { status: "error", response: { reason: "machine_unavailable" } },
    ]);
    await expect(reconnectingRun).resolves.toBe("join_refused:machine_unavailable");
    expect(reconnectingEvents).toContain("reconnect:join_refused:machine_reconnecting:1");

    for (const reason of ["machine_unavailable", "machine_mismatch"] as const) {
      const socket = new StubSocket();
      const result = Effect.runPromise(
        Effect.gen(function* () {
          const channel = yield* ComputerChannel;
          return yield* channel.serve(options, {
            onProbe: async () => ({}),
            onRun: () => undefined,
            onCancel: () => undefined,
            onJoined: () => undefined,
            onEvent: () => undefined,
            onClosed: () => undefined,
          });
        }).pipe(Effect.provide(computerChannelTestLayer({ connect: () => socket }))),
      );
      await Promise.resolve();
      socket.open();
      socket.message([
        "1",
        "1",
        "computer:machine-1",
        "phx_reply",
        { status: "error", response: { reason } },
      ]);
      await expect(result).resolves.toBe(`join_refused:${reason}`);
    }

    const transportSockets: StubSocket[] = [];
    const transportRun = Effect.runPromise(
      Effect.gen(function* () {
        const channel = yield* ComputerChannel;
        return yield* channel.serve(
          { ...options, reconnectBackoffMillis: 0, maximumReconnectAttempts: 1 },
          {
            onProbe: async () => ({}),
            onRun: () => undefined,
            onCancel: () => undefined,
            onJoined: () => undefined,
            onEvent: (event) => reconnectingEvents.push(event),
            onClosed: () => undefined,
          },
        );
      }).pipe(
        Effect.provide(
          computerChannelTestLayer({
            connect: () => {
              const socket = new StubSocket();
              transportSockets.push(socket);
              return socket;
            },
          }),
        ),
      ),
    );
    await Promise.resolve();
    transportSockets[0]?.open();
    transportSockets[0]?.close();
    await vi.advanceTimersByTimeAsync(0);
    transportSockets[1]?.open();
    transportSockets[1]?.close();
    await expect(transportRun).resolves.toBe("transport_retry_exhausted:closed");
  });

  it("ends after an unacknowledged heartbeat", async () => {
    vi.useFakeTimers();
    const socket = new StubSocket();
    const result = Effect.runPromise(
      Effect.gen(function* () {
        const channel = yield* ComputerChannel;
        return yield* channel.serve(
          { ...options, heartbeatMillis: 10 },
          {
            onProbe: async () => ({}),
            onRun: () => undefined,
            onCancel: () => undefined,
            onJoined: () => undefined,
            onEvent: () => undefined,
            onClosed: () => undefined,
          },
        );
      }).pipe(Effect.provide(computerChannelTestLayer({ connect: () => socket }))),
    );
    socket.open();
    await vi.advanceTimersByTimeAsync(20);
    expect(sentFrames(socket)).toContainEqual([null, "2", "phoenix", "heartbeat", {}]);
    await expect(result).resolves.toBe("heartbeat_timeout");
  });

  it("serves an accepted agent request through the local ACP process boundary", async () => {
    let handlers: ComputerChannelHandlers | undefined;
    const terminal: Array<Record<string, unknown>> = [];
    let startedEnvironment: Readonly<Record<string, string>> = {};
    const machineStatus: ComputerStatus = {
      machine_id: "machine-1",
      name: "test-computer",
      status: "active",
      token_expires_at: "2099-01-01T00:00:00.000Z",
    };
    const process: AgentProcess = {
      request: async (method) => {
        if (method === "initialize") return { agentCapabilities: {} };
        if (method === "session/new") return { sessionId: "session-up" };
        if (method === "session/prompt") return { stopReason: "end_turn" };
        return {};
      },
      notify: () => undefined,
      onNotification: () => () => undefined,
      onRequest: () => () => undefined,
      terminate: async () => undefined,
    };
    const channel = Layer.succeed(
      ComputerChannel,
      ComputerChannel.of({
        serve: (_options, value) => {
          handlers = value;
          value.onJoined();
          return Effect.succeed("phx_close");
        },
      }),
    );
    const processLayer = Layer.succeed(ComputerAgentProcess, {
      start: (_entry, _cwd, env) => {
        startedEnvironment = env;
        return Effect.succeed(process);
      },
    });
    const probe = Layer.succeed(
      ComputerProbe,
      ComputerProbe.of({
        probe: () =>
          Effect.succeed({
            schema: "openagents.computer_probe.v1" as const,
            host: {
              platform: "linux",
              release: "test",
              architecture: "x64",
              hostname: "test",
              shell: "",
              cpuCount: 1,
              totalMemoryBytes: 1,
              uptimeSeconds: 1,
            },
            codingAgents: [
              { name: "opencode", present: true, path: "/usr/bin/opencode", version: "1.2.3" },
            ],
            toolchains: [],
            roots: ["/workspace"],
            worktrees: [],
          }),
      }),
    );
    const layer = computerUpLayer.pipe(
      Layer.provide(
        Layer.mergeAll(
          channel,
          processLayer,
          Layer.succeed(
            ComputerClient,
            ComputerClient.of({
              start: () => Effect.die("unused"),
              wait: () => Effect.die("unused"),
              status: () => Effect.succeed(Option.some(machineStatus)),
            }),
          ),
          Layer.succeed(
            CredentialStore,
            CredentialStore.of({
              get: () => Effect.succeed(Option.some(Redacted.make("smct_test-secret"))),
              set: () => Effect.void,
              remove: () => Effect.void,
            }),
          ),
          Layer.succeed(
            ComputerJournal,
            ComputerJournal.of({
              append: (entry) => Effect.sync(() => terminal.push(entry)),
              read: () => Effect.succeed([]),
            }),
          ),
          probe,
          computerConfigurationTestLayer({ tier: "curated", roots: ["/workspace"] }),
        ),
      ),
    );
    await Effect.runPromise(
      Effect.gen(function* () {
        const up = yield* ComputerUp;
        return yield* up.serve("https://openagents.example", "0.2.1");
      }).pipe(Effect.provide(layer)),
    );
    const responderOutput: Array<Record<string, unknown>> = [];
    handlers?.onAgent?.(
      "agent-1",
      {
        request_id: "agent-1",
        agent_id: "opencode",
        prompt: "delegate",
        cwd: "/workspace",
        assignment_credential: "forge-secret",
        assignment_repository: "owner/repo",
        assignment_branch: "feature-1",
        env: { FORGE_TOKEN: "forge-secret" },
      },
      {
        session: () => undefined,
        chunk: () => undefined,
        exit: (value) => responderOutput.push(value),
        refused: (reason, detail) => responderOutput.push({ reason, detail }),
      },
    );
    for (let attempt = 0; attempt < 50 && responderOutput.length === 0; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    expect(responderOutput).toHaveLength(1);
    expect(responderOutput[0]).toMatchObject({ status: "completed", session_id: "session-up" });
    expect(startedEnvironment).not.toHaveProperty("FORGE_TOKEN");
    expect(JSON.stringify(terminal)).not.toContain("smct_test-secret");
    expect(JSON.stringify(terminal)).not.toContain("forge-secret");
    expect(terminal).toContainEqual(
      expect.objectContaining({
        decision: "credentials_delivered",
        outcome: "configured",
        detail: "scoped forge credentials configured for delegated push",
      }),
    );
  });
});

describe("Computer up service", () => {
  const probeReport = {
    schema: "openagents.computer_probe.v1",
    host: {
      platform: "linux",
      release: "test",
      architecture: "x64",
      hostname: "test",
      shell: "",
      cpuCount: 1,
      totalMemoryBytes: 1,
      uptimeSeconds: 1,
    },
    codingAgents: [],
    toolchains: [],
    roots: ["/workspace"],
    worktrees: [],
  } as const;
  const status: ComputerStatus = {
    machine_id: "machine-1",
    name: "test-computer",
    status: "active",
    token_expires_at: "2099-01-01T00:00:00.000Z",
  };

  const fixture = (tier: "probe" | "curated" | "shell", roots = ["/workspace"]) => {
    let channelHandlers: ComputerChannelHandlers | undefined;
    let channelOptions:
      | {
          readonly hello: unknown;
          readonly token: Redacted.Redacted<string>;
        }
      | undefined;
    const entries: Array<Record<string, unknown>> = [];
    const output: string[] = [];
    const terminal: Array<Record<string, unknown>> = [];
    const journal = Layer.succeed(
      ComputerJournal,
      ComputerJournal.of({
        append: (entry) => Effect.sync(() => entries.push(entry)),
        read: () => Effect.succeed([]),
      }),
    );
    const channel = Layer.succeed(
      ComputerChannel,
      ComputerChannel.of({
        serve: (options, handlers) => {
          channelOptions = options;
          channelHandlers = handlers;
          handlers.onJoined();
          return Effect.succeed("phx_close");
        },
      }),
    );
    const client = Layer.succeed(
      ComputerClient,
      ComputerClient.of({
        start: () => Effect.die("unused"),
        wait: () => Effect.die("unused"),
        status: () => Effect.succeed(Option.some(status)),
      }),
    );
    const credentials = Layer.succeed(
      CredentialStore,
      CredentialStore.of({
        get: () => Effect.succeed(Option.some(Redacted.make("smct_test-secret"))),
        set: () => Effect.void,
        remove: () => Effect.void,
      }),
    );
    const probe = Layer.succeed(
      ComputerProbe,
      ComputerProbe.of({ probe: () => Effect.succeed(probeReport) }),
    );
    const config = computerConfigurationTestLayer({
      tier,
      roots,
      preApproved: tier === "shell" ? ["node"] : [],
    });
    const layer = computerUpLayer.pipe(
      Layer.provide(Layer.mergeAll(channel, client, credentials, journal, probe, config)),
    );
    return {
      layer,
      entries,
      output,
      terminal,
      handlers: () => channelHandlers,
      hello: () => channelOptions?.hello,
      token: () => channelOptions?.token,
      emitRun: (payload: Record<string, unknown>) => {
        const handlers = channelHandlers;
        if (handlers === undefined) throw new Error("channel was not started");
        handlers.onRun("request-1", payload, {
          chunk: (text) => output.push(text),
          exit: (value) => terminal.push(value),
          refused: (reason, detail) => terminal.push({ reason, detail }),
        });
      },
    };
  };

  it("reports the CLI version, clamps limits, streams output, and journals outcomes", async () => {
    const value = fixture("shell", [process.cwd()]);
    await Effect.runPromise(
      Effect.gen(function* () {
        const up = yield* ComputerUp;
        return yield* up.serve("https://openagents.example", "0.1.7");
      }).pipe(Effect.provide(value.layer)),
    );
    expect(value.hello()).toMatchObject({ agent_version: "0.1.7", tier: "shell" });
    expect(value.token()).toBeDefined();
    value.emitRun({
      argv: [process.execPath, "-e", "process.stdout.write('abcdef')"],
      cwd: process.cwd(),
      tier: "shell",
      timeout_ms: 999_999,
      maximum_output_bytes: 3,
    });
    for (let attempt = 0; attempt < 50 && value.terminal.length === 0; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    expect(value.output.join("")).toBe("abc");
    expect(value.terminal[0]).toMatchObject({
      status: "completed",
      exit_code: 0,
      truncated: true,
    });
    expect(value.entries.map((entry) => entry.outcome)).toEqual(
      expect.arrayContaining(["pending", "running", "completed"]),
    );
    expect(value.entries.map((entry) => entry.decision)).toContain("allowed");
    expect(JSON.stringify(value.entries)).not.toContain("smct_test-secret");
  });

  it("refuses requests for tiers, roots, allowlists, and denied patterns", async () => {
    const value = fixture("probe");
    await Effect.runPromise(
      Effect.gen(function* () {
        const up = yield* ComputerUp;
        return yield* up.serve("https://openagents.example", "0.1.7");
      }).pipe(Effect.provide(value.layer)),
    );
    const cases = [
      {
        argv: ["echo", "hello"],
        cwd: "/workspace",
        tier: "shell",
        reason: "tier_insufficient",
      },
      {
        argv: ["node", "--version"],
        cwd: "/outside",
        tier: "probe",
        reason: "root_not_declared",
      },
      {
        argv: ["not-allowlisted"],
        cwd: "/workspace",
        tier: "curated",
        reason: "tier_insufficient",
      },
      {
        argv: ["cat", ".env"],
        cwd: "/workspace",
        tier: "probe",
        reason: "denied_argument",
      },
    ] as const;
    for (const request of cases) {
      value.emitRun(request);
      expect(value.terminal.at(-1)).toMatchObject({ reason: request.reason });
    }
    expect(value.entries.filter((entry) => entry.outcome === "refused")).toHaveLength(4);
    expect(JSON.stringify(value.entries)).not.toContain("smct_test-secret");
  });
});

describe("local Computer execution", () => {
  it("executes argv directly with scrubbed environment and bounded output", async () => {
    const chunks: string[] = [];
    const execution = executeComputerCommand(
      [
        process.execPath,
        "-e",
        "process.stdout.write((process.env.OPENAGENTS_COMPUTER_SECRET ?? 'missing') + 'x'.repeat(32))",
      ],
      process.cwd(),
      { timeoutMillis: 5_000, maximumOutputBytes: 8 },
      (chunk) => chunks.push(chunk),
    );
    const outcome = await execution.done;
    expect(outcome.exitCode).toBe(0);
    expect(outcome.truncated).toBe(true);
    expect(chunks.join("")).toBe("missingx");
    expect(chunks.join("")).not.toContain("oa_");
  });

  it("reports cancellation without fabricating an exit code", async () => {
    const execution = executeComputerCommand(
      [process.execPath, "-e", "setInterval(() => undefined, 10_000)"],
      process.cwd(),
      { timeoutMillis: 5_000, maximumOutputBytes: 64 },
      () => undefined,
    );
    execution.cancel();
    const outcome = await execution.done;
    expect(outcome.cancelled).toBe(true);
    expect(outcome.timedOut).toBe(false);
    expect(outcome.exitCode).toBe(null);
  });

  it("keeps a natural exit truthful when cancellation arrives afterward", async () => {
    const execution = executeComputerCommand(
      [process.execPath, "-e", "process.exit(0)"],
      process.cwd(),
      { timeoutMillis: 5_000, maximumOutputBytes: 64 },
      () => undefined,
    );
    const outcome = await execution.done;
    execution.cancel();
    expect(outcome.cancelled).toBe(false);
    expect(outcome.timedOut).toBe(false);
    expect(outcome.exitCode).toBe(0);
  });
});

describe("Computer CLI output", () => {
  const output = (
    documents: Array<{ readonly document: OutputDocument; readonly mode: OutputMode }>,
  ) =>
    outputTestLayer((document, mode) =>
      Effect.sync(() => {
        documents.push({ document, mode });
      }),
    );

  it("prints stable JSON policy and status without auth or network", async () => {
    const documents: Array<{ readonly document: OutputDocument; readonly mode: OutputMode }> = [];
    const credentialPath = join("/tmp", "openagents-cli-status-credentials.json");
    const layer = Layer.mergeAll(
      computerConfigurationTestLayer({ roots: [] }),
      output(documents),
      NodeServices.layer,
      environmentLayerFromValues({}),
      persistedConfigurationTestLayer({}),
      credentialStoreTestFileLayer(credentialPath),
      pendingDeviceAuthorizationStoreTestLayer(),
    );
    await Effect.runPromise(
      runCliWith(["--json", "computer", "policy"]).pipe(Effect.provide(layer)),
    );
    await Effect.runPromise(
      runCliWith(["--json", "computer", "status"]).pipe(Effect.provide(layer)),
    );
    expect(documents).toHaveLength(2);
    expect(documents[0]?.mode).toBe("json");
    expect(documents[0]?.document.value).toMatchObject({
      schema: "openagents.computer_policy.v1",
      tier: "probe",
      roots: [],
      authority: "local_machine",
      network: false,
    });
    expect(documents[1]?.document.value).toMatchObject({
      schema: "openagents.computer_status.v1",
      journal_retention_bytes: journalMaxBytes,
      journal_read_tail_bytes: journalReadTailBytes,
    });
    expect(JSON.stringify(documents)).not.toContain("oa_pat_");
    expect(JSON.stringify(documents)).not.toContain("oa_machine_");
    await rm(credentialPath, { force: true });
  });

  it("prints stable JSON probe output without auth or network", async () => {
    const documents: Array<{ readonly document: OutputDocument; readonly mode: OutputMode }> = [];
    const report = {
      schema: "openagents.computer_probe.v1" as const,
      host: {
        platform: "test",
        release: "test",
        architecture: "test",
        hostname: "test",
        shell: "",
        cpuCount: 1,
        totalMemoryBytes: 1,
        uptimeSeconds: 1,
      },
      codingAgents: [],
      toolchains: [],
      roots: [],
      worktrees: [],
    };
    const layer = Layer.mergeAll(
      computerConfigurationTestLayer({ roots: [] }),
      Layer.succeed(ComputerProbe, ComputerProbe.of({ probe: () => Effect.succeed(report) })),
      output(documents),
      NodeServices.layer,
    );
    await Effect.runPromise(
      runCliWith(["--json", "computer", "probe"]).pipe(Effect.provide(layer)),
    );
    expect(documents[0]?.mode).toBe("json");
    expect(documents[0]?.document.value).toEqual(report);
  });

  it("prints an absent journal as stable JSON", async () => {
    const documents: Array<{ readonly document: OutputDocument; readonly mode: OutputMode }> = [];
    const directory = await mkdtemp(join("/tmp", "openagents-cli-journal-cli-"));
    try {
      const layer = Layer.mergeAll(
        computerConfigurationTestLayer({ paths: computerPaths(join(directory, "config.json")) }),
        computerJournalTestLayer(join(directory, "journal.ndjson")),
        output(documents),
        NodeServices.layer,
      );
      await Effect.runPromise(
        runCliWith(["--json", "computer", "journal"]).pipe(Effect.provide(layer)),
      );
      expect(documents[0]?.document.value).toEqual({
        schema: "openagents.computer_journal.v1",
        entries: [],
      });
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });
});

import { connect, type Socket } from "node:net";

import { Effect, Stream } from "effect";
import {
  decodeKhalaRuntimeEvent,
  KhalaRuntimeEventSchemaLiteral,
  type KhalaRuntimeSource,
} from "@openagentsinc/agent-runtime-schema";
import {
  makeAcpHarnessAdapter,
  type AcpAdapterEvent,
  type AcpTransport,
} from "@openagentsinc/agent-harness-contract";
import type { AgentHarness } from "@openagentsinc/agent-harness-contract";
import { HarnessCapabilityUnsupported } from "@openagentsinc/agent-harness-contract";
import type { HarnessSession } from "@openagentsinc/agent-harness-contract";
import type { HarnessStreamEvent } from "@openagentsinc/agent-harness-contract";
import type { HarnessToolApprovalDecision } from "@openagentsinc/agent-harness-contract";
import { describe, expect, test } from "vite-plus/test";

import {
  denyMetaAgentAcpPermission,
  isMetaAgentAcpServerEnabled,
  makeFixtureMetaAgentHarness,
  META_AGENT_ACP_SERVER_ENV_FLAG,
  startMetaAgentAcpServer,
  startMetaAgentAcpServerIfEnabled,
} from "./meta-agent-acp-server.ts";

const SOURCE: KhalaRuntimeSource = { lane: "test_fixture" };

interface JsonRecord {
  readonly [key: string]: unknown;
}

const asRecord = (value: unknown): JsonRecord | null =>
  typeof value === "object" && value !== null ? (value as JsonRecord) : null;

const asString = (value: unknown): string => (typeof value === "string" ? value : "");

const sequences = (events: ReadonlyArray<HarnessStreamEvent>) => events.map((e) => e.sequence);

const stopReasonOf = (events: ReadonlyArray<AcpAdapterEvent>): string => {
  const last = events.at(-1);
  return last?.type === "acp_turn_stop" ? last.stopReason : "missing";
};

/**
 * A real TCP ACP CLIENT over the loopback server: newline-delimited JSON-RPC,
 * projecting `session/update` notifications into the same `AcpAdapterEvent`
 * shapes the live grok/cursor transports produce, and answering
 * `session/request_permission` when asked. This is the desktop-side mirror of
 * the SDK's in-memory loopback client — the conformance oracle for the server.
 */
interface TcpAcpClient {
  readonly request: (method: string, params: unknown) => Promise<JsonRecord>;
  readonly updates: Array<AcpAdapterEvent>;
  readonly permissionRequests: Array<JsonRecord>;
  readonly close: () => Promise<void>;
}

const connectTcpAcpClient = (
  port: number,
  options?: { readonly permissionOptionId?: string },
): Promise<TcpAcpClient> =>
  new Promise<TcpAcpClient>((resolve, reject) => {
    const socket: Socket = connect({ port, host: "127.0.0.1" });
    socket.setEncoding("utf8");
    const pending = new Map<number, (value: JsonRecord) => void>();
    const startedCalls = new Map<string, string>();
    const updates: Array<AcpAdapterEvent> = [];
    const permissionRequests: Array<JsonRecord> = [];
    let nextId = 1;
    let buffer = "";

    const write = (message: unknown): void => {
      if (!socket.destroyed) socket.write(`${JSON.stringify(message)}\n`);
    };

    const handleServerMessage = (message: unknown): void => {
      const record = asRecord(message);
      if (record === null) return;
      const method = asString(record.method);

      if (method === "session/update") {
        const params = asRecord(record.params) ?? {};
        const update = asRecord(params.update) ?? {};
        switch (asString(update.sessionUpdate)) {
          case "agent_message_chunk": {
            const content = asRecord(update.content) ?? {};
            updates.push({ type: "acp_text_delta", text: asString(content.text) });
            break;
          }
          case "agent_thought_chunk": {
            const content = asRecord(update.content) ?? {};
            updates.push({ type: "acp_thought_delta", text: asString(content.text) });
            break;
          }
          case "tool_call": {
            const toolCallId = asString(update.toolCallId);
            const toolName = asString(update.title) || "tool";
            startedCalls.set(toolCallId, toolName);
            updates.push({ type: "acp_tool_call", toolCallId, toolName });
            break;
          }
          case "tool_call_update": {
            const toolCallId = asString(update.toolCallId);
            updates.push({
              type: "acp_tool_result",
              toolCallId,
              toolName: startedCalls.get(toolCallId) ?? "tool",
              ok: asString(update.status) === "completed",
            });
            break;
          }
          default:
            break;
        }
        return;
      }

      if (method === "session/request_permission" && record.id !== undefined) {
        permissionRequests.push(asRecord(record.params) ?? {});
        write({
          jsonrpc: "2.0",
          id: record.id,
          result: {
            outcome: {
              outcome: "selected",
              optionId: options?.permissionOptionId ?? "allow-once",
            },
          },
        });
        return;
      }

      if (method === "" && typeof record.id === "number") {
        const resolvePending = pending.get(record.id);
        if (resolvePending !== undefined) {
          pending.delete(record.id);
          resolvePending(asRecord(record.result) ?? {});
        }
      }
    };

    socket.on("data", (chunk: string) => {
      buffer += chunk;
      let newlineIndex = buffer.indexOf("\n");
      while (newlineIndex !== -1) {
        const line = buffer.slice(0, newlineIndex).trim();
        buffer = buffer.slice(newlineIndex + 1);
        if (line.length > 0) {
          try {
            handleServerMessage(JSON.parse(line));
          } catch {
            // ignore non-JSON
          }
        }
        newlineIndex = buffer.indexOf("\n");
      }
    });

    socket.once("error", reject);
    socket.on("connect", () => {
      resolve({
        request: (method, params) =>
          new Promise<JsonRecord>((resolveRequest) => {
            const id = nextId++;
            pending.set(id, resolveRequest);
            write({ jsonrpc: "2.0", id, method, params });
          }),
        updates,
        permissionRequests,
        close: () =>
          new Promise<void>((resolveClose) => {
            socket.end(() => resolveClose());
            socket.destroy();
          }),
      });
    });
  });

/** Handshake over the TCP loopback and return an {@link AcpTransport}. */
const makeTcpTransport = (port: number, options?: { readonly permissionOptionId?: string }) =>
  Effect.gen(function* () {
    const client = yield* Effect.promise(() => connectTcpAcpClient(port, options));
    const init = yield* Effect.promise(() => client.request("initialize", { protocolVersion: 1 }));
    expect(init.protocolVersion).toBe(1);
    const session = yield* Effect.promise(() =>
      client.request("session/new", { cwd: "/tmp", mcpServers: [] }),
    );
    const sessionId = asString(session.sessionId);
    expect(sessionId).not.toBe("");

    const transport: AcpTransport = {
      promptTurn: (params) =>
        Effect.gen(function* () {
          client.updates.length = 0;
          const result = yield* Effect.promise(() =>
            client.request("session/prompt", {
              sessionId,
              prompt: [{ type: "text", text: params.prompt }],
            }),
          );
          const stopReason = asString(result.stopReason) || "end_turn";
          return [
            { type: "acp_turn_started" } as const,
            ...client.updates,
            { type: "acp_turn_stop", stopReason } as const,
          ];
        }),
      shutdown: () => Effect.promise(() => client.close()),
    };
    return { transport, client, sessionId };
  });

// ---------------------------------------------------------------------------
// Gate: default OFF.
// ---------------------------------------------------------------------------

describe("meta-agent ACP server — gate is default OFF (#9181)", () => {
  test("the env flag is off unless explicitly set to 1", () => {
    expect(isMetaAgentAcpServerEnabled({})).toBe(false);
    expect(isMetaAgentAcpServerEnabled({ [META_AGENT_ACP_SERVER_ENV_FLAG]: "0" })).toBe(false);
    expect(isMetaAgentAcpServerEnabled({ [META_AGENT_ACP_SERVER_ENV_FLAG]: "true" })).toBe(false);
    expect(isMetaAgentAcpServerEnabled({ [META_AGENT_ACP_SERVER_ENV_FLAG]: "1" })).toBe(true);
  });

  test("startMetaAgentAcpServerIfEnabled resolves null when the gate is off", async () => {
    const server = await startMetaAgentAcpServerIfEnabled({});
    expect(server).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Loopback-only by construction.
// ---------------------------------------------------------------------------

describe("meta-agent ACP server — loopback-only", () => {
  test("refuses to bind a non-loopback host (construction invariant)", () => {
    expect(() => startMetaAgentAcpServer({ host: "0.0.0.0" })).toThrow(/loopback-only/);
  });

  test("binds 127.0.0.1 on an ephemeral port", async () => {
    const server = await startMetaAgentAcpServer();
    try {
      expect(server.host).toBe("127.0.0.1");
      expect(server.port).toBeGreaterThan(0);
      expect(server.url).toBe(`tcp://127.0.0.1:${server.port}`);
    } finally {
      await server.stop();
    }
  });
});

// ---------------------------------------------------------------------------
// Conformance: the SDK's own ACP CLIENT adapter drives the loopback SERVER over
// a real TCP socket and the composed harness passes the contiguous-stream law.
// ---------------------------------------------------------------------------

describe("meta-agent ACP server — conformance via the SDK ACP client adapter (TCP loopback)", () => {
  test("a full turn through the loopback server + client adapter is a contiguous khala stream", async () => {
    const server = await startMetaAgentAcpServer();
    try {
      const result = await Effect.runPromise(
        Effect.gen(function* () {
          const { transport } = yield* makeTcpTransport(server.port);
          const adapter = makeAcpHarnessAdapter({
            harnessId: "openagents-desktop-loopback",
            harnessKind: "custom",
            transport,
          });
          const session = yield* adapter.start({ sessionId: "s1", source: SOURCE });
          const control = yield* session.promptTurn({ turnId: "t1", prompt: "hi" });
          const events = yield* Stream.runCollect(control.events);
          const done = yield* control.done;
          yield* transport.shutdown();
          return { events, done };
        }),
      );

      expect(result.events[0]?.kind).toBe("turn.started");
      expect(result.events.at(-1)?.kind).toBe("turn.finished");
      // turn.started + 4 fixture text deltas + turn.finished, contiguous from 0.
      expect(sequences(result.events)).toEqual([0, 1, 2, 3, 4, 5]);
      expect(result.events.filter((e) => e.kind === "text.delta").length).toBe(4);
      expect(result.done.finishReason).toBe("stop");
      expect(result.done.lastCursor).toBe(5);
    } finally {
      await server.stop();
    }
  });

  test("consecutive prompt turns each settle with their own stop reason", async () => {
    const server = await startMetaAgentAcpServer();
    try {
      const stopReasons = await Effect.runPromise(
        Effect.gen(function* () {
          const { transport } = yield* makeTcpTransport(server.port);
          const first = yield* transport.promptTurn({ prompt: "one" });
          const second = yield* transport.promptTurn({ prompt: "two" });
          yield* transport.shutdown();
          return [stopReasonOf(first), stopReasonOf(second)];
        }),
      );
      expect(stopReasons).toEqual(["end_turn", "end_turn"]);
    } finally {
      await server.stop();
    }
  });

  test("the v0 backing is the real SDK metaAgentHarness (fleet contract), not a bespoke stub", () => {
    const harness = makeFixtureMetaAgentHarness();
    // metaAgentHarness advertises the canonical fleet identity.
    expect(harness.specificationVersion).toBe("agent-harness-v1");
    expect(harness.harnessId).toBe("openagents");
  });
});

// ---------------------------------------------------------------------------
// Deny-by-default: a gated tool call is refused, never executed, even though
// the wire still carries the tool_call update.
// ---------------------------------------------------------------------------

/** Fixture harness whose single turn asks for a gated built-in tool approval. */
const makeApprovalFixtureHarness = (
  recorded: Array<{ toolCallId: string; decision: HarnessToolApprovalDecision }>,
): AgentHarness => {
  const harnessId = "approval-fixture";
  const base = (sessionId: string, turnId: string, sequence: number, suffix: string) => ({
    schema: KhalaRuntimeEventSchemaLiteral,
    eventId: `evt.${turnId}.${sequence}.${suffix}`,
    turnId,
    threadId: sessionId,
    sequence,
    observedAt: "2026-07-20T00:00:00.000Z",
    source: SOURCE,
    visibility: "private",
    redactionClass: "private_ref",
    causalityRefs: [] as ReadonlyArray<string>,
  });
  const unsupported = (capability: "suspend_turn" | "continue_turn" | "detach" | "compact") =>
    Effect.fail(new HarnessCapabilityUnsupported({ harnessId, capability }));
  return {
    specificationVersion: "agent-harness-v1",
    harnessId,
    harnessKind: "test_fixture",
    adapterKind: "test_fixture",
    builtinTools: [{ nativeName: "Bash", commonName: "bash", description: "run a shell command" }],
    start: ({ sessionId }) =>
      Effect.sync(() => {
        const session: HarnessSession = {
          sessionId,
          isResume: false,
          promptTurn: ({ turnId }) =>
            Effect.sync(() => {
              const events: ReadonlyArray<HarnessStreamEvent> = [
                decodeKhalaRuntimeEvent({
                  ...base(sessionId, turnId, 0, "start"),
                  kind: "turn.started",
                }),
                decodeKhalaRuntimeEvent({
                  ...base(sessionId, turnId, 1, "toolcall"),
                  kind: "tool.call",
                  toolCallId: "toolcall.fix.1",
                  toolName: "bash",
                  authority: {
                    authorityRef: "authority.fix.1",
                    policyRef: "policy.fixture",
                    decisionRef: "decision.fixture.pending",
                    toolRef: "toolref.fixture.bash",
                    status: "operator_escalation_required",
                    allowed: false,
                    blockerRefs: ["blocker.owner_approval"],
                  },
                }),
                decodeKhalaRuntimeEvent({
                  ...base(sessionId, turnId, 2, "finish"),
                  kind: "turn.finished",
                  finishReason: "stop",
                }),
              ];
              return {
                turnId,
                events: Stream.fromIterable(events),
                done: Effect.succeed({ turnId, finishReason: "stop" as const, lastCursor: 2 }),
                submitToolResult: () => Effect.void,
                submitToolApproval: (toolCallId, decision) =>
                  Effect.sync(() => {
                    recorded.push({ toolCallId, decision });
                  }),
                submitUserMessage: () => Effect.void,
                interrupt: () => Effect.void,
              };
            }),
          continueTurn: () => unsupported("continue_turn"),
          suspendTurn: () => unsupported("suspend_turn"),
          compact: () => unsupported("compact"),
          detach: () => unsupported("detach"),
          stop: () => Effect.succeed({ harnessId, sessionId, data: {} }),
          destroy: () => Effect.void,
        };
        return session;
      }),
  };
};

describe("meta-agent ACP server — deny-by-default permissioning (#9181)", () => {
  test("the default decider denies every gated tool call", async () => {
    const decision = await Effect.runPromise(
      denyMetaAgentAcpPermission({
        sessionId: "s1",
        payload: {
          kind: "tool_approval",
          displayText: "Allow the agent to run bash?",
          toolCallId: "toolcall.fix.1",
          toolName: "bash",
          authority: {
            authorityRef: "authority.fix.1",
            policyRef: "policy.fixture",
            decisionRef: "decision.fixture.pending",
            toolRef: "toolref.fixture.bash",
            status: "operator_escalation_required",
            allowed: false,
            blockerRefs: ["blocker.owner_approval"],
          },
        },
        askClient: Effect.succeed("allow-once"),
      }),
    );
    expect(decision).toBe("deny");
  });

  test("a gated tool call over the loopback server is denied, never executed", async () => {
    const recorded: Array<{ toolCallId: string; decision: HarnessToolApprovalDecision }> = [];
    // Default decider (deny). The client would ANSWER allow-once, but the server
    // must never even ask, because the desktop decider denies first.
    const server = await startMetaAgentAcpServer({
      harness: makeApprovalFixtureHarness(recorded),
    });
    try {
      const outcome = await Effect.runPromise(
        Effect.gen(function* () {
          const { transport, client } = yield* makeTcpTransport(server.port, {
            permissionOptionId: "allow-once",
          });
          const events = yield* transport.promptTurn({ prompt: "run it" });
          yield* transport.shutdown();
          return { events, permissionRequests: client.permissionRequests };
        }),
      );
      // Deny-by-default: the tool was refused, so the recorded decision is deny,
      // and the desktop decider never delegated the question to the ACP client.
      expect(recorded).toEqual([{ toolCallId: "toolcall.fix.1", decision: "deny" }]);
      expect(outcome.permissionRequests.length).toBe(0);
      // The wire still carried the tool_call update (honest surface, no bypass).
      expect(outcome.events.some((e) => e.type === "acp_tool_call")).toBe(true);
    } finally {
      await server.stop();
    }
  });
});

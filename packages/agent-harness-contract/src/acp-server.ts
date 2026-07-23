import { Deferred, Effect, Fiber, Ref, Stream } from "effect";
import type {
  KhalaRuntimeFinishReason,
  KhalaRuntimeSource,
  RuntimeInteractionPayload,
} from "@openagentsinc/agent-runtime-schema";
import type { AgentHarness } from "./adapter.ts";
import type { HarnessToolApprovalDecision } from "./host-tool.ts";
import type { HarnessPromptControl, HarnessSession } from "./session.ts";
import type { HarnessStreamEvent } from "./stream.ts";

/**
 * ACP agent SERVER helper (ai#39): the inversion of `acp-adapter.ts`. Where the
 * adapter makes us an ACP CLIENT of a peer agent, this helper exposes ANY
 * {@link AgentHarness} (including `metaAgentHarness`) AS an ACP agent, so an
 * external ACP host (Zed, our own ACP client adapter, …) can drive it:
 *
 * - `initialize` / `authenticate` — handshake; no auth methods are advertised
 *   (the host injected an already-configured harness; the SDK holds no
 *   credentials).
 * - `session/new` — starts a harness session.
 * - `session/prompt` — runs one prompt turn; harness stream events are
 *   projected onto `session/update` notifications
 *   (`agent_message_chunk` / `agent_thought_chunk` / `tool_call` /
 *   `tool_call_update`), and the response settles with the ACP `stopReason`
 *   mapped from the turn's finish reason.
 * - `session/cancel` — interrupts the in-flight turn.
 *
 * Permission requests are mapped onto the canonical `RuntimeInteraction`
 * model: a harness `tool.call` whose authority is
 * `operator_escalation_required` becomes a `tool_approval`
 * {@link RuntimeInteractionPayload}; the default decision path asks the
 * connected ACP client via `session/request_permission` and DENIES on any
 * unclear, failed, or timed-out outcome (fail-closed, deny-by-default). Hosts
 * may override `decidePermission` to route the payload through their own
 * durable interaction store — the deny-by-default posture is theirs to keep.
 *
 * Transport-neutral: the helper speaks message OBJECTS through `send` and
 * `receive`, so the same connection logic runs over an in-memory loopback in
 * tests or a line-delimited JSON socket in a host process. The in-repo ACP
 * client adapter is the conformance oracle: drive this server with
 * `makeAcpHarnessAdapter` over a loopback transport and the composed harness
 * passes the same law suites as every other adapter (see `acp-server.test.ts`).
 */

interface JsonRecord {
  readonly [key: string]: unknown;
}

const asRecord = (value: unknown): JsonRecord | null =>
  typeof value === "object" && value !== null ? (value as JsonRecord) : null;

const asString = (value: unknown): string => (typeof value === "string" ? value : "");

/** ACP wire protocol version this server speaks. */
export const ACP_SERVER_PROTOCOL_VERSION = 1;

/** Map a harness finish reason onto the ACP `stopReason` vocabulary. */
export const finishReasonToStopReason = (finishReason: KhalaRuntimeFinishReason): string =>
  finishReason === "stop"
    ? "end_turn"
    : finishReason === "length"
      ? "max_tokens"
      : finishReason === "cancelled" || finishReason === "interrupted"
        ? "cancelled"
        : finishReason === "content-filter"
          ? "refusal"
          : "end_turn";

/** Context handed to a host-supplied permission decider. */
export interface AcpServerPermissionRequest {
  readonly sessionId: string;
  /** The canonical durable interaction payload for this approval. */
  readonly payload: Extract<RuntimeInteractionPayload, { readonly kind: "tool_approval" }>;
  /**
   * Ask the connected ACP client via `session/request_permission`. Resolves
   * `deny` on any unclear outcome, protocol failure, or timeout (fail-closed).
   */
  readonly askClient: Effect.Effect<HarnessToolApprovalDecision>;
}

export interface AcpAgentServerOptions {
  /** The harness this connection exposes as an ACP agent. */
  readonly harness: AgentHarness;
  /** Outbound message sink (notifications, requests, responses) to the ACP client. */
  readonly send: (message: unknown) => Effect.Effect<void>;
  /** Event source labelling for harness sessions started by this connection. */
  readonly source?: KhalaRuntimeSource;
  /**
   * Permission decision seam. Defaults to `(request) => request.askClient`
   * (ask the connected client, deny on anything unclear).
   */
  readonly decidePermission?: (
    request: AcpServerPermissionRequest,
  ) => Effect.Effect<HarnessToolApprovalDecision>;
  /** Milliseconds to wait for a client permission outcome before denying. Default 60000. */
  readonly permissionTimeoutMillis?: number;
}

/** One ACP server connection: feed inbound client messages to `receive`. */
export interface AcpAgentServerConnection {
  /** Handle one inbound message object from the ACP client. */
  readonly receive: (message: unknown) => Effect.Effect<void>;
  /** Interrupt in-flight turns and destroy every session this connection started. */
  readonly shutdown: () => Effect.Effect<void>;
}

interface ActivePrompt {
  readonly control: HarnessPromptControl;
  readonly fiber: Fiber.Fiber<void, never>;
}

/**
 * Build one ACP agent server connection over the given harness. Connection
 * state is message-serial: the host must deliver inbound messages for one
 * connection sequentially (both the in-memory loopback and a line-delimited
 * socket reader do this naturally).
 */
export const makeAcpAgentServerConnection = (
  options: AcpAgentServerOptions,
): Effect.Effect<AcpAgentServerConnection> =>
  Effect.gen(function* () {
    const source: KhalaRuntimeSource = options.source ?? { lane: "agent_client_protocol" };
    const permissionTimeoutMillis = options.permissionTimeoutMillis ?? 60_000;

    const sessions = new Map<string, HarnessSession>();
    const activePrompts = new Map<string, ActivePrompt>();
    /** Server-originated request ids awaiting a client response. */
    const pendingClientResponses = new Map<number, Deferred.Deferred<JsonRecord, never>>();
    const counterRef = yield* Ref.make(0);
    const nextCount = Ref.getAndUpdate(counterRef, (n) => n + 1);

    const respond = (id: unknown, result: unknown): Effect.Effect<void> =>
      options.send({ jsonrpc: "2.0", id, result });

    const respondError = (id: unknown, code: number, message: string): Effect.Effect<void> =>
      options.send({ jsonrpc: "2.0", id, error: { code, message } });

    const notify = (method: string, params: unknown): Effect.Effect<void> =>
      options.send({ jsonrpc: "2.0", method, params });

    /** Send a server→client request and await the client's response (or empty on timeout). */
    const requestClient = (method: string, params: unknown): Effect.Effect<JsonRecord> =>
      Effect.gen(function* () {
        const id = 1_000_000 + (yield* nextCount);
        const deferred = yield* Deferred.make<JsonRecord, never>();
        pendingClientResponses.set(id, deferred);
        yield* options.send({ jsonrpc: "2.0", id, method, params });
        return yield* Deferred.await(deferred).pipe(
          Effect.timeout(permissionTimeoutMillis),
          Effect.catch(() =>
            Effect.sync(() => {
              pendingClientResponses.delete(id);
              return {} as JsonRecord;
            }),
          ),
        );
      });

    /** Deny-by-default outcome mapping for `session/request_permission`. */
    const outcomeToDecision = (result: JsonRecord): HarnessToolApprovalDecision => {
      const outcome = asRecord(result.outcome);
      if (outcome === null || asString(outcome.outcome) !== "selected") return "deny";
      const optionId = asString(outcome.optionId);
      if (optionId === "allow-once") return "allow-once";
      if (optionId === "allow-always") return "allow-session";
      return "deny";
    };

    const askClientPermission = (
      sessionId: string,
      payload: Extract<RuntimeInteractionPayload, { readonly kind: "tool_approval" }>,
    ): Effect.Effect<HarnessToolApprovalDecision> =>
      requestClient("session/request_permission", {
        sessionId,
        toolCall: {
          toolCallId: payload.toolCallId,
          title: payload.toolName,
        },
        options: [
          { optionId: "allow-once", name: "Allow once", kind: "allow_once" },
          { optionId: "allow-always", name: "Allow for this session", kind: "allow_always" },
          { optionId: "reject-once", name: "Reject", kind: "reject_once" },
        ],
      }).pipe(Effect.map(outcomeToDecision));

    /** Project one harness stream event onto `session/update` notifications. */
    const handleEvent = (
      sessionId: string,
      control: HarnessPromptControl,
      event: HarnessStreamEvent,
    ): Effect.Effect<void> =>
      Effect.gen(function* () {
        switch (event.kind) {
          case "text.delta":
            yield* notify("session/update", {
              sessionId,
              update: {
                sessionUpdate: "agent_message_chunk",
                content: { type: "text", text: event.text },
              },
            });
            return;
          case "reasoning.delta":
            yield* notify("session/update", {
              sessionId,
              update: {
                sessionUpdate: "agent_thought_chunk",
                content: { type: "text", text: event.text },
              },
            });
            return;
          case "tool.call": {
            yield* notify("session/update", {
              sessionId,
              update: {
                sessionUpdate: "tool_call",
                toolCallId: event.toolCallId,
                title: event.toolName,
                kind: "other",
                status: "pending",
              },
            });
            if (event.authority.status === "operator_escalation_required") {
              // Map the approval onto the canonical RuntimeInteraction payload
              // and decide it fail-closed: deny unless the decider says allow.
              const payload: Extract<
                RuntimeInteractionPayload,
                { readonly kind: "tool_approval" }
              > = {
                kind: "tool_approval",
                displayText: `Allow the agent to run ${event.toolName}?`,
                toolCallId: event.toolCallId,
                toolName: event.toolName,
                authority: event.authority,
              };
              const askClient = askClientPermission(sessionId, payload);
              const decide =
                options.decidePermission === undefined
                  ? askClient
                  : options.decidePermission({ sessionId, payload, askClient });
              const decision = yield* decide.pipe(
                Effect.catchCause(() => Effect.succeed("deny" as const)),
              );
              // Submit the decision; a harness that cannot take it stays denied
              // by construction (nothing executes without an accepted approval).
              yield* control
                .submitToolApproval(event.toolCallId, decision)
                .pipe(Effect.catchCause(() => Effect.void));
            }
            return;
          }
          case "tool.result":
            yield* notify("session/update", {
              sessionId,
              update: {
                sessionUpdate: "tool_call_update",
                toolCallId: event.toolCallId,
                status: "completed",
              },
            });
            return;
          case "tool.error":
            yield* notify("session/update", {
              sessionId,
              update: {
                sessionUpdate: "tool_call_update",
                toolCallId: event.toolCallId,
                status: "failed",
              },
            });
            return;
          default:
            // turn framing, agent.child.*, usage, and other kinds have no ACP
            // wire vocabulary; the prompt response carries the stop reason.
            return;
        }
      });

    const runPrompt = (id: unknown, params: JsonRecord): Effect.Effect<void> =>
      Effect.gen(function* () {
        const sessionId = asString(params.sessionId);
        const session = sessions.get(sessionId);
        if (session === undefined) {
          yield* respondError(id, -32602, `unknown session ${JSON.stringify(sessionId)}`);
          return;
        }
        if (activePrompts.has(sessionId)) {
          yield* respondError(id, -32600, "a prompt turn is already running for this session");
          return;
        }
        const blocks = Array.isArray(params.prompt) ? params.prompt : [];
        const prompt = blocks
          .map((block) => {
            const record = asRecord(block);
            return record !== null && asString(record.type) === "text" ? asString(record.text) : "";
          })
          .join("");
        const turnId = `turn.acp.${yield* nextCount}`;

        const turn = Effect.gen(function* () {
          const control = yield* session.promptTurn({ turnId, prompt });
          const fiber = yield* Effect.forkDetach(
            Effect.gen(function* () {
              yield* control.events.pipe(
                Stream.runForEach((event) => handleEvent(sessionId, control, event)),
              );
              const result = yield* control.done;
              yield* respond(id, { stopReason: finishReasonToStopReason(result.finishReason) });
            }).pipe(
              Effect.catchCause(() =>
                respondError(id, -32603, "prompt turn failed").pipe(
                  Effect.catchCause(() => Effect.void),
                ),
              ),
              Effect.ensuring(Effect.sync(() => activePrompts.delete(sessionId))),
            ),
          );
          activePrompts.set(sessionId, { control, fiber });
        });

        yield* turn.pipe(Effect.catchCause(() => respondError(id, -32603, "prompt failed")));
      });

    const receive = (message: unknown): Effect.Effect<void> =>
      Effect.gen(function* () {
        const record = asRecord(message);
        if (record === null) return;
        const method = asString(record.method);

        // A response to a server-originated request (permission outcome).
        if (method === "" && typeof record.id === "number") {
          const deferred = pendingClientResponses.get(record.id);
          if (deferred !== undefined) {
            pendingClientResponses.delete(record.id);
            yield* Deferred.succeed(deferred, asRecord(record.result) ?? {});
          }
          return;
        }

        const params = asRecord(record.params) ?? {};
        switch (method) {
          case "initialize":
            yield* respond(record.id, {
              protocolVersion: ACP_SERVER_PROTOCOL_VERSION,
              agentCapabilities: { loadSession: false },
              authMethods: [],
            });
            return;
          case "authenticate":
            yield* respond(record.id, {});
            return;
          case "session/new": {
            const sessionId = `acp.session.${yield* nextCount}`;
            yield* options.harness.start({ sessionId, source }).pipe(
              Effect.flatMap((session) =>
                Effect.gen(function* () {
                  sessions.set(sessionId, session);
                  yield* respond(record.id, { sessionId });
                }),
              ),
              Effect.catch((error) =>
                respondError(record.id, -32603, `session start failed: ${error.failureClass}`),
              ),
            );
            return;
          }
          case "session/prompt":
            yield* runPrompt(record.id, params);
            return;
          case "session/cancel": {
            const sessionId = asString(params.sessionId);
            const active = activePrompts.get(sessionId);
            if (active !== undefined) {
              yield* active.control.interrupt();
              yield* Fiber.interrupt(active.fiber);
              activePrompts.delete(sessionId);
            }
            return;
          }
          default:
            if (record.id !== undefined) {
              yield* respondError(record.id, -32601, `method not found: ${method}`);
            }
            return;
        }
      });

    const shutdown = (): Effect.Effect<void> =>
      Effect.gen(function* () {
        for (const [, active] of activePrompts) {
          yield* active.control.interrupt();
          yield* Fiber.interrupt(active.fiber);
        }
        activePrompts.clear();
        for (const [, session] of sessions) {
          yield* session.destroy();
        }
        sessions.clear();
        pendingClientResponses.clear();
      });

    return { receive, shutdown };
  });

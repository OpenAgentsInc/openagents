import { createHash } from "node:crypto";
import { resolve } from "node:path";

import { AgentStdioHandlerError, AgentStdioTransport } from "@openagentsinc/agent-stdio-transport";

import { capabilityGuardedReverseHandler } from "./capabilities.ts";
import { STABLE_CONFORMANCE_CASES } from "./cases.ts";
import { definePeerScenario, runPeerScenario } from "./harness.ts";
import { ConformanceProjectionState, projectSessionUpdateForConformance } from "./projection.ts";

export type ExecutedFaultResult = Readonly<{
  fault: string;
  layer: string;
  result: "pass" | "fail";
  oracle: string;
  boundedMs: number;
  detail: string;
}>;

const methodCase = (method: string) => {
  const value = STABLE_CONFORMANCE_CASES.find(
    (candidate) => candidate.direction === "client-to-agent" && candidate.method === method,
  );
  if (value === undefined) throw new Error(`missing conformance case ${method}`);
  return value;
};

const rejectionKind = async (promise: Promise<unknown>): Promise<string> => {
  try {
    await promise;
    return "resolved";
  } catch (error) {
    return error !== null && typeof error === "object" && "kind" in error
      ? String((error as { kind: unknown }).kind)
      : error instanceof Error
        ? error.name
        : "unknown";
  }
};

const initializeResult = {
  protocolVersion: 1,
  agentCapabilities: {},
  authMethods: [],
  agentInfo: { name: "fault-peer", version: "1" },
};

const execute = async (fault: string): Promise<Readonly<{ oracle: string; detail: string }>> => {
  if (fault === "malformed-frame" || fault === "oversized-frame") {
    const raw = fault === "malformed-frame" ? "not-json\n" : `${"x".repeat(300)}\n`;
    const kind = await rejectionKind(
      runPeerScenario(
        definePeerScenario({ name: fault, actions: [{ method: "initialize", raw }] }),
        [{ method: "initialize", params: { protocolVersion: 1 } }],
        {},
        fault === "oversized-frame" ? { limits: { maxLineBytes: 256, maxBufferedBytes: 512 } } : {},
      ),
    );
    if (kind !== "protocol_violation") throw new Error(`unexpected ${kind}`);
    return { oracle: "production-transport-parser", detail: kind };
  }
  if (fault === "unknown-method") {
    const result = await runPeerScenario(
      definePeerScenario({
        name: fault,
        actions: [
          {
            method: "initialize",
            result: initializeResult,
            reverseRequests: [{ method: "future/unknown", params: {} }],
            ignoreReverseErrors: true,
          },
        ],
      }),
      [{ method: "initialize", params: { protocolVersion: 1 } }],
    );
    if (!result.transcript.some((row) => JSON.stringify(row.native).includes('"code":-32601')))
      throw new Error("unknown reverse method was not refused");
    return { oracle: "production-unknown-reverse-refusal", detail: "method-not-found" };
  }
  if (fault === "invalid-params") {
    let handlerInvoked = false;
    const result = await runPeerScenario(
      definePeerScenario({
        name: fault,
        actions: [
          {
            method: "initialize",
            result: initializeResult,
            reverseRequests: [{ method: "fs/read_text_file", params: {} }],
            ignoreReverseErrors: true,
          },
        ],
      }),
      [{ method: "initialize", params: { protocolVersion: 1 } }],
      {
        "fs/read_text_file": () => {
          handlerInvoked = true;
          return { content: "must-not-run" };
        },
      },
    );
    if (handlerInvoked) throw new Error("invalid params reached authority handler");
    if (!result.transcript.some((row) => JSON.stringify(row.native).includes('"code":-32602')))
      throw new Error("invalid params did not receive structured refusal");
    return { oracle: "pinned-envelope-codec", detail: "invalid-params-refused" };
  }
  if (fault === "duplicate-response" || fault === "late-response") {
    const result = await runPeerScenario(
      definePeerScenario({
        name: fault,
        actions: [
          {
            method: "initialize",
            result: initializeResult,
            ...(fault === "duplicate-response"
              ? { duplicateResponse: true }
              : { lateDuplicateMs: 10 }),
          },
        ],
      }),
      [{ method: "initialize", params: { protocolVersion: 1 } }],
      {},
      { settleMs: 30 },
    );
    if (result.receipt.counters.unknownOrLateResponses !== 1)
      throw new Error("late response was not quarantined");
    return { oracle: "production-response-correlation", detail: "quarantined=1" };
  }
  if (["reverse-timeout", "reverse-refusal", "capability-lie"].includes(fault)) {
    let invoked = false;
    const method = "fs/read_text_file";
    const handler = (() => {
      if (fault === "reverse-timeout") return async () => new Promise(() => undefined);
      if (fault === "reverse-refusal")
        return () => {
          throw new AgentStdioHandlerError(-32_003, "refused by policy");
        };
      return capabilityGuardedReverseHandler(
        method,
        { fs: { readTextFile: false, writeTextFile: false }, terminal: false },
        () => {
          invoked = true;
          return { content: "must-not-run" };
        },
      );
    })();
    const result = await runPeerScenario(
      definePeerScenario({
        name: fault,
        actions: [
          {
            method: "initialize",
            result: initializeResult,
            reverseRequests: [
              {
                method,
                params: { sessionId: "fixture-session-1", path: "/workspace/a.txt" },
              },
            ],
            ignoreReverseErrors: true,
          },
        ],
      }),
      [{ method: "initialize", params: { protocolVersion: 1 } }],
      { [method]: handler },
      { limits: { reverseRequestTimeoutMs: 20 } },
    );
    if (fault === "reverse-timeout" && result.receipt.counters.reverseTimeouts !== 1)
      throw new Error("reverse timeout not observed");
    if (fault === "capability-lie" && invoked) throw new Error("unadvertised broker ran");
    return {
      oracle: "negotiated-reverse-authority",
      detail:
        fault === "reverse-timeout"
          ? "bounded-timeout"
          : fault === "capability-lie"
            ? "handler-not-invoked"
            : "structured-refusal",
    };
  }
  if (fault === "auth-omission") {
    const result = await runPeerScenario(
      definePeerScenario({
        name: fault,
        actions: [{ method: "initialize", result: initializeResult }],
      }),
      [{ method: "initialize", params: { protocolVersion: 1 } }],
    );
    const response = result.results[0] as { authMethods?: ReadonlyArray<unknown> };
    if ((response.authMethods ?? []).length !== 0) throw new Error("auth unexpectedly advertised");
    return { oracle: "initialize-auth-negotiation", detail: "no-auth-method" };
  }
  if (fault === "auth-failure") {
    const kind = await rejectionKind(
      runPeerScenario(
        definePeerScenario({
          name: fault,
          actions: [{ method: "authenticate", error: { code: -32_001, message: "auth failed" } }],
        }),
        [{ method: "authenticate", params: { methodId: "cached_token" } }],
      ),
    );
    if (kind !== "remote_error") throw new Error(`unexpected ${kind}`);
    return { oracle: "production-remote-error", detail: kind };
  }
  if (fault === "partial-output") {
    const event = projectSessionUpdateForConformance({
      sessionUpdate: "agent_message_chunk",
      content: { type: "text", text: "partial" },
    });
    if (event.kind !== "message-delta") throw new Error("partial output was lost");
    return { oracle: "conformance-projection", detail: event.kind };
  }
  if (fault === "update-after-completion") {
    const state = new ConformanceProjectionState();
    state.apply({
      generation: 1,
      sessionId: "s",
      updateId: "1",
      sequence: 1,
      update: {
        sessionUpdate: "tool_call_update",
        toolCallId: "t",
        status: "completed",
      },
    });
    const disposition = state.apply({
      generation: 1,
      sessionId: "s",
      updateId: "2",
      sequence: 2,
      update: { sessionUpdate: "tool_call_update", toolCallId: "t", status: "in_progress" },
    });
    if (disposition.reason !== "tool-state-regression") throw new Error("terminal state regressed");
    return { oracle: "projection-state-machine", detail: disposition.reason };
  }
  if (fault.startsWith("exit-")) {
    const methodByPhase: Readonly<Record<string, string>> = {
      "exit-startup": "initialize",
      "exit-initialize": "initialize",
      "exit-authenticate": "authenticate",
      "exit-session": "session/new",
      "exit-prompt": "session/prompt",
      "exit-drain": "session/close",
    };
    const method = methodByPhase[fault];
    if (method === undefined) throw new Error("unknown exit phase");
    const value = methodCase(method);
    const kind = await rejectionKind(
      runPeerScenario(
        definePeerScenario(
          fault === "exit-startup"
            ? { name: fault, actions: [], exitOnStart: 17 }
            : {
                name: fault,
                actions: [{ method, exitBeforeResponse: true, exitCode: 17 }],
              },
        ),
        [{ method, params: value.params }],
      ),
    );
    if (kind !== "process_exit") throw new Error(`unexpected ${kind}`);
    return { oracle: "production-process-lifecycle", detail: `${fault}:${kind}` };
  }
  if (fault === "slow-consumer") {
    const transport = await AgentStdioTransport.start({
      executable: process.execPath,
      args: [resolve(import.meta.dirname, "../scripts/scripted-peer.mjs")],
      env: { OA_ACP_SCENARIO: JSON.stringify({ name: fault, actions: [], pauseInput: true }) },
      limits: {
        maxLineBytes: 300_000,
        maxOutboundQueue: 2,
        maxInFlightRequests: 10,
        requestTimeoutMs: 1_000,
        shutdownGraceMs: 10,
        terminateGraceMs: 10,
      },
    });
    const pending = Array.from({ length: 3 }, (_, index) =>
      transport
        .request("session/prompt", {
          sessionId: `s-${index}`,
          prompt: [{ type: "text", text: "x".repeat(200_000) }],
        })
        .catch(() => undefined),
    );
    const kind = await rejectionKind(
      transport.request("session/prompt", {
        sessionId: "s-overload",
        prompt: [{ type: "text", text: "x".repeat(200_000) }],
      }),
    );
    const receipt = transport.getReceipt();
    await transport.dispose();
    await Promise.all(pending);
    if (kind !== "overload") throw new Error(`unexpected ${kind}`);
    if (receipt.counters.peakOutboundQueue !== 2 || receipt.counters.overloads !== 1)
      throw new Error("slow-consumer bounds were not exact");
    return {
      oracle: "production-stdout-backpressure-bound",
      detail: "maxOutboundQueue=2,peak=2,overloads=1",
    };
  }
  if (fault === "queue-overload") {
    const kind = await rejectionKind(
      runPeerScenario(
        definePeerScenario({
          name: fault,
          actions: [{ method: "session/prompt", result: { stopReason: "end_turn" }, delayMs: 25 }],
        }),
        ["a", "b"].map((sessionId) => ({
          method: "session/prompt",
          params: { sessionId, prompt: [{ type: "text", text: "fixture" }] },
        })),
        {},
        { limits: { maxInFlightRequests: 1 } },
      ),
    );
    if (kind !== "overload") throw new Error(`unexpected ${kind}`);
    return { oracle: "production-in-flight-bound", detail: kind };
  }
  if (fault === "cancellation-race") {
    const controller = new AbortController();
    setImmediate(() => controller.abort());
    const kind = await rejectionKind(
      runPeerScenario(
        definePeerScenario({
          name: fault,
          actions: [{ method: "session/prompt", result: { stopReason: "end_turn" }, delayMs: 50 }],
        }),
        [
          {
            method: "session/prompt",
            params: {
              sessionId: "s",
              prompt: [{ type: "text", text: "fixture" }],
            },
            signal: controller.signal,
          },
        ],
      ),
    );
    if (kind !== "cancelled") throw new Error(`unexpected ${kind}`);
    return { oracle: "production-abort-race", detail: kind };
  }
  if (fault === "replay-live-interleaving") {
    const state = new ConformanceProjectionState();
    const base = {
      generation: 1,
      sessionId: "s",
      update: {
        sessionUpdate: "agent_message_chunk",
        content: { type: "text", text: "fixture" },
      },
    } as const;
    state.apply({ ...base, updateId: "replay-1", sequence: 1 });
    state.apply({ ...base, updateId: "live-3", sequence: 3 });
    const disposition = state.apply({ ...base, updateId: "replay-2", sequence: 2 });
    if (disposition.reason !== "out-of-order") throw new Error("replay crossed live frontier");
    return { oracle: "projection-order-gate", detail: disposition.reason };
  }
  if (fault === "restart-generation-crossing") {
    const state = new ConformanceProjectionState();
    const update = {
      sessionUpdate: "agent_message_chunk",
      content: { type: "text", text: "fixture" },
    };
    state.apply({ generation: 2, sessionId: "s", updateId: "new", sequence: 1, update });
    const disposition = state.apply({
      generation: 1,
      sessionId: "s",
      updateId: "old",
      sequence: 2,
      update,
    });
    if (disposition.reason !== "old-generation") throw new Error("old generation crossed restart");
    return { oracle: "projection-generation-gate", detail: disposition.reason };
  }
  throw new Error(`fault has no executable oracle: ${fault}`);
};

export const executeFaultCase = async (
  layer: string,
  fault: string,
): Promise<ExecutedFaultResult> => {
  const started = Date.now();
  try {
    const outcome = await execute(fault);
    return {
      layer,
      fault,
      result: "pass",
      oracle: outcome.oracle,
      boundedMs: Date.now() - started,
      detail: outcome.detail,
    };
  } catch (error) {
    return {
      layer,
      fault,
      result: "fail",
      oracle: "execution-failed",
      boundedMs: Date.now() - started,
      detail: createHash("sha256").update(String(error)).digest("hex"),
    };
  }
};

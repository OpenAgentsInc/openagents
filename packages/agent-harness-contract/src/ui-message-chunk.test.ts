import type {
  AgentRuntimeRedactionClass,
  AgentRuntimeVisibility,
  KhalaRuntimeSource,
} from "@openagentsinc/agent-runtime-schema";
import { decodeKhalaRuntimeEvent } from "@openagentsinc/agent-runtime-schema";
import { describe, expect, test } from "vite-plus/test";

import { buildTextDelta, buildTurnFinished, buildTurnStarted } from "./event-builder.ts";
import { KhalaRuntimeEventSchemaLiteral, type HarnessStreamEvent } from "./stream.ts";
import {
  decodeUiMessageChunk,
  encodeUiMessageChunk,
  khalaEventToUiChunks,
} from "./ui-message-chunk.ts";

const SOURCE: KhalaRuntimeSource = { lane: "test_fixture" };

const AUTHORITY = {
  authorityRef: "auth.1",
  policyRef: "policy.default",
  decisionRef: "decision.1",
  toolRef: "Bash",
  status: "allowed",
  allowed: true,
  blockerRefs: [],
} as const;

/** Local builder for the event kinds `event-builder.ts` does not cover. */
const buildEvent = (
  sequence: number,
  fields: Record<string, unknown>,
  overrides?: {
    readonly visibility?: AgentRuntimeVisibility;
    readonly redactionClass?: AgentRuntimeRedactionClass;
  },
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    schema: KhalaRuntimeEventSchemaLiteral,
    eventId: `evt.t1.${sequence}`,
    turnId: "t1",
    threadId: "s1",
    sequence,
    observedAt: "2026-07-21T00:00:00.000Z",
    source: SOURCE,
    visibility: overrides?.visibility ?? "private",
    redactionClass: overrides?.redactionClass ?? "private_ref",
    causalityRefs: [],
    ...fields,
  });

/** The scripted builder turn: turn.started, two text deltas, turn.finished. */
const scriptedTurn = (): ReadonlyArray<HarnessStreamEvent> => [
  buildTurnStarted({ turnId: "t1", threadId: "s1", sequence: 0, source: SOURCE }),
  buildTextDelta({
    turnId: "t1",
    threadId: "s1",
    sequence: 1,
    source: SOURCE,
    messageId: "msg.t1",
    text: "Hello ",
  }),
  buildTextDelta({
    turnId: "t1",
    threadId: "s1",
    sequence: 2,
    source: SOURCE,
    messageId: "msg.t1",
    text: "world",
  }),
  buildTurnFinished({
    turnId: "t1",
    threadId: "s1",
    sequence: 3,
    source: SOURCE,
    finishReason: "stop",
  }),
];

const toolEvents = (): ReadonlyArray<HarnessStreamEvent> => [
  buildEvent(10, {
    kind: "tool.input.delta",
    toolCallId: "call.1",
    toolName: "Bash",
    chunkId: "chunk.t1.10",
    inputDelta: '{"command":',
    authority: AUTHORITY,
  }),
  buildEvent(11, {
    kind: "tool.call",
    toolCallId: "call.1",
    toolName: "Bash",
    inputRef: "input.call.1",
    authority: AUTHORITY,
  }),
  buildEvent(12, {
    kind: "tool.result",
    toolCallId: "call.1",
    toolName: "Bash",
    resultRef: "result.call.1",
    authority: AUTHORITY,
    providerExecuted: true,
  }),
];

describe("khalaEventToUiChunks — the layer-1 projection", () => {
  test("a scripted turn projects to start, deltas, and finish with the source cursor on every chunk", () => {
    const chunks = scriptedTurn().flatMap((event) => khalaEventToUiChunks(event));

    expect(chunks.map((chunk) => chunk.type)).toEqual([
      "message-start",
      "text-delta",
      "text-delta",
      "message-finish",
    ]);
    expect(chunks.map((chunk) => chunk.cursor)).toEqual([0, 1, 2, 3]);

    const [start, deltaA, deltaB, finish] = chunks;
    expect(start).toMatchObject({ type: "message-start", messageId: "t1" });
    expect(deltaA).toMatchObject({ type: "text-delta", id: "msg.t1", delta: "Hello " });
    expect(deltaB).toMatchObject({ type: "text-delta", id: "msg.t1", delta: "world" });
    expect(finish).toMatchObject({ type: "message-finish", finishReason: "stop" });
  });

  test("tool events project to the tool state-machine chunks with common-name normalization", () => {
    const chunks = toolEvents().flatMap((event) => khalaEventToUiChunks(event));

    expect(chunks.map((chunk) => chunk.type)).toEqual([
      "tool-input-streaming",
      "tool-input-available",
      "tool-output-available",
    ]);
    expect(chunks[0]).toMatchObject({
      toolCallId: "call.1",
      inputTextDelta: '{"command":',
      tool: { wireName: "bash", nativeName: "Bash", commonName: "bash" },
    });
    expect(chunks[1]).toMatchObject({ toolCallId: "call.1", inputRef: "input.call.1" });
    expect(chunks[2]).toMatchObject({
      toolCallId: "call.1",
      resultRef: "result.call.1",
      tool: { wireName: "bash", nativeName: "Bash", providerExecuted: true },
    });
  });

  test("tool.error projects to tool-output-error carrying only the safe text and refs", () => {
    const chunks = khalaEventToUiChunks(
      buildEvent(13, {
        kind: "tool.error",
        toolCallId: "call.1",
        toolName: "Bash",
        errorRef: "error.call.1",
        messageSafe: "command failed",
        authority: AUTHORITY,
      }),
    );
    expect(chunks).toHaveLength(1);
    expect(chunks[0]).toMatchObject({
      type: "tool-output-error",
      toolCallId: "call.1",
      errorText: "command failed",
      errorRef: "error.call.1",
    });
  });

  test("desktop-display-only kinds with no chunk equivalent project to []", () => {
    const displayOnly: ReadonlyArray<HarnessStreamEvent> = [
      buildEvent(20, {
        kind: "tool.input.completed",
        toolCallId: "call.1",
        toolName: "Bash",
        authority: AUTHORITY,
      }),
      buildEvent(21, {
        kind: "agent.child.started",
        childAgentId: "child.1",
        childRunId: "run.1",
        parentAgentId: "parent.1",
      }),
      buildEvent(22, { kind: "usage.recorded", usage: { usageRef: "usage.1" } }),
      buildEvent(23, { kind: "provider.metadata", providerMetadata: { metadataRefs: [] } }),
      buildEvent(24, {
        kind: "file.change",
        fileChange: { fileChangeRef: "fc.1", pathRef: "path.1", op: "modified" },
      }),
      buildEvent(25, {
        kind: "compaction.recorded",
        beforeContextRef: "ctx.before",
        afterContextRef: "ctx.after",
      }),
      buildEvent(26, {
        kind: "raw.sidecar_ref",
        rawEventRef: "raw.1",
        rawEventKind: "claude_sdk_event",
      }),
    ];
    for (const event of displayOnly) {
      expect(khalaEventToUiChunks(event)).toEqual([]);
    }
  });

  test("send flags gate reasoning, start, and finish chunks", () => {
    const reasoningDelta = buildEvent(30, {
      kind: "reasoning.delta",
      messageId: "msg.t1",
      chunkId: "chunk.t1.30",
      text: "thinking",
    });
    const reasoningEnd = buildEvent(31, { kind: "reasoning.completed", messageId: "msg.t1" });
    const [started, , , finished] = scriptedTurn();

    expect(khalaEventToUiChunks(reasoningDelta)[0]).toMatchObject({
      type: "reasoning-delta",
      id: "msg.t1",
      delta: "thinking",
    });
    expect(khalaEventToUiChunks(reasoningEnd)[0]).toMatchObject({ type: "reasoning-end" });
    expect(khalaEventToUiChunks(reasoningDelta, { sendReasoning: false })).toEqual([]);
    expect(khalaEventToUiChunks(reasoningEnd, { sendReasoning: false })).toEqual([]);
    expect(khalaEventToUiChunks(started!, { sendStart: false })).toEqual([]);
    expect(khalaEventToUiChunks(finished!, { sendFinish: false })).toEqual([]);
  });

  test("turn.interrupted projects to message-abort", () => {
    const chunks = khalaEventToUiChunks(
      buildEvent(40, { kind: "turn.interrupted", reasonRef: "reason.user_stop" }),
    );
    expect(chunks[0]).toMatchObject({ type: "message-abort", reasonRef: "reason.user_stop" });
  });

  test("visibility gating: unadmitted events drop, transient visibilities mark chunks transient", () => {
    const privateDelta = buildTextDelta({
      turnId: "t1",
      threadId: "s1",
      sequence: 1,
      source: SOURCE,
      messageId: "msg.t1",
      text: "secret",
    });
    // The builder default visibility is "private": a public-only projection drops it.
    expect(khalaEventToUiChunks(privateDelta, { admitVisibilities: ["public"] })).toEqual([]);

    const operatorDelta = buildEvent(
      50,
      { kind: "text.delta", messageId: "msg.t1", chunkId: "chunk.t1.50", text: "operator note" },
      { visibility: "operator", redactionClass: "operator_summary" },
    );
    expect(khalaEventToUiChunks(operatorDelta)[0]).toMatchObject({
      type: "text-delta",
      transient: true,
    });

    const publicDelta = buildEvent(
      51,
      { kind: "text.delta", messageId: "msg.t1", chunkId: "chunk.t1.51", text: "public" },
      { visibility: "public", redactionClass: "public_ref" },
    );
    const [publicChunk] = khalaEventToUiChunks(publicDelta);
    expect(publicChunk).toMatchObject({ type: "text-delta" });
    expect(publicChunk).not.toHaveProperty("transient");
  });

  test("every projected chunk is Schema-encodable and round-trips exactly", () => {
    const chunks = [
      ...scriptedTurn(),
      ...toolEvents(),
      buildEvent(60, {
        kind: "step.started",
        stepId: "step.1",
      }),
      buildEvent(61, {
        kind: "step.finished",
        stepId: "step.1",
        finishReason: "tool-calls",
      }),
      buildEvent(62, { kind: "text.completed", messageId: "msg.t1" }),
    ].flatMap((event) => khalaEventToUiChunks(event));

    expect(chunks.length).toBeGreaterThanOrEqual(10);
    for (const chunk of chunks) {
      expect(decodeUiMessageChunk(encodeUiMessageChunk(chunk))).toEqual(chunk);
    }
  });
});

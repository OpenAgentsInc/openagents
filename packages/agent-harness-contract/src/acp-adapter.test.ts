import { Effect, Schema as S, Stream } from "effect";
import {
  type AcpAdapterContext,
  type AcpAdapterEvent,
  acpEventToKhalaEvents,
  acpPermissionToRuntimeInteractionPayload,
  makeAcpHarnessAdapter,
} from "./acp-adapter.ts";
import {
  RuntimeInteractionPayload,
  type KhalaRuntimeSource,
} from "@openagentsinc/agent-runtime-schema";
import { describe, expect, test } from "vite-plus/test";

import type { HarnessStreamEvent } from "./stream.ts";

const SOURCE: KhalaRuntimeSource = {
  lane: "agent_client_protocol",
  adapterKind: "agent_client_protocol",
};

const collect = (
  stream: Stream.Stream<HarnessStreamEvent, unknown>,
): Effect.Effect<ReadonlyArray<HarnessStreamEvent>, unknown> => Stream.runCollect(stream);

const sequences = (events: ReadonlyArray<HarnessStreamEvent>) => events.map((e) => e.sequence);

/** A contiguous counter that drives the pure projection deterministically. */
const makeContext = (): AcpAdapterContext => {
  let counter = 0;
  return {
    turnId: "turn.acp.1",
    threadId: "thread.acp.1",
    source: SOURCE,
    nextSequence: () => counter++,
  };
};

const decodeRuntimeInteractionPayload = S.decodeUnknownSync(RuntimeInteractionPayload);

describe("acp projection — event vocabulary maps to a contiguous KhalaRuntimeEvent stream", () => {
  test("a representative ACP sequence projects turn.started..turn.finished with contiguous sequences", () => {
    const ctx = makeContext();
    const script: ReadonlyArray<AcpAdapterEvent> = [
      { type: "acp_turn_started" },
      { type: "acp_thought_delta", text: "thinking" },
      { type: "acp_text_delta", text: "hello" },
      { type: "acp_tool_call", toolCallId: "toolcall.acp.1", toolName: "shell" },
      // A permission request emits NO stream event — it routes to RuntimeInteraction.
      { type: "acp_permission_request", toolCallId: "toolcall.acp.1", toolName: "shell" },
      { type: "acp_tool_result", toolCallId: "toolcall.acp.1", toolName: "shell", ok: true },
      { type: "acp_turn_stop", stopReason: "end_turn" },
    ];

    const events = script.flatMap((event) => acpEventToKhalaEvents(event, ctx));

    expect(events.map((e) => e.kind)).toEqual([
      "turn.started",
      "reasoning.delta",
      "text.delta",
      "tool.call",
      "tool.result",
      "turn.finished",
    ]);
    // The permission request consumed no sequence — the stream stays contiguous.
    expect(sequences(events)).toEqual([0, 1, 2, 3, 4, 5]);
    expect(events[0]?.kind).toBe("turn.started");
    expect(events.at(-1)?.kind).toBe("turn.finished");
  });

  test("a failed tool result projects tool.error with the safe message", () => {
    const ctx = makeContext();
    const events = acpEventToKhalaEvents(
      {
        type: "acp_tool_result",
        toolCallId: "toolcall.acp.9",
        toolName: "apply_patch",
        ok: false,
        messageSafe: "patch did not apply",
      },
      ctx,
    );
    const [event] = events;
    expect(event?.kind).toBe("tool.error");
    if (event?.kind === "tool.error") {
      expect(event.messageSafe).toBe("patch did not apply");
    }
  });

  test("a refusal stop projects a content-filter finish reason", () => {
    const ctx = makeContext();
    const [event] = acpEventToKhalaEvents({ type: "acp_turn_stop", stopReason: "refusal" }, ctx);
    expect(event?.kind).toBe("turn.finished");
    if (event?.kind === "turn.finished") {
      expect(event.finishReason).toBe("content-filter");
    }
  });
});

describe("acp projection — tool-name normalization via toolIdentity", () => {
  test("a native ACP tool name is normalized onto the common vocabulary", () => {
    const ctx = makeContext();
    const [call] = acpEventToKhalaEvents(
      { type: "acp_tool_call", toolCallId: "toolcall.acp.1", toolName: "shell" },
      ctx,
    );
    expect(call?.kind).toBe("tool.call");
    if (call?.kind === "tool.call") {
      // ACP `shell` normalizes to the common `bash`.
      expect(call.toolName).toBe("bash");
    }
  });

  test("an ACP tool with no common equivalent keeps its native name", () => {
    const ctx = makeContext();
    const [call] = acpEventToKhalaEvents(
      { type: "acp_tool_call", toolCallId: "toolcall.acp.2", toolName: "cursor_run" },
      ctx,
    );
    if (call?.kind === "tool.call") {
      expect(call.toolName).toBe("cursor_run");
    }
  });
});

describe("acp permission — approvals route through the RuntimeInteraction model", () => {
  test("a permission request produces a valid tool_approval RuntimeInteractionPayload", () => {
    const payload = acpPermissionToRuntimeInteractionPayload({
      type: "acp_permission_request",
      toolCallId: "toolcall.acp.1",
      toolName: "shell",
    });
    // Shape-check by decoding through the canonical schema.
    const decoded = decodeRuntimeInteractionPayload(payload);
    expect(decoded.kind).toBe("tool_approval");
    if (decoded.kind === "tool_approval") {
      expect(decoded.toolName).toBe("bash");
      expect(decoded.toolCallId).toBe("toolcall.acp.1");
      expect(decoded.authority.status).toBe("operator_escalation_required");
      expect(decoded.authority.allowed).toBe(false);
    }
  });

  test("an inactive built-in emulation request carries the inactive-builtin blocker", () => {
    const payload = acpPermissionToRuntimeInteractionPayload({
      type: "acp_permission_request",
      toolCallId: "toolcall.acp.7",
      toolName: "Bash",
      inactiveBuiltin: true,
    });
    const decoded = decodeRuntimeInteractionPayload(payload);
    if (decoded.kind === "tool_approval") {
      expect(decoded.authority.blockerRefs).toContain("blocker.inactive_builtin_tool");
    }
  });
});

describe("acp adapter factory — one factory admits any ACP peer", () => {
  const grok = makeAcpHarnessAdapter({ harnessId: "grok", harnessKind: "grok_cli" });
  // `cursor_cli` is not a member of AgentDefinitionHarnessKind; a Cursor peer
  // declares harnessKind `custom` while its adapterKind carries the ACP identity.
  const cursor = makeAcpHarnessAdapter({
    harnessId: "cursor",
    harnessKind: "custom",
    adapterKind: "cursor_cli",
  });

  test("both peers are ACP adapters that route approvals through RuntimeInteraction", () => {
    expect(grok.harnessId).toBe("grok");
    expect(grok.harnessKind).toBe("grok_cli");
    expect(grok.adapterKind).toBe("agent_client_protocol");
    expect(grok.supportsBuiltinToolApprovals).toBe(false);

    expect(cursor.harnessId).toBe("cursor");
    expect(cursor.adapterKind).toBe("cursor_cli");
    expect(cursor.specificationVersion).toBe("agent-harness-v1");
  });

  test("a full turn streams turn.started -> ... -> turn.finished with contiguous sequences", async () => {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const session = yield* grok.start({ sessionId: "s1", source: SOURCE });
        const control = yield* session.promptTurn({ turnId: "t1", prompt: "hi" });
        const events = yield* collect(control.events);
        const done = yield* control.done;
        return { events, done };
      }),
    );

    expect(result.events[0]?.kind).toBe("turn.started");
    expect(result.events.at(-1)?.kind).toBe("turn.finished");
    // The default script's permission-free projection is contiguous from 0.
    expect(sequences(result.events)).toEqual(result.events.map((_, index) => index));
    expect(result.done.finishReason).toBe("stop");
    expect(result.done.lastCursor).toBe(result.events.length - 1);
  });

  test("suspend then continue replays from cursor+1 with no gap and no duplicate", async () => {
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        // Phase 1: pull only the first two events, then suspend.
        const session = yield* cursor.start({ sessionId: "s1", source: SOURCE });
        const control = yield* session.promptTurn({ turnId: "t1", prompt: "hi" });
        const phase1 = yield* collect(control.events.pipe(Stream.take(2)));
        const continuation = yield* session.suspendTurn();

        // Phase 2: a FRESH session (different process) resumes from the cursor.
        const session2 = yield* cursor.start({
          sessionId: "s1",
          source: SOURCE,
          continueFrom: continuation,
        });
        const control2 = yield* session2.continueTurn({});
        const phase2 = yield* collect(control2.events);

        return { phase1, continuation, phase2 };
      }),
    );

    expect(sequences(outcome.phase1)).toEqual([0, 1]);
    expect(outcome.continuation.cursor).toBe(1);
    expect(outcome.continuation.lossy).toBe(false);
    // Phase 2 attaches at cursor + 1 — no gap, no duplicate.
    expect(outcome.phase2[0]?.sequence).toBe(outcome.continuation.cursor + 1);

    const merged = sequences([...outcome.phase1, ...outcome.phase2]);
    expect(merged[0]).toBe(0);
    expect(new Set(merged).size).toBe(merged.length);
    // Contiguous with no gap across the slice boundary.
    for (let i = 1; i < merged.length; i += 1) {
      expect(merged[i]).toBe((merged[i - 1] ?? -1) + 1);
    }
  });
});

import { describe, expect, it } from "vite-plus/test";
import fc from "fast-check";
import { decodeKhalaRuntimeEvent } from "@openagentsinc/agent-runtime-schema";
import { decodeStableAcpMethodPayload } from "@openagentsinc/agent-client-protocol/stable";
import {
  createAcpRuntimeNativeEnvelope,
  createBoundedAcpNativeEvidenceStore,
  redactAcpEvidence,
  type AcpRuntimeNativeEnvelope,
} from "./native-envelope.ts";
import { AcpRuntimeProjector, bindAcpSession } from "./projection.ts";

const now = "2026-07-16T12:00:00.000Z";
const stored: AcpRuntimeNativeEnvelope[] = [];
const binding = () =>
  bindAcpSession({
    profile: "grok",
    processGeneration: 7,
    connectionRef: "connection.1",
    peerSessionId: "provider-session",
    canonicalThreadSeed: "user-thread",
  });
const envelope = (
  updateId: string,
  payload: unknown,
  discriminant = typeof (payload as { sessionUpdate?: unknown })?.sessionUpdate === "string"
    ? String((payload as { sessionUpdate: string }).sessionUpdate)
    : "future/update",
): AcpRuntimeNativeEnvelope => {
  const value = createAcpRuntimeNativeEnvelope({
    profile: "grok",
    protocolVersion: 1,
    connectionRef: "connection.1",
    processGeneration: 7,
    method: "session/update",
    updateId,
    sessionId: "provider-session",
    observedAt: now,
    discriminant,
    validatedPayload: payload,
  });
  if ("kind" in value) throw new Error(value.safeDetail);
  return value;
};
const projector = () =>
  new AcpRuntimeProjector({
    binding: binding(),
    turnSeed: "turn-1",
    store: {
      async put(value) {
        stored.push(value);
        return { rawEventRef: `native.${value.updateId}` };
      },
    },
  });

describe("ACP native envelope", () => {
  it("is bounded, lossless, private, and extension namespaced", () => {
    const value = createAcpRuntimeNativeEnvelope({
      profile: "cursor",
      protocolVersion: 1,
      connectionRef: "c",
      processGeneration: 1,
      method: "cursor/ask_question",
      requestId: 4,
      updateId: "4",
      sessionId: "s",
      observedAt: now,
      discriminant: "cursor/ask_question",
      validatedPayload: { value: "kept" },
      nativeMeta: { totalTokens: 7 },
      maxBytes: 100,
    });
    expect("kind" in value).toBe(false);
    if (!("kind" in value)) {
      expect(value.extensionNamespace).toBe("cursor");
      expect(value.validatedPayload).toEqual({ value: "kept" });
      expect(value.nativeMeta).toEqual({ totalTokens: 7 });
      expect(value.retention).toBe("private-native");
      expect(() => ((value.validatedPayload as { value: string }).value = "mutated")).toThrow();
      expect(() => ((value.nativeMeta as { totalTokens: number }).totalTokens = 9)).toThrow();
    }
    expect(
      createAcpRuntimeNativeEnvelope({
        profile: "grok",
        protocolVersion: 1,
        connectionRef: "c",
        processGeneration: 1,
        method: "m",
        updateId: "u",
        observedAt: now,
        discriminant: "x",
        validatedPayload: { huge: "x".repeat(200) },
        maxBytes: 20,
      }),
    ).toMatchObject({ kind: "native-envelope-rejected", reason: "payload-too-large" });
  });

  it("bounds aggregate evidence without eviction and redacts adversarial secret values", async () => {
    const store = createBoundedAcpNativeEvidenceStore({ maxEntries: 1, maxBytes: 1_000 });
    await store.put(envelope("bounded-1", { sessionUpdate: "plan", entries: [] }));
    await expect(
      store.put(envelope("bounded-2", { sessionUpdate: "plan", entries: [] })),
    ).rejects.toThrow("overloaded");
    expect(store.size()).toBe(1);
    expect(
      redactAcpEvidence({ message: "sk-secretvalue123456", authorization: "Bearer abc" }),
    ).toEqual({ message: "[redacted]", authorization: "[redacted]" });
  });
});

describe("ACP canonical projection", () => {
  it("projects every stable update family and retains native evidence first", async () => {
    stored.length = 0;
    const p = projector();
    const updates = [
      { sessionUpdate: "user_message_chunk", content: { type: "text", text: "hello" } },
      { sessionUpdate: "agent_message_chunk", content: { type: "text", text: "answer" } },
      { sessionUpdate: "agent_thought_chunk", content: { type: "text", text: "thinking" } },
      {
        sessionUpdate: "tool_call",
        toolCallId: "native-tool",
        title: "Read",
        status: "pending",
        rawInput: { secret: true },
      },
      { sessionUpdate: "tool_call_update", toolCallId: "native-tool", status: "in_progress" },
      {
        sessionUpdate: "tool_call_update",
        toolCallId: "native-tool",
        status: "completed",
        rawOutput: "private",
      },
      { sessionUpdate: "plan", entries: [{ content: "one", status: "pending", priority: "high" }] },
      {
        sessionUpdate: "available_commands_update",
        availableCommands: [{ name: "review", description: "Review changes" }],
      },
      { sessionUpdate: "current_mode_update", currentModeId: "agent" },
      { sessionUpdate: "config_option_update", configOptions: [] },
      { sessionUpdate: "session_info_update", title: "title", updatedAt: now },
      { sessionUpdate: "usage_update", used: 10, size: 100, cost: { amount: 1, currency: "USD" } },
    ];
    for (let index = 0; index < updates.length; index += 1) {
      expect(
        decodeStableAcpMethodPayload({
          direction: "agent-to-client",
          method: "session/update",
          phase: "params",
          payload: { sessionId: "provider-session", update: updates[index] },
        })._tag,
      ).toBe("Decoded");
      const result = await p.apply({
        envelope: envelope(String(index), updates[index]),
        sequence: index,
      });
      expect(result.outcome).toBe("applied");
      expect(result.events[0]).toMatchObject({
        kind: "raw.sidecar_ref",
        rawEventRef: `native.${index}`,
      });
      decodeKhalaRuntimeEvent(result.events[0]);
    }
    expect(stored).toHaveLength(updates.length);
    const completed = p.complete("end_turn", now);
    expect(completed.map((event) => event.kind)).toEqual([
      "text.completed",
      "reasoning.completed",
      "turn.finished",
    ]);
    completed.forEach((event) => decodeKhalaRuntimeEvent(event));
  });

  it("retains attachments and unknown variants as safe degraded state", async () => {
    const p = projector();
    const attachment = await p.apply({
      envelope: envelope("a", {
        sessionUpdate: "agent_message_chunk",
        content: { type: "image", data: "secret-image" },
      }),
      sequence: 1,
    });
    const unknown = await p.apply({
      envelope: envelope("b", { sessionUpdate: "future_update", token: "secret" }),
      sequence: 2,
    });
    expect(attachment.events[1]).toMatchObject({
      kind: "degraded",
      safeSummary: "unsupported agent_message_chunk content",
    });
    expect(unknown.events[1]).toMatchObject({ kind: "degraded" });
    expect(JSON.stringify(unknown.events)).not.toContain("secret");
  });

  it("never projects decode-failed known updates or prompt results", async () => {
    const p = projector();
    const invalidUpdate = createAcpRuntimeNativeEnvelope({
      profile: "grok",
      protocolVersion: 1,
      connectionRef: "connection.1",
      processGeneration: 7,
      method: "session/update",
      updateId: "invalid-known",
      sessionId: "provider-session",
      observedAt: now,
      discriminant: "usage_update",
      validatedPayload: { sessionUpdate: "usage_update", used: "invalid", size: 10 },
      validationStatus: "decode-failure",
    });
    if ("kind" in invalidUpdate) throw new Error("bad fixture");
    const projected = await p.apply({ envelope: invalidUpdate, sequence: 1 });
    expect(projected).toMatchObject({
      outcome: "quarantined",
      events: [{ kind: "raw.sidecar_ref" }, { kind: "degraded" }],
    });
    expect(projected.events.some((event) => event.kind === "usage-snapshot")).toBe(false);
    const invalidPrompt = createAcpRuntimeNativeEnvelope({
      profile: "grok",
      protocolVersion: 1,
      connectionRef: "connection.1",
      processGeneration: 7,
      method: "session/prompt",
      requestId: "invalid-prompt",
      updateId: "invalid-prompt-result",
      sessionId: "provider-session",
      observedAt: now,
      discriminant: "prompt_response/bad",
      validatedPayload: { stopReason: 42 },
      validationStatus: "decode-failure",
    });
    if ("kind" in invalidPrompt) throw new Error("bad fixture");
    const settled = await p.settle(invalidPrompt, "unknown");
    expect(settled).toMatchObject([{ kind: "raw.sidecar_ref" }, { kind: "degraded" }]);
    expect(settled.some((event) => event.kind === "turn.finished")).toBe(false);
  });

  it("covers every stable content, tool-content, tool-kind, status, and stop variant", async () => {
    const contents = [
      { type: "text", text: "fixture" },
      { type: "image", data: "AA==", mimeType: "image/png" },
      { type: "audio", data: "AA==", mimeType: "audio/wav" },
      { type: "resource_link", name: "fixture", uri: "file:///workspace/a.txt" },
      { type: "resource", resource: { uri: "file:///workspace/a.txt", text: "fixture" } },
    ];
    for (const [index, content] of contents.entries()) {
      const p = projector();
      const result = await p.apply({
        envelope: envelope(`content-${index}`, { sessionUpdate: "agent_message_chunk", content }),
        sequence: 1,
      });
      expect(result.outcome).toBe("applied");
      expect(result.events[0].kind).toBe("raw.sidecar_ref");
      expect(result.events[1].kind).toBe(content.type === "text" ? "text.delta" : "degraded");
    }
    const toolContents = [
      { type: "content", content: { type: "text", text: "fixture" } },
      { type: "diff", path: "/workspace/a.txt", newText: "new" },
      { type: "terminal", terminalId: "terminal-1" },
    ];
    const toolKinds = [
      "read",
      "edit",
      "delete",
      "move",
      "search",
      "execute",
      "think",
      "fetch",
      "switch_mode",
      "other",
    ];
    const statuses = ["pending", "in_progress", "completed", "failed"];
    for (const [index, kind] of toolKinds.entries()) {
      const p = projector();
      const result = await p.apply({
        envelope: envelope(`kind-${index}`, {
          sessionUpdate: "tool_call",
          toolCallId: `t-${index}`,
          title: kind,
          kind,
          status: "pending",
          content: toolContents,
        }),
        sequence: 1,
      });
      expect(result.events[1].kind).toBe("tool.call");
    }
    for (const [index, status] of statuses.entries()) {
      const p = projector();
      await p.apply({
        envelope: envelope(`start-${index}`, {
          sessionUpdate: "tool_call",
          toolCallId: "t",
          title: "Tool",
          status: status === "pending" ? "pending" : "in_progress",
        }),
        sequence: 1,
      });
      const result = await p.apply({
        envelope: envelope(`status-${index}`, {
          sessionUpdate: "tool_call_update",
          toolCallId: "t",
          status,
          content: toolContents,
        }),
        sequence: 2,
      });
      expect(result.outcome).toBe("applied");
    }
    const reasons = new Map<string, string>([
      ["end_turn", "stop"],
      ["max_tokens", "length"],
      ["max_turn_requests", "unknown"],
      ["refusal", "content-filter"],
      ["cancelled", "cancelled"],
    ]);
    for (const [reason, canonical] of reasons) {
      const event = projector().complete(reason, now).at(-1);
      expect(event).toMatchObject({ kind: "turn.finished", finishReason: canonical });
      decodeKhalaRuntimeEvent(event);
    }
  });

  it("projects structured refs-only plan, command, mode, config, session, and usage snapshots", async () => {
    const p = projector();
    const cases = [
      [
        {
          sessionUpdate: "plan",
          entries: [{ content: "private plan", status: "in_progress", priority: "high" }],
        },
        {
          kind: "plan-snapshot",
          snapshot: { entries: [{ status: "in_progress", priority: "high" }] },
        },
      ],
      [
        {
          sessionUpdate: "available_commands_update",
          availableCommands: [{ name: "review", description: "private description" }],
        },
        { kind: "available-commands" },
      ],
      [
        { sessionUpdate: "current_mode_update", currentModeId: "agent" },
        { kind: "mode-snapshot", snapshot: { cleared: false } },
      ],
      [
        {
          sessionUpdate: "config_option_update",
          configOptions: [{ id: "model", currentValue: "private-model" }],
        },
        { kind: "config-snapshot" },
      ],
      [
        { sessionUpdate: "session_info_update", title: null, updatedAt: now },
        { kind: "session-info", snapshot: { titleRef: null, updatedAt: now } },
      ],
      [
        { sessionUpdate: "usage_update", used: 4, size: 16, cost: { amount: 1 } },
        { kind: "usage-snapshot", snapshot: { used: 4, size: 16 } },
      ],
    ] as const;
    for (let index = 0; index < cases.length; index += 1) {
      const result = await p.apply({
        envelope: envelope(`snapshot-${index}`, cases[index]![0]),
        sequence: index,
      });
      expect(result.events[1]).toMatchObject(cases[index]![1]);
      expect(JSON.stringify(result.events[1])).not.toContain("private");
    }
  });

  it("does not use provider session IDs as canonical identity", () => {
    const first = binding();
    const second = bindAcpSession({
      profile: "grok",
      processGeneration: 8,
      connectionRef: "connection.1",
      peerSessionId: "provider-session",
      canonicalThreadSeed: "user-thread",
    });
    expect(first.threadId).not.toBe("provider-session");
    expect(first.threadId).not.toBe(second.threadId);
  });

  it("deduplicates replay and quarantines order, generation, terminal regression, and late updates", async () => {
    const p = projector();
    const first = envelope("one", {
      sessionUpdate: "tool_call",
      toolCallId: "t",
      title: "Run",
      status: "pending",
    });
    expect((await p.apply({ envelope: first, sequence: 1 })).outcome).toBe("applied");
    expect((await p.apply({ envelope: first, sequence: 1 })).outcome).toBe("duplicate");
    expect(
      (
        await p.apply({
          envelope: envelope("zero", { sessionUpdate: "plan", entries: [] }),
          sequence: 0,
        })
      ).reason,
    ).toBe("out-of-order");
    await p.apply({
      envelope: envelope("done", {
        sessionUpdate: "tool_call_update",
        toolCallId: "t",
        status: "completed",
      }),
      sequence: 2,
    });
    expect(
      (
        await p.apply({
          envelope: envelope("regress", {
            sessionUpdate: "tool_call_update",
            toolCallId: "t",
            status: "in_progress",
          }),
          sequence: 3,
        })
      ).reason,
    ).toBe("tool-state-regression");
    p.complete("end_turn", now);
    expect(
      (
        await p.apply({
          envelope: envelope("late", {
            sessionUpdate: "agent_message_chunk",
            content: { type: "text", text: "late" },
          }),
          sequence: 4,
        })
      ).reason,
    ).toBe("late-after-turn");
  });

  it("admits prompt settlement natively and emits exactly one terminal turn", async () => {
    const p = projector();
    const settled = createAcpRuntimeNativeEnvelope({
      profile: "grok",
      protocolVersion: 1,
      connectionRef: "connection.1",
      processGeneration: 7,
      method: "session/prompt",
      requestId: "prompt.1",
      updateId: "prompt-result.1",
      sessionId: "provider-session",
      observedAt: now,
      discriminant: "prompt_response/end_turn",
      validatedPayload: { stopReason: "end_turn" },
    });
    if ("kind" in settled) throw new Error("bad fixture");
    const first = await p.settle(settled, "end_turn");
    expect(first.filter((event) => event.kind === "turn.finished")).toHaveLength(1);
    expect(await p.settle(settled, "end_turn")).toEqual([]);
    const race = createAcpRuntimeNativeEnvelope({
      profile: "grok",
      protocolVersion: 1,
      connectionRef: "connection.1",
      processGeneration: 7,
      method: "session/prompt",
      requestId: "prompt.private-fallback",
      updateId: "prompt-result.2",
      sessionId: "provider-session",
      observedAt: now,
      discriminant: "_x.ai/session/prompt_complete",
      validatedPayload: { stopReason: "end_turn" },
    });
    if ("kind" in race) throw new Error("bad fixture");
    const late = await p.settle(race, "end_turn");
    expect(late).toMatchObject([{ kind: "degraded" }]);
  });

  it("preserves equal adjacent chunks and is duplicate-insertion idempotent", async () => {
    await fc.assert(
      fc.asyncProperty(
        fc.array(fc.string({ maxLength: 20 }), { minLength: 1, maxLength: 20 }),
        async (chunks) => {
          const p = projector();
          const events: string[] = [];
          for (let index = 0; index < chunks.length; index += 1) {
            const native = envelope(`property-${index}`, {
              sessionUpdate: "agent_message_chunk",
              content: { type: "text", text: chunks[index] },
            });
            const applied = await p.apply({ envelope: native, sequence: index });
            if (chunks[index])
              events.push(
                ...applied.events
                  .filter(
                    (event): event is Extract<typeof event, { kind: "text.delta" }> =>
                      event.kind === "text.delta",
                  )
                  .map((event) => event.text),
              );
            expect((await p.apply({ envelope: native, sequence: index })).outcome).toBe(
              "duplicate",
            );
          }
          expect(events.join("")).toBe(chunks.filter(Boolean).join(""));
        },
      ),
      { numRuns: 50 },
    );
  });

  it("terminalizes an unfinished tool exactly once when the turn ends", async () => {
    const p = projector();
    await p.apply({
      envelope: envelope("unfinished-tool", {
        sessionUpdate: "tool_call",
        toolCallId: "unfinished",
        title: "Long operation",
        status: "in_progress",
      }),
      sequence: 1,
    });
    const completed = p.complete("cancelled", now);
    expect(completed.filter((event) => event.kind === "tool.error")).toMatchObject([
      { messageSafe: "ACP tool interrupted by cancellation" },
    ]);
    expect(p.complete("cancelled", now)).toEqual([]);
  });
});

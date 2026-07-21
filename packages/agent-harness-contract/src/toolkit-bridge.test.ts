import { Effect, Schema as S, Stream } from "effect";
import { Tool, Toolkit } from "effect/unstable/ai";
import { RuntimeInteractionPayload } from "@openagentsinc/agent-runtime-schema";
import { describe, expect, test } from "vite-plus/test";

import {
  applyHostToolApprovalDecision,
  harnessHostToolSpecFromTool,
  harnessHostToolSpecsFromToolkit,
  hostToolApprovalInteractionPayload,
  hostToolCallToUiChunks,
  resolveHostToolCall,
  toolNeedsApproval,
} from "./toolkit-bridge.ts";
import { decodeUiMessageChunk } from "./ui-message-chunk.ts";

/**
 * Round-trip conformance for STREAM-07 (#9135): a schema-typed Effect AI
 * `Tool` projects to the `HarnessHostToolSpec` wire form, a harness host-tool
 * call resolves through the Toolkit handler Layer, `needsApproval` composes
 * with the canonical `RuntimeInteraction` approval model, and preliminary
 * handler results surface as streaming tool chunks.
 */

const EchoUpper = Tool.make("echo_upper", {
  description: "Uppercase the provided text.",
  parameters: S.Struct({ text: S.String }),
  success: S.Struct({ upper: S.String }),
  needsApproval: true,
});

const AddNumbers = Tool.make("add_numbers", {
  description: "Add two numbers.",
  parameters: S.Struct({ a: S.Number, b: S.Number }),
  success: S.Struct({ sum: S.Number }),
});

const kit = Toolkit.make(EchoUpper, AddNumbers);

const makeHandlers = (observed: { calls: Array<string> }) =>
  kit.toLayer(
    kit.of({
      echo_upper: ({ text }, ctx) =>
        Effect.gen(function* () {
          observed.calls.push(`echo_upper:${text}`);
          yield* ctx.preliminary({ upper: "PARTIAL" });
          return { upper: text.toUpperCase() };
        }),
      add_numbers: ({ a, b }) =>
        Effect.sync(() => {
          observed.calls.push(`add_numbers:${a}+${b}`);
          return { sum: a + b };
        }),
    }),
  );

describe("toolkit-bridge (STREAM-07)", () => {
  test("an Effect AI Tool projects to the HarnessHostToolSpec JSON Schema wire form", () => {
    const spec = harnessHostToolSpecFromTool(EchoUpper);
    expect(spec.name).toBe("echo_upper");
    expect(spec.description).toBe("Uppercase the provided text.");
    expect(spec.inputJsonSchema).toEqual({
      type: "object",
      properties: { text: { type: "string" } },
      required: ["text"],
      additionalProperties: false,
    });
  });

  test("a Toolkit projects to a deterministic name-sorted spec list", () => {
    const specs = harnessHostToolSpecsFromToolkit(kit);
    expect(specs.map((spec) => spec.name)).toEqual(["add_numbers", "echo_upper"]);
  });

  test("a valid host-tool call resolves through the handler Layer to the final result", async () => {
    const observed = { calls: [] as Array<string> };
    const result = await Effect.runPromise(
      resolveHostToolCall({
        toolkit: kit,
        call: { toolCallId: "toolcall.1", toolName: "add_numbers", input: { a: 2, b: 3 } },
      }).pipe(Effect.provide(makeHandlers(observed))),
    );
    expect(result).toEqual({ toolCallId: "toolcall.1", output: { sum: 5 } });
    expect(observed.calls).toEqual(["add_numbers:2+3"]);
  });

  test("the final result wins over preliminary results", async () => {
    const observed = { calls: [] as Array<string> };
    const result = await Effect.runPromise(
      resolveHostToolCall({
        toolkit: kit,
        call: { toolCallId: "toolcall.2", toolName: "echo_upper", input: { text: "hi" } },
        approval: "allow-once",
      }).pipe(Effect.provide(makeHandlers(observed))),
    );
    expect(result).toEqual({ toolCallId: "toolcall.2", output: { upper: "HI" } });
  });

  test("invalid input produces an isError result with safe detail, not a defect", async () => {
    const observed = { calls: [] as Array<string> };
    const result = await Effect.runPromise(
      resolveHostToolCall({
        toolkit: kit,
        call: { toolCallId: "toolcall.3", toolName: "add_numbers", input: { a: "nope" } },
      }).pipe(Effect.provide(makeHandlers(observed))),
    );
    expect(result.isError).toBe(true);
    expect(result.toolCallId).toBe("toolcall.3");
    const output = result.output as { error: string; detail: string };
    expect(output.error).toBe("ToolParameterValidationError");
    expect(observed.calls).toEqual([]);
  });

  test("an unknown tool produces an isError result", async () => {
    const observed = { calls: [] as Array<string> };
    const result = await Effect.runPromise(
      resolveHostToolCall({
        toolkit: kit,
        call: { toolCallId: "toolcall.4", toolName: "not_a_tool", input: {} },
      }).pipe(Effect.provide(makeHandlers(observed))),
    );
    expect(result.isError).toBe(true);
    expect((result.output as { error: string }).error).toBe("unknown_host_tool");
  });

  test("needsApproval evaluates statically and projects to a tool_approval interaction payload", async () => {
    expect(
      await Effect.runPromise(
        toolNeedsApproval(EchoUpper, { text: "x" }, { toolCallId: "toolcall.5" }),
      ),
    ).toBe(true);
    expect(
      await Effect.runPromise(
        toolNeedsApproval(AddNumbers, { a: 1, b: 2 }, { toolCallId: "toolcall.5" }),
      ),
    ).toBe(false);

    const payload = hostToolApprovalInteractionPayload(EchoUpper, {
      toolCallId: "toolcall.5",
      toolName: "echo_upper",
    });
    const decoded = S.decodeUnknownSync(RuntimeInteractionPayload)(payload);
    expect(decoded.kind).toBe("tool_approval");
    if (decoded.kind !== "tool_approval") throw new Error("expected tool_approval");
    expect(decoded.toolCallId).toBe("toolcall.5");
    expect(decoded.toolName).toBe("echo_upper");
    expect(decoded.authority.status).toBe("operator_escalation_required");
    expect(decoded.authority.allowed).toBe(false);
    expect(decoded.authority.blockerRefs).toEqual(["blocker.owner_approval"]);
  });

  test("a dynamic needsApproval function receives the decoded params and context", async () => {
    const Guarded = Tool.make("guarded", {
      parameters: S.Struct({ dangerous: S.Boolean }),
      success: S.String,
      needsApproval: (params: { readonly dangerous: boolean }) => params.dangerous,
    });
    expect(
      await Effect.runPromise(
        toolNeedsApproval(Guarded, { dangerous: true }, { toolCallId: "toolcall.6" }),
      ),
    ).toBe(true);
    expect(
      await Effect.runPromise(
        toolNeedsApproval(Guarded, { dangerous: false }, { toolCallId: "toolcall.6" }),
      ),
    ).toBe(false);
  });

  test("harness approval decisions map onto the canonical RuntimeInteraction decision", () => {
    expect(applyHostToolApprovalDecision("allow-once")).toEqual({
      decision: { kind: "tool_approval", outcome: "approve" },
      proceed: true,
      rememberForSession: false,
    });
    expect(applyHostToolApprovalDecision("allow-session")).toEqual({
      decision: { kind: "tool_approval", outcome: "approve" },
      proceed: true,
      rememberForSession: true,
    });
    expect(applyHostToolApprovalDecision("deny")).toEqual({
      decision: { kind: "tool_approval", outcome: "deny" },
      proceed: false,
      rememberForSession: false,
    });
  });

  test("a denied approval refuses the handler run without executing it", async () => {
    const observed = { calls: [] as Array<string> };
    const result = await Effect.runPromise(
      resolveHostToolCall({
        toolkit: kit,
        call: { toolCallId: "toolcall.7", toolName: "echo_upper", input: { text: "hi" } },
        approval: "deny",
      }).pipe(Effect.provide(makeHandlers(observed))),
    );
    expect(result.isError).toBe(true);
    expect((result.output as { error: string }).error).toBe("host_tool_denied");
    expect(observed.calls).toEqual([]);
  });

  test("preliminary handler results surface as streaming tool chunks before the final chunk", async () => {
    const observed = { calls: [] as Array<string> };
    const chunks = await Effect.runPromise(
      Stream.runCollect(
        hostToolCallToUiChunks({
          toolkit: kit,
          call: { toolCallId: "toolcall.8", toolName: "echo_upper", input: { text: "go" } },
          approval: "allow-once",
          makeResultRef: ({ preliminary, index }) =>
            `result.echo.${index}.${preliminary ? "preliminary" : "final"}`,
          cursor: 41,
        }),
      ).pipe(Effect.provide(makeHandlers(observed))),
    );
    expect(chunks.map((chunk) => chunk.type)).toEqual([
      "tool-output-preliminary",
      "tool-output-available",
    ]);
    expect(chunks[0]).toEqual({
      cursor: 41,
      type: "tool-output-preliminary",
      toolCallId: "toolcall.8",
      tool: { wireName: "echo_upper", nativeName: "echo_upper" },
      resultRef: "result.echo.0.preliminary",
    });
    expect(chunks[1]).toEqual({
      cursor: 41,
      type: "tool-output-available",
      toolCallId: "toolcall.8",
      tool: { wireName: "echo_upper", nativeName: "echo_upper" },
      resultRef: "result.echo.1.final",
    });
    // The preliminary chunk is a first-class member of the wire vocabulary.
    for (const chunk of chunks) {
      expect(decodeUiMessageChunk(chunk)).toEqual(chunk);
    }
  });

  test("a failing chunk-stream call emits one safe tool-output-error chunk", async () => {
    const observed = { calls: [] as Array<string> };
    const chunks = await Effect.runPromise(
      Stream.runCollect(
        hostToolCallToUiChunks({
          toolkit: kit,
          call: { toolCallId: "toolcall.9", toolName: "add_numbers", input: { a: "bad" } },
          makeResultRef: () => "result.unused",
        }),
      ).pipe(Effect.provide(makeHandlers(observed))),
    );
    expect(chunks).toHaveLength(1);
    const chunk = chunks[0];
    if (chunk?.type !== "tool-output-error") throw new Error("expected tool-output-error");
    expect(chunk.toolCallId).toBe("toolcall.9");
    expect(chunk.errorText).toContain("ToolParameterValidationError");
    expect(observed.calls).toEqual([]);
  });

  test("a denied chunk-stream call emits a refusal chunk without running the handler", async () => {
    const observed = { calls: [] as Array<string> };
    const chunks = await Effect.runPromise(
      Stream.runCollect(
        hostToolCallToUiChunks({
          toolkit: kit,
          call: { toolCallId: "toolcall.10", toolName: "echo_upper", input: { text: "hi" } },
          approval: "deny",
          makeResultRef: () => "result.unused",
        }),
      ).pipe(Effect.provide(makeHandlers(observed))),
    );
    expect(chunks.map((chunk) => chunk.type)).toEqual(["tool-output-error"]);
    expect(observed.calls).toEqual([]);
  });
});

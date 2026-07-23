import { Effect, Schema as S, Stream } from "effect";
import { Tool, type Toolkit } from "effect/unstable/ai";
import type { Prompt } from "effect/unstable/ai";
import type {
  RuntimeInteractionDecision,
  RuntimeInteractionPayload,
} from "@openagentsinc/agent-runtime-schema";
import { toolIdentity } from "./common-tool.ts";
import {
  HarnessHostToolSpec,
  type HarnessHostToolResult,
  type HarnessToolApprovalDecision,
} from "./host-tool.ts";
import type {
  UiToolOutputAvailableChunk,
  UiToolOutputErrorChunk,
  UiToolOutputPreliminaryChunk,
} from "./ui-message-chunk.ts";

/**
 * Toolkit bridge (STREAM-07 #9135): reconcile the harness host-tool surface
 * with the Effect v4 AI `Tool`/`Toolkit` substrate so ONE tool definition
 * serves both the model-call lanes (Effect AI) and the coding-agent harness
 * adapters. Source analysis:
 * `docs/fable/2026-07-21-ai-sdk-and-effect-ai-streaming-harvest-audit.md`
 * §4.1; the exact upstream API is verified against the installed dist at
 * `effect/dist/unstable/ai/Tool.d.ts` (`Tool.make`, `parametersSchema`,
 * `needsApproval`, `Tool.getJsonSchema`, `Tool.getDescription`,
 * `HandlerResult.preliminary`) and `effect/dist/unstable/ai/Toolkit.d.ts`
 * (`Toolkit.make`, `toLayer` handlers-as-Layer, `WithHandler.handle`
 * streaming `HandlerResult`s, `HandlerContext.preliminary`).
 *
 * The split is deliberate:
 *
 * - The typed Effect Schema `Tool` is the AUTHORING form. Handlers are
 *   supplied as a Layer (`toolkit.toLayer(handlers)`), so the same Layer
 *   serves `LanguageModel` tool-call resolution and harness host-tool
 *   dispatch.
 * - `HarnessHostToolSpec` (`inputJsonSchema`) stays the WIRE form — runtimes
 *   speak MCP/JSON Schema. {@link harnessHostToolSpecFromTool} is the
 *   projection between them (Schema → JSON Schema via `Tool.getJsonSchema`).
 * - Approvals ride the ONE canonical `RuntimeInteraction` model (kind
 *   `tool_approval`), mirroring `acpPermissionToRuntimeInteractionPayload` in
 *   `acp-adapter.ts` — a `needsApproval` tool never grows a second bespoke
 *   approval path.
 * - Preliminary handler results (`HandlerContext.preliminary`) map onto the
 *   STREAM-02 chunk vocabulary as `tool-output-preliminary`; the final result
 *   maps onto `tool-output-available`.
 */

// ---------------------------------------------------------------------------
// Toolkit → harness direction: Schema authoring form → JSON Schema wire form
// ---------------------------------------------------------------------------

const decodeHostToolSpec = S.decodeUnknownSync(HarnessHostToolSpec);

/**
 * Project an Effect AI `Tool` onto the harness wire form. The typed
 * `parametersSchema` is derived to JSON Schema with `Tool.getJsonSchema`
 * (which uses `Schema.toJsonSchemaDocument` internally, per
 * `Tool.getJsonSchemaFromSchema` in the dist), so a harness adapter can hand
 * the same tool to whatever native registration mechanism its runtime uses
 * (MCP, ACP, SDK). The projection decodes through `HarnessHostToolSpec`, so a
 * non-JSON-serializable derivation fails closed here, not on the wire.
 */
export const harnessHostToolSpecFromTool = (tool: Tool.Any): HarnessHostToolSpec =>
  decodeHostToolSpec({
    name: tool.name,
    description: Tool.getDescription(tool) ?? "",
    inputJsonSchema: Tool.getJsonSchema(tool),
  });

/**
 * Project every tool in a `Toolkit` onto the harness wire form, sorted by
 * name so the projection is deterministic regardless of construction order.
 */
export const harnessHostToolSpecsFromToolkit = (
  toolkit: Toolkit.Any,
): ReadonlyArray<HarnessHostToolSpec> =>
  Object.values(toolkit.tools)
    .map(harnessHostToolSpecFromTool)
    .sort((left, right) => left.name.localeCompare(right.name));

// ---------------------------------------------------------------------------
// Harness → Toolkit direction: host-tool call resolution through the handlers
// ---------------------------------------------------------------------------

/**
 * A host-tool call as the harness sees it: the runtime named a tool and
 * supplied JSON input. `input` is untrusted `unknown` — the tool's own
 * `parametersSchema` decodes it inside `WithHandler.handle` (a decode failure
 * surfaces as a typed `AiError` `ToolParameterValidationError`, which this
 * bridge folds into an `isError` result, never a defect).
 */
export interface HarnessHostToolCall {
  readonly toolCallId: string;
  readonly toolName: string;
  readonly input: unknown;
}

/** Options for {@link resolveHostToolCall} and {@link hostToolCallToUiChunks}. */
export interface ResolveHostToolCallOptions<Tools extends Record<string, Tool.Any>> {
  /** The toolkit whose handler Layer resolves the call. */
  readonly toolkit: Toolkit.Toolkit<Tools>;
  /** The call the runtime placed. */
  readonly call: HarnessHostToolCall;
  /**
   * Harness approval decision for this call, when the tool `needsApproval`.
   * `deny` refuses the handler run with an `isError` result; `allow-once` and
   * `allow-session` proceed. Omitted means no approval was required.
   */
  readonly approval?: HarnessToolApprovalDecision;
}

type ToolkitServices<Tools extends Record<string, Tool.Any>> =
  | Tool.HandlersFor<Tools>
  | Tool.HandlerServices<Tools[keyof Tools]>;

const MAX_SAFE_DETAIL_LENGTH = 500;

const boundedText = (text: string): string =>
  text.length > MAX_SAFE_DETAIL_LENGTH ? `${text.slice(0, MAX_SAFE_DETAIL_LENGTH)}…` : text;

/**
 * Bounded, public-shape failure detail. Prefers the precise `AiError` reason
 * tag (`ToolParameterValidationError`, ...) over the umbrella `_tag`, and
 * never echoes the raw error object.
 */
const safeFailureDetail = (error: unknown): { readonly tag: string; readonly detail: string } => {
  const record =
    typeof error === "object" && error !== null ? (error as Record<string, unknown>) : {};
  const reason =
    typeof record.reason === "object" && record.reason !== null
      ? (record.reason as Record<string, unknown>)
      : {};
  const tag =
    typeof reason._tag === "string"
      ? reason._tag
      : typeof record._tag === "string"
        ? record._tag
        : "UnknownError";
  const message =
    typeof record.message === "string"
      ? record.message
      : typeof reason.description === "string"
        ? reason.description
        : String(error);
  return { tag, detail: boundedText(message) };
};

const stringifyBounded = (value: unknown): string => {
  try {
    return boundedText(JSON.stringify(value) ?? "null");
  } catch {
    return "unserializable tool output";
  }
};

const errorResult = (toolCallId: string, error: string, detail: string): HarnessHostToolResult => ({
  toolCallId,
  output: { error, detail: boundedText(detail) },
  isError: true,
});

const decodeJsonOutput = S.decodeUnknownSync(S.Json);

interface FinalHandlerOutcome {
  /** Every streamed handler result, preliminary results in emission order. */
  readonly results: ReadonlyArray<Tool.HandlerResult<Tool.Any>>;
}

const collectHandlerResults = <Tools extends Record<string, Tool.Any>>(
  withHandler: Toolkit.WithHandler<Tools>,
  call: HarnessHostToolCall,
): Effect.Effect<FinalHandlerOutcome, unknown, ToolkitServices<Tools>> =>
  Effect.gen(function* () {
    const stream = yield* withHandler.handle(
      call.toolName as keyof Tools,
      call.input as Tool.Parameters<Tools[keyof Tools]>,
    );
    const results = yield* Stream.runCollect(stream);
    return { results } as FinalHandlerOutcome;
  }) as Effect.Effect<FinalHandlerOutcome, unknown, ToolkitServices<Tools>>;

const handlerOutcomeToResult = (
  call: HarnessHostToolCall,
  outcome: FinalHandlerOutcome,
): HarnessHostToolResult => {
  let final: Tool.HandlerResult<Tool.Any> | undefined;
  for (let i = outcome.results.length - 1; i >= 0; i -= 1) {
    const candidate = outcome.results[i];
    if (candidate !== undefined && !candidate.preliminary) {
      final = candidate;
      break;
    }
  }
  if (final === undefined) {
    return errorResult(
      call.toolCallId,
      "no_tool_result",
      `Handler for "${call.toolName}" completed without a final result.`,
    );
  }
  let output: HarnessHostToolResult["output"];
  try {
    output = decodeJsonOutput(final.encodedResult);
  } catch {
    return errorResult(
      call.toolCallId,
      "unserializable_tool_output",
      `Handler for "${call.toolName}" returned a non-JSON encoded result.`,
    );
  }
  return {
    toolCallId: call.toolCallId,
    output,
    ...(final.isFailure ? { isError: true } : {}),
  };
};

/**
 * Resolve a harness host-tool call through the Toolkit handler Layer: decode
 * the untrusted JSON input with the tool's `parametersSchema`, run the
 * handler, and fold the streamed `HandlerResult`s into one
 * `HarnessHostToolResult` (`encodedResult` of the final, authoritative
 * result). Failures never escape as defects or typed failures — an unknown
 * tool, an input decode failure (`AiError` `ToolParameterValidationError`), a
 * handler error, and a `failureMode: "return"` failure result all produce an
 * `isError` result with bounded safe detail, because the harness submits the
 * result back to the runtime either way.
 *
 * When `approval` is `deny`, the handler NEVER runs and the result is a
 * refusal — this is the enforcement half of the one-path approval model (see
 * {@link hostToolApprovalInteractionPayload}).
 */
export const resolveHostToolCall = <Tools extends Record<string, Tool.Any>>(
  options: ResolveHostToolCallOptions<Tools>,
): Effect.Effect<HarnessHostToolResult, never, ToolkitServices<Tools>> =>
  Effect.gen(function* () {
    const { call } = options;
    if (options.approval === "deny") {
      return errorResult(
        call.toolCallId,
        "host_tool_denied",
        `Approval decision "deny" refused the "${call.toolName}" handler run.`,
      );
    }
    const withHandler = yield* options.toolkit;
    if (!Object.hasOwn(withHandler.tools, call.toolName)) {
      return errorResult(
        call.toolCallId,
        "unknown_host_tool",
        `Tool "${call.toolName}" is not in the toolkit.`,
      );
    }
    return yield* collectHandlerResults(withHandler, call).pipe(
      Effect.map((outcome) => handlerOutcomeToResult(call, outcome)),
      Effect.catch((error) => {
        const { tag, detail } = safeFailureDetail(error);
        return Effect.succeed(errorResult(call.toolCallId, tag, detail));
      }),
    );
  }) as Effect.Effect<HarnessHostToolResult, never, ToolkitServices<Tools>>;

// ---------------------------------------------------------------------------
// Approval composition: needsApproval → RuntimeInteraction (kind tool_approval)
// ---------------------------------------------------------------------------

/**
 * Evaluate a tool's `needsApproval` declaration for one call. Covers all
 * three upstream shapes (`Tool.NeedsApproval` in the dist): absent/`false`
 * (no approval), `true` (always), and a `NeedsApprovalFunction` receiving the
 * decoded params plus a `NeedsApprovalContext` (`toolCallId`, `messages`) and
 * returning `boolean | Effect<boolean>`.
 */
export const toolNeedsApproval = <T extends Tool.Any>(
  tool: T,
  params: Tool.Parameters<T>,
  context: {
    readonly toolCallId: string;
    readonly messages?: ReadonlyArray<Prompt.Message>;
  },
): Effect.Effect<boolean> => {
  const needs = tool.needsApproval;
  if (needs === undefined || typeof needs === "boolean") {
    return Effect.succeed(needs === true);
  }
  const evaluated = needs(params, {
    toolCallId: context.toolCallId,
    messages: context.messages ?? [],
  });
  return typeof evaluated === "boolean" ? Effect.succeed(evaluated) : evaluated;
};

/**
 * Project a `needsApproval` host-tool call onto the canonical
 * `RuntimeInteractionPayload` of kind `tool_approval` — the durable,
 * provider-neutral approval model every harness approval routes through
 * (HARN-04). This mirrors `acpPermissionToRuntimeInteractionPayload` in
 * `acp-adapter.ts`: the pending request carries
 * `operator_escalation_required` authority (owner decision pending), never a
 * self-granted allow, and the decision is applied downstream via
 * `applyRuntimeInteractionDecision`. There is ONE approval path; the Toolkit
 * `needsApproval` flag composes with it instead of adding a second.
 */
export const hostToolApprovalInteractionPayload = (
  tool: Tool.Any,
  call: Pick<HarnessHostToolCall, "toolCallId" | "toolName">,
): RuntimeInteractionPayload => {
  const identity = toolIdentity(call.toolName);
  return {
    kind: "tool_approval",
    displayText: `Allow the host tool ${identity.wireName} to run?`,
    toolCallId: call.toolCallId,
    toolName: identity.wireName,
    authority: {
      authorityRef: `authority.host_tool.${call.toolCallId}`,
      policyRef: "policy.toolkit_bridge",
      decisionRef: "decision.host_tool_approval_pending",
      toolRef: `toolref.host_tool.${tool.name}`,
      status: "operator_escalation_required",
      allowed: false,
      blockerRefs: ["blocker.owner_approval"],
    },
  };
};

/**
 * The application of one harness approval decision: the canonical
 * `RuntimeInteractionDecision` to resolve the interaction with, whether the
 * handler run proceeds, and whether the approval persists for the session.
 */
export interface HostToolApprovalApplication {
  /** The decision that resolves the `tool_approval` interaction. */
  readonly decision: Extract<RuntimeInteractionDecision, { readonly kind: "tool_approval" }>;
  /** Whether the Toolkit handler run proceeds. */
  readonly proceed: boolean;
  /** `allow-session` remembers the approval for the rest of the session. */
  readonly rememberForSession: boolean;
}

/**
 * Map the harness approval decisions (`allow-once` / `allow-session` /
 * `deny`, `HARNESS_TOOL_APPROVAL_DECISIONS` in `host-tool.ts`) onto the
 * binary `RuntimeInteractionDecision` outcome plus the proceed/refuse and
 * session-memory semantics. `resolveHostToolCall` consumes the same decision
 * via its `approval` option, so decision projection and enforcement cannot
 * drift.
 */
export const applyHostToolApprovalDecision = (
  decision: HarnessToolApprovalDecision,
): HostToolApprovalApplication =>
  decision === "deny"
    ? {
        decision: { kind: "tool_approval", outcome: "deny" },
        proceed: false,
        rememberForSession: false,
      }
    : {
        decision: { kind: "tool_approval", outcome: "approve" },
        proceed: true,
        rememberForSession: decision === "allow-session",
      };

// ---------------------------------------------------------------------------
// Preliminary results → streaming tool chunks (STREAM-02 vocabulary)
// ---------------------------------------------------------------------------

/** The chunk subset a host-tool call run emits. */
export type HostToolUiChunk =
  | UiToolOutputPreliminaryChunk
  | UiToolOutputAvailableChunk
  | UiToolOutputErrorChunk;

/** Options for {@link hostToolCallToUiChunks}. */
export interface HostToolCallChunkOptions<
  Tools extends Record<string, Tool.Any>,
> extends ResolveHostToolCallOptions<Tools> {
  /**
   * Mint the safe `resultRef` for one emitted result. The chunk vocabulary
   * carries refs, never raw payloads (STREAM-02) — the caller stores the raw
   * `encodedResult` wherever its redaction model allows and returns the ref.
   * `index` is the zero-based emission index across preliminary and final
   * results.
   */
  readonly makeResultRef: (meta: {
    readonly preliminary: boolean;
    readonly index: number;
  }) => string;
  /** Durable replay cursor stamped on every emitted chunk, when known. */
  readonly cursor?: number;
}

/**
 * Run a host-tool call and emit the STREAM-02 chunk projection live: each
 * preliminary `HandlerResult` (emitted by the handler through
 * `HandlerContext.preliminary`) becomes a `tool-output-preliminary` chunk,
 * the final authoritative result becomes `tool-output-available`, and every
 * failure shape (denied approval, unknown tool, input decode failure, handler
 * error, `failureMode: "return"` failure result) becomes `tool-output-error`
 * with bounded safe text. The stream never fails — errors are chunks.
 */
export const hostToolCallToUiChunks = <Tools extends Record<string, Tool.Any>>(
  options: HostToolCallChunkOptions<Tools>,
): Stream.Stream<HostToolUiChunk, never, ToolkitServices<Tools>> => {
  const { call } = options;
  const tool = toolIdentity(call.toolName);
  const base = options.cursor === undefined ? {} : { cursor: options.cursor };
  const errorChunk = (errorText: string): UiToolOutputErrorChunk => ({
    ...base,
    type: "tool-output-error",
    toolCallId: call.toolCallId,
    tool,
    errorText: boundedText(errorText),
  });
  const resultChunk = (result: Tool.HandlerResult<Tool.Any>, index: number): HostToolUiChunk => {
    if (result.isFailure) {
      return errorChunk(stringifyBounded(result.encodedResult));
    }
    const resultRef = options.makeResultRef({ preliminary: result.preliminary, index });
    return result.preliminary
      ? { ...base, type: "tool-output-preliminary", toolCallId: call.toolCallId, tool, resultRef }
      : { ...base, type: "tool-output-available", toolCallId: call.toolCallId, tool, resultRef };
  };
  return Stream.unwrap(
    Effect.gen(function* () {
      if (options.approval === "deny") {
        return Stream.make(
          errorChunk(`Approval decision "deny" refused the "${call.toolName}" handler run.`),
        );
      }
      const withHandler = yield* options.toolkit;
      if (!Object.hasOwn(withHandler.tools, call.toolName)) {
        return Stream.make(errorChunk(`Tool "${call.toolName}" is not in the toolkit.`));
      }
      const errorChunkStream = (error: unknown): Stream.Stream<HostToolUiChunk> => {
        const { tag, detail } = safeFailureDetail(error);
        return Stream.make(errorChunk(`${tag}: ${detail}`));
      };
      return yield* withHandler
        .handle(call.toolName as keyof Tools, call.input as Tool.Parameters<Tools[keyof Tools]>)
        .pipe(
          Effect.map((results) =>
            results.pipe(
              Stream.mapAccum(
                () => 0,
                (index: number, result: Tool.HandlerResult<Tool.Any>) =>
                  [index + 1, [resultChunk(result, index)]] as const,
              ),
              Stream.catch(errorChunkStream),
            ),
          ),
          Effect.catch((error) => Effect.succeed(errorChunkStream(error))),
        );
    }) as Effect.Effect<
      Stream.Stream<HostToolUiChunk, never, ToolkitServices<Tools>>,
      never,
      ToolkitServices<Tools>
    >,
  );
};

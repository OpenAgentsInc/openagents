import { Effect } from "effect";
import { ProbeLlmEvents, type ProbeLlmEvent } from "./events.js";
import { makeProbeLlmToolResultValue } from "./messages.js";
import { projectProbeLlmToolResult, type ProbeLlmTools } from "./tool.js";

export interface ProbeLlmToolCallInput {
  readonly id: string;
  readonly name: string;
  readonly input: unknown;
}

export interface ProbeLlmToolDispatchResult {
  readonly result: ReturnType<typeof makeProbeLlmToolResultValue>;
  readonly events: ReadonlyArray<ProbeLlmEvent>;
}

export function dispatchProbeLlmTool(
  tools: ProbeLlmTools,
  call: ProbeLlmToolCallInput,
): Effect.Effect<ProbeLlmToolDispatchResult, never> {
  const tool = tools[call.name];

  if (tool === undefined) {
    return Effect.succeed(makeToolError(call, `Unknown tool: ${call.name}`));
  }

  if (tool.execute === undefined) {
    return Effect.succeed(makeToolError(call, `Tool has no execute handler: ${call.name}`));
  }

  if (!isRecord(call.input)) {
    return Effect.succeed(makeToolError(call, "Invalid tool input: expected an object"));
  }

  return tool.execute(call.input, { id: call.id, name: call.name }).pipe(
    Effect.map((output) => {
      const result = projectProbeLlmToolResult(tool, output);

      return {
        result,
        events: [
          ProbeLlmEvents.toolResult({
            id: call.id,
            name: call.name,
            result,
          }),
        ],
      };
    }),
    Effect.catchTag("ProbeLlmToolFailure", (failure) => Effect.succeed(makeToolError(call, failure.message))),
  );
}

function makeToolError(call: ProbeLlmToolCallInput, message: string): ProbeLlmToolDispatchResult {
  const result = makeProbeLlmToolResultValue(message, "error");

  return {
    result,
    events: [
      ProbeLlmEvents.toolError({
        id: call.id,
        name: call.name,
        message,
      }),
      ProbeLlmEvents.toolResult({
        id: call.id,
        name: call.name,
        result,
      }),
    ],
  };
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

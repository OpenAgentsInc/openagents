import { Effect, Schema as S } from "effect";
import { ProbeLlmToolResultValue, makeProbeLlmToolResultValue } from "./messages";

export const ProbeLlmJsonSchema = S.Record(S.String, S.Unknown);
export type ProbeLlmJsonSchema = typeof ProbeLlmJsonSchema.Type;

export const ProbeLlmToolDefinition = S.Struct({
  name: S.String,
  description: S.String,
  inputSchema: ProbeLlmJsonSchema,
  outputSchema: S.optional(ProbeLlmJsonSchema),
});
export type ProbeLlmToolDefinition = typeof ProbeLlmToolDefinition.Type;

export class ProbeLlmToolFailure extends S.TaggedErrorClass<ProbeLlmToolFailure>()("ProbeLlmToolFailure", {
  message: S.String,
}) {}

export interface ProbeLlmToolExecuteContext {
  readonly id: string;
  readonly name: string;
}

export interface ProbeLlmTool {
  readonly name: string;
  readonly description: string;
  readonly inputSchema: ProbeLlmJsonSchema;
  readonly outputSchema?: ProbeLlmJsonSchema;
  readonly execute?: (
    input: Readonly<Record<string, unknown>>,
    context: ProbeLlmToolExecuteContext,
  ) => Effect.Effect<unknown, ProbeLlmToolFailure>;
  readonly projectResult?: (output: unknown) => ProbeLlmToolResultValue;
}

export type ProbeLlmTools = Readonly<Record<string, ProbeLlmTool>>;

export function defineProbeLlmTool(input: ProbeLlmTool): ProbeLlmTool {
  return input;
}

export function probeLlmToolDefinitions(tools: ProbeLlmTools): ReadonlyArray<ProbeLlmToolDefinition> {
  return Object.entries(tools).map(([name, tool]) => ({
    name,
    description: tool.description,
    inputSchema: tool.inputSchema,
    outputSchema: tool.outputSchema,
  }));
}

export function projectProbeLlmToolResult(tool: ProbeLlmTool, output: unknown): ProbeLlmToolResultValue {
  return tool.projectResult?.(output) ?? makeProbeLlmToolResultValue(output);
}

import { Context, Effect, Schema } from "effect";

export class ToolCallError extends Schema.TaggedError<ToolCallError>()(
  "ToolCallError",
  {
    toolName: Schema.String,
    message: Schema.String,
    cause: Schema.optional(Schema.Defect)
  }
) {}

export type ToolExecutor = {
  readonly call: (options: {
    readonly toolName: string;
    readonly input: unknown;
    readonly timeoutMs?: number | undefined;
  }) => Effect.Effect<unknown, ToolCallError>;
};

export class ToolExecutorService extends Context.Tag(
  "@openagentsinc/dse/ToolExecutor"
)<ToolExecutorService, ToolExecutor>() {}


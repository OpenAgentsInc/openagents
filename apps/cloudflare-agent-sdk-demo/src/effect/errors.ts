import * as Data from "effect/Data";

export class WorkersError extends Data.TaggedError("WorkersError")<{
  readonly message: string;
  readonly cause?: unknown;
}> {}

export class ExecutionContextError extends Data.TaggedError(
  "ExecutionContextError"
)<{
  readonly message: string;
  readonly cause?: unknown;
}> {}

import { Effect, pipe } from "effect";
import * as ParseResult from "effect/ParseResult";
import * as S from "effect/Schema";

export type ToolContent =
	| { type: "text"; text: string }
	| { type: "image"; data: string; mimeType: string };

export interface ToolResult<Details = unknown> {
	content: ToolContent[];
	details?: Details;
}

export type ToolErrorReason =
	| "invalid_arguments"
	| "not_found"
	| "missing_old_text"
	| "not_unique"
	| "unchanged"
	| "command_failed"
	| "aborted";

export class ToolExecutionError extends Error {
  readonly _tag = "ToolExecutionError";
  constructor(readonly reason: ToolErrorReason, message: string) {
    super(message);
    this.name = "ToolExecutionError";
  }
}

export interface Tool<Params, Details = unknown, R = never, E = never> {
  name: string;
  label: string;
  description: string;
  schema: S.Schema<Params>;
  execute: (
    params: Params,
    options?: { signal?: AbortSignal },
  ) => Effect.Effect<ToolResult<Details>, E | ToolExecutionError, R>;
}

export const runTool = <Params, Details, R, E>(
  tool: Tool<Params, Details, R, E>,
  input: unknown,
  options?: { signal?: AbortSignal },
) =>
  pipe(
    input,
    S.decodeUnknown(tool.schema),
    Effect.mapError(
      (error: ParseResult.ParseError) =>
        new ToolExecutionError("invalid_arguments", error.message),
    ),
    Effect.flatMap((params) => tool.execute(params, options)),
  );

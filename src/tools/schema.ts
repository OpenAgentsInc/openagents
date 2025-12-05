import { Effect, pipe } from "effect";
import * as Stream from "effect/Stream";
import * as ParseResult from "effect/ParseResult";
import * as S from "effect/Schema";

export type ToolContent =
	| { type: "text"; text: string }
	| { type: "image"; data: string; mimeType: string };

export const isTextContent = (content: ToolContent): content is { type: "text"; text: string } =>
	content.type === "text";

export interface ToolResult<Details = unknown> {
  content: ToolContent[];
  details?: Details;
}

export interface StreamingToolResult<Details = unknown> extends ToolResult<Details> {
  /**
   * Optional stream for live tool output (e.g., ANSI text from bash).
   * Consumers can attach to this stream to surface incremental progress (HUD, logs).
   */
  stream?: Stream.Stream<ToolContent>;
}

export interface ToolRunOptions {
  signal?: AbortSignal;
  onStream?: (chunk: ToolContent) => void;
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
    options?: ToolRunOptions,
  ) => Effect.Effect<StreamingToolResult<Details>, E | ToolExecutionError, R>;
}

export const runTool = <Params, Details, R, E>(
  tool: Tool<Params, Details, R, E>,
  input: unknown,
  options?: ToolRunOptions,
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

import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import * as S from "effect/Schema";
import type { Tool } from "./schema.js";
import { ToolExecutionError } from "./schema.js";
import * as Stream from "effect/Stream";
import * as Command from "@effect/platform/Command";
import * as CommandExecutor from "@effect/platform/CommandExecutor";

interface GrepDetails {
  pattern: string;
  path: string;
  resolvedPath: string;
  command: string;
  fixed: boolean;
  ignoreCase: boolean;
  maxResults?: number;
  matches: number;
  exitCode: number;
  durationMs: number;
  outputBytes: number;
  stderrBytes: number;
}

const GrepParametersSchema = S.Struct({
  pattern: S.String.pipe(S.minLength(1), S.annotations({ description: "Regex or fixed string to search for" })),
  path: S.optional(S.String),
  fixed: S.optional(S.Boolean),
  ignoreCase: S.optional(S.Boolean),
  maxResults: S.optional(S.Number.pipe(S.int(), S.greaterThan(0))),
});

type GrepParameters = S.Schema.Type<typeof GrepParametersSchema>;

const collectOutput = (stream: Stream.Stream<Uint8Array, never, never>) =>
  Stream.runFold(stream, { text: "", bytes: 0 }, (state, chunk) => {
    const text = Buffer.from(chunk).toString("utf-8");
    return { text: state.text + text, bytes: state.bytes + chunk.length };
  });

const buildArgs = (params: GrepParameters) => {
  const args = ["-n", "-H"];
  if (params.fixed) args.push("-F");
  if (params.ignoreCase) args.push("-i");
  if (params.maxResults) args.push("-m", String(params.maxResults));
  args.push(params.pattern, params.path ?? ".");
  return args;
};

export const grepTool: Tool<
  GrepParameters,
  GrepDetails,
  CommandExecutor.CommandExecutor | Path.Path | FileSystem.FileSystem
> = {
  name: "grep",
  label: "grep",
  description: "Search for a pattern in files using ripgrep (rg). Returns matching lines with line numbers.",
  schema: GrepParametersSchema,
  execute: (params) =>
    Effect.scoped(
      Effect.gen(function* () {
        const pathService = yield* Path.Path;
        const executor = yield* CommandExecutor.CommandExecutor;
        const fs = yield* FileSystem.FileSystem;

        const target = pathService.resolve(params.path ?? ".");
        const exists = yield* fs.exists(target).pipe(
          Effect.mapError(
            (e) => new ToolExecutionError("command_failed", `Failed to check path: ${e.message}`),
          ),
        );
        if (!exists) {
          return yield* Effect.fail(
            new ToolExecutionError("not_found", `Path not found: ${params.path ?? "."}`),
          );
        }

        const args = buildArgs(params);
        const cmd = Command.make("rg", ...args);
        const commandString = ["rg", ...args].join(" ");
        const startedAt = Date.now();

        const proc = yield* Effect.acquireRelease(
          executor.start(cmd),
          (p) =>
            p.isRunning.pipe(
              Effect.flatMap((running) => (running ? p.kill("SIGKILL") : Effect.void)),
              Effect.orElse(() => Effect.void),
            ),
        ).pipe(Effect.mapError((e) => new ToolExecutionError("command_failed", String(e))));

        const [stdout, stderr, exitCode] = yield* Effect.all([
          collectOutput(proc.stdout as any),
          collectOutput(proc.stderr as any),
          proc.exitCode,
        ]).pipe(Effect.mapError((e) => new ToolExecutionError("command_failed", String(e))));

        const durationMs = Date.now() - startedAt;
        const exitNum = Number(exitCode as unknown as number);
        if (exitNum !== 0 && exitNum !== 1) {
          return yield* Effect.fail(
            new ToolExecutionError(
              "command_failed",
              `rg exited with code ${exitNum}: ${stderr.text || stdout.text || "(no output)"}`,
            ),
          );
        }

        const output = stdout.text.trim();
        const matches = output ? output.split("\n").filter((line) => line.trim().length > 0).length : 0;
        const baseText = output || "No matches found.";

        return {
          content: [{ type: "text" as const, text: baseText }],
          details: {
            pattern: params.pattern,
            path: params.path ?? ".",
            resolvedPath: target,
            command: commandString,
            fixed: params.fixed ?? false,
            ignoreCase: params.ignoreCase ?? false,
            ...(params.maxResults !== undefined ? { maxResults: params.maxResults } : {}),
            matches,
            exitCode: exitNum,
            durationMs,
            outputBytes: stdout.bytes,
            stderrBytes: stderr.bytes,
          },
        };
      }),
    ) as any,
};

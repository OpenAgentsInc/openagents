import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import * as S from "effect/Schema";
import type { Tool } from "./schema.js";
import { ToolExecutionError } from "./schema.js";
import * as Stream from "effect/Stream";
import * as Command from "@effect/platform/Command";
import * as CommandExecutor from "@effect/platform/CommandExecutor";

const GrepParametersSchema = S.Struct({
  pattern: S.String.pipe(S.minLength(1), S.annotations({ description: "Regex or fixed string to search for" })),
  path: S.optional(S.String),
  fixed: S.optional(S.Boolean),
  ignoreCase: S.optional(S.Boolean),
  maxResults: S.optional(S.Number.pipe(S.int(), S.greaterThan(0))),
});

type GrepParameters = S.Schema.Type<typeof GrepParametersSchema>;

const collectOutput = (stream: Stream.Stream<Uint8Array, never, never>) =>
  Stream.runFold(stream, "", (acc, chunk) => acc + Buffer.from(chunk).toString("utf-8"));

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
  undefined,
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

      const exitNum = Number(exitCode as unknown as number);
      if (exitNum !== 0 && exitNum !== 1) {
        return yield* Effect.fail(
          new ToolExecutionError(
            "command_failed",
            `rg exited with code ${exitNum}: ${stderr || stdout || "(no output)"}`,
          ),
        );
      }

      const output = stdout.trim();
      if (!output) {
        return {
          content: [{ type: "text" as const, text: "No matches found." }],
        };
      }

      return {
        content: [{ type: "text" as const, text: output }],
      };
    }),
    ) as any,
};

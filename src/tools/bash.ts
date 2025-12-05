import * as Command from "@effect/platform/Command";
import * as CommandExecutor from "@effect/platform/CommandExecutor";
import { Effect } from "effect";
import * as Stream from "effect/Stream";
import * as S from "effect/Schema";
import type { Tool } from "./schema.js";
import { ToolExecutionError } from "./schema.js";

const MAX_OUTPUT = 10 * 1024 * 1024; // 10 MB

const BashParametersSchema = S.Struct({
  command: S.String.pipe(
    S.minLength(1),
    S.annotations({ description: "Bash command to execute" }),
  ),
  timeout: S.optional(
    S.Number.pipe(
      S.greaterThan(0),
      S.annotations({ description: "Timeout in seconds (optional, no default timeout)" }),
    ),
  ),
});

type BashParameters = S.Schema.Type<typeof BashParametersSchema>;

const collectLimited = (stream: Stream.Stream<Uint8Array, unknown, unknown>): Effect.Effect<string, unknown, unknown> =>
  Stream.runFold(stream, "", (acc, chunk) => {
    if (acc.length >= MAX_OUTPUT) {
      return acc;
    }
    const remaining = MAX_OUTPUT - acc.length;
    const text = Buffer.from(chunk.subarray(0, remaining)).toString("utf-8");
    return acc + text;
  });

export const bashTool: Tool<BashParameters, undefined, CommandExecutor.CommandExecutor> = {
  name: "bash",
  label: "bash",
  description:
    "Execute a bash command in the current working directory. Returns stdout and stderr. Optionally provide a timeout in seconds.",
  // Local-context: runs inline in this process; cannot be suspended or resumed mid-command.
  schema: BashParametersSchema,
  execute: (params) =>
    Effect.scoped(
      Effect.gen(function* () {
        const executor = yield* CommandExecutor.CommandExecutor;

        const cmd = Command.make("sh", "-c", params.command);

        const process = yield* Effect.acquireRelease(executor.start(cmd), (proc) =>
          proc.isRunning.pipe(
            Effect.flatMap((running) => (running ? proc.kill("SIGKILL") : Effect.void)),
            Effect.orElse(() => Effect.void),
          ),
        ).pipe(
          Effect.mapError((e) => new ToolExecutionError("aborted", String(e))),
        );

        const baseCollect = Effect.all(
          [collectLimited(process.stdout as any), collectLimited(process.stderr as any), process.exitCode] as const,
        ).pipe(
          Effect.mapError((e) => new ToolExecutionError("aborted", String(e))),
        );

        const withTimeout =
          params.timeout && params.timeout > 0
            ? Effect.timeoutFail(baseCollect, {
                duration: params.timeout * 1000,
                onTimeout: () =>
                  new ToolExecutionError("aborted", `Command timed out after ${params.timeout} seconds`),
              })
            : baseCollect;

        const [stdout, stderr, exitCode] = yield* withTimeout;
        const output =
          [stdout, stderr].filter((s) => s && s.trim() !== "").join("\n") || "(no output)";

        const exitNum = Number(exitCode as unknown as number);
        if (Number.isFinite(exitNum) && exitNum !== 0) {
          return yield* Effect.fail(
            new ToolExecutionError("command_failed", `${output}\n\nCommand exited with code ${exitNum}`),
          );
        }

        return {
          content: [{ type: "text" as const, text: output }],
        };
      }),
    ) as any,
};

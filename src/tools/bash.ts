import * as Command from "@effect/platform/Command";
import * as CommandExecutor from "@effect/platform/CommandExecutor";
import { Effect } from "effect";
import * as Stream from "effect/Stream";
import * as S from "effect/Schema";
import type { Tool } from "./schema.js";
import { ToolExecutionError } from "./schema.js";

const MAX_OUTPUT = 10 * 1024 * 1024; // 10 MB
const normalizeExitCode = (exitCode: unknown) => {
  const asNumber = Number(exitCode as number);
  return Number.isFinite(asNumber) ? asNumber : 0;
};

interface BashDetails {
  command: string;
  timeoutSeconds?: number;
  exitCode: number;
  durationMs: number;
  outputBytes: number;
  truncatedOutput: boolean;
}

type LimitedOutput = { text: string; bytes: number; truncated: boolean };

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

const collectLimited = (
  stream: Stream.Stream<Uint8Array, unknown, unknown>,
): Effect.Effect<LimitedOutput, unknown, unknown> =>
  Stream.runFold(
    stream,
    { text: "", bytes: 0, truncated: false } as LimitedOutput,
    (state, chunk) => {
      if (state.truncated) {
        return { ...state, bytes: state.bytes + chunk.length };
      }

      const remaining = Math.max(0, MAX_OUTPUT - state.text.length);
      const text = Buffer.from(chunk.subarray(0, remaining)).toString("utf-8");
      const truncated = state.truncated || state.text.length + chunk.length > MAX_OUTPUT;

      return {
        text: state.text + text,
        bytes: state.bytes + chunk.length,
        truncated,
      };
    },
  );

export const bashTool: Tool<BashParameters, BashDetails, CommandExecutor.CommandExecutor> = {
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
        const startedAt = Date.now();

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
        const durationMs = Date.now() - startedAt;
        const output =
          [stdout.text, stderr.text].filter((s) => s && s.trim() !== "").join("\n") || "(no output)";

        const exitNum = normalizeExitCode(exitCode);
        if (Number.isFinite(exitNum) && exitNum !== 0) {
          return yield* Effect.fail(
            new ToolExecutionError("command_failed", `${output}\n\nCommand exited with code ${exitNum}`),
          );
        }

        return {
          content: [{ type: "text" as const, text: output }],
          details: {
            command: params.command,
            timeoutSeconds: params.timeout,
            exitCode: exitNum,
            durationMs,
            outputBytes: stdout.bytes + stderr.bytes,
            truncatedOutput: stdout.truncated || stderr.truncated,
          },
        };
      }),
    ) as any,
};

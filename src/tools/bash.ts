import * as Command from "@effect/platform/Command";
import * as CommandExecutor from "@effect/platform/CommandExecutor";
import { Effect } from "effect";
import * as Stream from "effect/Stream";
import * as Ref from "effect/Ref";
import * as S from "effect/Schema";
import type { Tool, ToolContent } from "./schema.js";
import { ToolExecutionError } from "./schema.js";

const MAX_OUTPUT = 10 * 1024 * 1024; // 10 MB
const normalizeExitCode = (exitCode: unknown) => {
  const asNumber = Number(exitCode as number);
  return Number.isFinite(asNumber) ? asNumber : 0;
};

interface BashDetails {
  command: string;
  runInBackground: boolean;
  pid?: number;
  timeoutSeconds: number | undefined;
  exitCode: number | undefined;
  durationMs: number | undefined;
  outputBytes: number | undefined;
  truncatedOutput: boolean | undefined;
  streaming?: boolean;
}

type LimitedOutput = { text: string; bytes: number; truncated: boolean };
type BashStreamDetails = BashDetails & { stream: Stream.Stream<ToolContent, ToolExecutionError> };

const BashParametersSchema = S.Struct({
  command: S.String.pipe(
    S.minLength(1),
    S.annotations({ description: "Bash command to execute" }),
  ),
  description: S.optional(
    S.String.pipe(S.annotations({ description: "Short description of what this command does" })),
  ),
  run_in_background: S.optional(S.Boolean),
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
    "Execute a bash command in the current working directory. Supports optional description, background execution, and timeout in seconds. Returns stdout/stderr when run in foreground.",
  // Local-context: runs inline in this process; cannot be suspended or resumed mid-command.
  schema: BashParametersSchema,
  execute: (params) =>
    Effect.scoped(
      Effect.gen(function* () {
        const executor = yield* CommandExecutor.CommandExecutor;

        const cmd = Command.make("sh", "-c", params.command);
        const startedAt = Date.now();

        if (params.run_in_background) {
          const process = yield* executor.start(cmd).pipe(
            Effect.mapError((e) => new ToolExecutionError("aborted", String(e))),
          );

          return {
            content: [
              {
                type: "text" as const,
                text: `Started background process${process.pid ? ` (pid ${process.pid})` : ""} for ${params.command}`,
              },
            ],
            details: {
              command: params.command,
              runInBackground: true,
              pid: process.pid,
              timeoutSeconds: params.timeout,
              exitCode: -1,
              durationMs: Date.now() - startedAt,
              outputBytes: 0,
              truncatedOutput: false,
            },
          };
        }

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
            runInBackground: false,
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

export const bashStreamTool: Tool<BashParameters, BashStreamDetails, CommandExecutor.CommandExecutor> = {
  name: "bash_stream",
  label: "bash_stream",
  description:
    "Execute a bash command and stream stdout/stderr incrementally. Emits chunks as ToolContent text blocks and appends an exit code chunk when done.",
  schema: BashParametersSchema,
  execute: (params) =>
    Effect.gen(function* () {
      const executor = yield* CommandExecutor.CommandExecutor;
      const cmd = Command.make("sh", "-c", params.command);
      const bytesRef = yield* Ref.make(0);
      const startedAt = Date.now();

      const details: BashStreamDetails = {
        command: params.command,
        runInBackground: false,
        timeoutSeconds: params.timeout ?? undefined,
        streaming: true,
        exitCode: undefined,
        durationMs: undefined,
        outputBytes: undefined,
        truncatedOutput: false,
        stream: Stream.empty,
      };

      const toTextBlock = (chunk: Uint8Array) => ({
        type: "text" as const,
        text: Buffer.from(chunk).toString("utf-8"),
      });

      const stream = Stream.unwrapScoped(
        Effect.gen(function* () {
          const process = yield* executor.start(cmd).pipe(
            Effect.mapError((e) => new ToolExecutionError("aborted", String(e))),
          );

          const trackBytes = (chunk: Uint8Array) => Ref.update(bytesRef, (n) => n + chunk.length);

          const stdout = process.stdout.pipe(
            Stream.mapError((e) => new ToolExecutionError("command_failed", String(e))),
            Stream.tap(trackBytes),
            Stream.map(toTextBlock),
          );

          const stderr = process.stderr.pipe(
            Stream.mapError((e) => new ToolExecutionError("command_failed", String(e))),
            Stream.tap(trackBytes),
            Stream.map(toTextBlock),
          );

          const output = Stream.merge(stdout, stderr);

          const exitEffect = (() => {
            const baseExit = process.exitCode.pipe(
              Effect.mapError((e) => new ToolExecutionError("command_failed", String(e))),
            );

            const withTimeout =
              params.timeout && params.timeout > 0
                ? Effect.timeoutFail(baseExit, {
                    duration: params.timeout * 1000,
                    onTimeout: () =>
                      new ToolExecutionError("aborted", `Command timed out after ${params.timeout} seconds`),
                  })
                : baseExit;

            return withTimeout.pipe(
              Effect.tapError(() =>
                process.kill("SIGKILL").pipe(
                  Effect.catchAll(() => Effect.void),
                ),
              ),
              Effect.map(normalizeExitCode),
              Effect.tap((code) =>
                Ref.get(bytesRef).pipe(
                  Effect.tap((bytes) =>
                    Effect.sync(() => {
                      details.exitCode = code;
                      details.durationMs = Date.now() - startedAt;
                      details.outputBytes = bytes;
                      details.truncatedOutput = false;
                    }),
                  ),
                ),
              ),
              Effect.map((code) => ({
                type: "text" as const,
                text: `exit ${code}\n`,
              })),
            );
          })();

          const exitChunk = Stream.fromEffect(exitEffect);

          return Stream.concat(output, exitChunk);
        }),
      );

      details.stream = stream;

      return {
        content: [],
        details,
      };
    }),
};

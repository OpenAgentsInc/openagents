import { Effect, Layer } from "effect";
import * as Context from "effect/Context";
import { ChildProcess, ChildProcessSpawner } from "effect/unstable/process";

import { GitExecutionError } from "./errors.js";

export interface GitCloneInput {
  readonly url: string;
  readonly directory?: string;
}

export interface GitRunnerInterface {
  readonly clone: (input: GitCloneInput) => Effect.Effect<void, GitExecutionError>;
}

export class GitRunner extends Context.Service<GitRunner, GitRunnerInterface>()(
  "@openagentsinc/cli/GitRunner",
) {}

export const gitCloneArgv = (input: GitCloneInput): ReadonlyArray<string> => [
  "clone",
  "--",
  input.url,
  ...(input.directory === undefined ? [] : [input.directory]),
];

export const gitRunnerLayer = Layer.effect(
  GitRunner,
  Effect.gen(function* () {
    const spawner = yield* ChildProcessSpawner.ChildProcessSpawner;
    const clone = Effect.fn("GitRunner.clone")(function* (input: GitCloneInput) {
      const args = gitCloneArgv(input);
      const command = ChildProcess.make("git", args, {
        shell: false,
        stdin: "inherit",
        stdout: "inherit",
        stderr: "inherit",
      });
      const exitCode = yield* spawner.exitCode(command).pipe(
        Effect.mapError(
          (cause) =>
            new GitExecutionError({
              operation: "git clone",
              message: "The CLI could not start git clone.",
              cause,
            }),
        ),
      );
      const numericExitCode = Number(exitCode);
      if (numericExitCode !== 0) {
        return yield* new GitExecutionError({
          operation: "git clone",
          exitCode: numericExitCode,
          message: `git clone exited with status ${numericExitCode}.`,
        });
      }
    });
    return GitRunner.of({ clone });
  }),
);

export const gitRunnerTestLayer = (clone: GitRunnerInterface["clone"]): Layer.Layer<GitRunner> =>
  Layer.succeed(GitRunner, GitRunner.of({ clone }));

import { Effect, Layer, Stream } from "effect";
import * as Context from "effect/Context";
import { ChildProcess, ChildProcessSpawner } from "effect/unstable/process";

import { GitExecutionError, InputError } from "./errors.js";
import { credentialHelperCommand } from "./git-credential-helper.js";

export interface GitCloneInput {
  readonly url: string;
  readonly directory?: string;
}

export interface GitRemoteInput {
  readonly origin: string;
  readonly url: string;
  readonly directory: string;
  readonly remote: string;
}

export interface GitRemoteResult {
  readonly remote: string;
  readonly nextPushArguments: ReadonlyArray<string>;
}

export interface GitRunnerInterface {
  readonly clone: (input: GitCloneInput) => Effect.Effect<void, GitExecutionError>;
  readonly attachRemote: (
    input: GitRemoteInput,
  ) => Effect.Effect<GitRemoteResult, GitExecutionError | InputError>;
  readonly inferRepository: (
    origin: string,
    directory?: string,
  ) => Effect.Effect<string, GitExecutionError | InputError>;
  readonly configureCredentialHelper: (
    origin: string,
    scope: "local" | "global",
  ) => Effect.Effect<void, GitExecutionError>;
}

export class GitRunner extends Context.Service<GitRunner, GitRunnerInterface>()(
  "@openagentsinc/cli/GitRunner",
) {}

export const gitCloneArgv = (input: GitCloneInput): ReadonlyArray<string> => [
  "-c",
  `credential.${new URL(input.url).origin}.helper=${credentialHelperCommand(new URL(input.url).origin)}`,
  "clone",
  "--",
  input.url,
  ...(input.directory === undefined ? [] : [input.directory]),
];

const remoteNamePattern = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/;

export const validateRemoteName = Effect.fn("GitRunner.validateRemoteName")(function* (
  remote: string,
) {
  if (
    !remoteNamePattern.test(remote) ||
    remote.includes("..") ||
    remote.endsWith(".") ||
    remote.endsWith(".lock")
  ) {
    return yield* new InputError({ message: `Invalid Git remote name: ${remote}` });
  }
  return remote;
});

export const repositoryFromRemoteUrl = Effect.fn("GitRunner.repositoryFromRemoteUrl")(function* (
  origin: string,
  remoteUrl: string,
) {
  const endpoint = new URL(origin);
  const remote = yield* Effect.try({
    try: () => new URL(remoteUrl),
    catch: () =>
      new InputError({
        message: "The origin remote is not an admitted OpenAgents repository URL.",
      }),
  });
  const parts = remote.pathname.split("/");
  const owner = parts[2];
  const repositoryWithSuffix = parts[3];
  if (
    remote.origin !== endpoint.origin ||
    remote.username !== "" ||
    remote.password !== "" ||
    remote.search !== "" ||
    remote.hash !== "" ||
    parts.length !== 4 ||
    parts[1] !== "git" ||
    owner === undefined ||
    repositoryWithSuffix === undefined ||
    !repositoryWithSuffix.endsWith(".git")
  ) {
    return yield* new InputError({
      message: "The origin remote is not an admitted OpenAgents repository URL.",
    });
  }
  const repository = repositoryWithSuffix.slice(0, -4);
  const [decodedOwner, decodedRepository] = yield* Effect.try({
    try: () => [decodeURIComponent(owner), decodeURIComponent(repository)] as const,
    catch: () =>
      new InputError({
        message: "The origin remote is not an admitted OpenAgents repository URL.",
      }),
  });
  if (
    decodedOwner.includes("/") ||
    decodedRepository.includes("/") ||
    decodedOwner.length === 0 ||
    decodedRepository.length === 0
  ) {
    return yield* new InputError({
      message: "The origin remote is not an admitted OpenAgents repository URL.",
    });
  }
  return `${decodedOwner}/${decodedRepository}`;
});

export const gitRunnerLayer = Layer.effect(
  GitRunner,
  Effect.gen(function* () {
    const spawner = yield* ChildProcessSpawner.ChildProcessSpawner;
    const runGit = Effect.fn("GitRunner.runGit")(function* (
      operation: string,
      args: ReadonlyArray<string>,
      directory?: string,
    ) {
      const command = ChildProcess.make("git", args, {
        ...(directory === undefined ? {} : { cwd: directory }),
        shell: false,
        stdin: "ignore",
        stdout: "pipe",
        stderr: "ignore",
      });
      return yield* Effect.scoped(
        Effect.gen(function* () {
          const handle = yield* spawner.spawn(command).pipe(
            Effect.mapError(
              (cause) =>
                new GitExecutionError({
                  operation,
                  message: `The CLI could not start ${operation}.`,
                  cause,
                }),
            ),
          );
          const decoder = new TextDecoder();
          let stdout = "";
          yield* Stream.runForEach(handle.stdout, (chunk) =>
            Effect.sync(() => {
              if (stdout.length < 16_384) stdout += decoder.decode(chunk, { stream: true });
            }),
          ).pipe(
            Effect.mapError(
              (cause) =>
                new GitExecutionError({
                  operation,
                  message: `The CLI could not read ${operation} output.`,
                  cause,
                }),
            ),
          );
          const exitCode = yield* handle.exitCode.pipe(
            Effect.mapError(
              (cause) =>
                new GitExecutionError({
                  operation,
                  message: `The CLI could not wait for ${operation}.`,
                  cause,
                }),
            ),
          );
          return { exitCode: Number(exitCode), stdout: stdout.slice(0, 16_384).trim() };
        }),
      );
    });

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

    const attachRemote = Effect.fn("GitRunner.attachRemote")(function* (input: GitRemoteInput) {
      const remote = yield* validateRemoteName(input.remote);
      yield* repositoryFromRemoteUrl(input.origin, input.url);

      const worktree = yield* runGit(
        "git worktree validation",
        ["rev-parse", "--is-inside-work-tree"],
        input.directory,
      );
      if (worktree.exitCode !== 0 || worktree.stdout !== "true") {
        return yield* new InputError({
          message: `${input.directory} is not a Git worktree. The repository was created remotely.`,
        });
      }

      const existing = yield* runGit(
        "git remote lookup",
        ["remote", "get-url", "--", remote],
        input.directory,
      );
      if (existing.exitCode === 0 && existing.stdout !== input.url) {
        return yield* new InputError({
          message: `Remote ${remote} already points to ${existing.stdout}. The CLI did not overwrite it.`,
        });
      }
      if (existing.exitCode !== 0) {
        const added = yield* runGit(
          "git remote add",
          ["remote", "add", remote, input.url],
          input.directory,
        );
        if (added.exitCode !== 0) {
          return yield* new GitExecutionError({
            operation: "git remote add",
            exitCode: added.exitCode,
            message: `git remote add exited with status ${added.exitCode}.`,
          });
        }
      }
      return { remote, nextPushArguments: ["push", "-u", remote, "HEAD"] };
    });

    const inferRepository = Effect.fn("GitRunner.inferRepository")(function* (
      origin: string,
      directory = ".",
    ) {
      const existing = yield* runGit(
        "git origin lookup",
        ["remote", "get-url", "--", "origin"],
        directory,
      );
      if (existing.exitCode !== 0 || existing.stdout.length === 0) {
        return yield* new InputError({
          message: "Pass OWNER/REPO or configure an admitted OpenAgents origin remote.",
        });
      }
      return yield* repositoryFromRemoteUrl(origin, existing.stdout);
    });

    const configureCredentialHelper = Effect.fn("GitRunner.configureCredentialHelper")(function* (
      origin: string,
      scope: "local" | "global",
    ) {
      const args = [
        "config",
        scope === "local" ? "--local" : "--global",
        `credential.${origin}.helper`,
        credentialHelperCommand(origin),
      ];
      const command = ChildProcess.make("git", args, {
        shell: false,
        stdin: "ignore",
        stdout: "ignore",
        stderr: "inherit",
      });
      const exitCode = yield* spawner.exitCode(command).pipe(
        Effect.mapError(
          (cause) =>
            new GitExecutionError({
              operation: "git credential helper configuration",
              message: "The CLI could not start git config.",
              cause,
            }),
        ),
      );
      if (Number(exitCode) !== 0) {
        return yield* new GitExecutionError({
          operation: "git credential helper configuration",
          exitCode: Number(exitCode),
          message: `git config exited with status ${Number(exitCode)}.`,
        });
      }
    });
    return GitRunner.of({ clone, attachRemote, inferRepository, configureCredentialHelper });
  }),
);

export const gitRunnerTestLayer = (clone: GitRunnerInterface["clone"]): Layer.Layer<GitRunner> =>
  Layer.succeed(
    GitRunner,
    GitRunner.of({
      clone,
      attachRemote: (input) =>
        Effect.succeed({
          remote: input.remote,
          nextPushArguments: ["push", "-u", input.remote, "HEAD"],
        }),
      inferRepository: () => Effect.succeed("octavia/project"),
      configureCredentialHelper: () => Effect.void,
    }),
  );

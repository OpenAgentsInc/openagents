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

export interface GitCredentialHelperState {
  readonly local: boolean;
  readonly global: boolean;
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
    directory?: string,
  ) => Effect.Effect<void, GitExecutionError>;
  readonly credentialHelperState: (
    origin: string,
  ) => Effect.Effect<GitCredentialHelperState, GitExecutionError>;
}

export class GitRunner extends Context.Service<GitRunner, GitRunnerInterface>()(
  "@openagentsinc/cli/GitRunner",
) {}

export const gitCloneArgv = (input: GitCloneInput): ReadonlyArray<string> => [
  "-c",
  "credential.helper=",
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
        message: "That Git remote URL is not an admitted OpenAgents repository URL.",
      }),
  });
  const parts = remote.pathname.split("/");
  const owner = parts[1];
  const repositoryWithSuffix = parts[2];
  if (
    remote.origin !== endpoint.origin ||
    remote.username !== "" ||
    remote.password !== "" ||
    remote.search !== "" ||
    remote.hash !== "" ||
    parts.length !== 3 ||
    owner === undefined ||
    repositoryWithSuffix === undefined ||
    !repositoryWithSuffix.endsWith(".git")
  ) {
    return yield* new InputError({
      message: "That Git remote URL is not an admitted OpenAgents repository URL.",
    });
  }
  const repository = repositoryWithSuffix.slice(0, -4);
  const [decodedOwner, decodedRepository] = yield* Effect.try({
    try: () => [decodeURIComponent(owner), decodeURIComponent(repository)] as const,
    catch: () =>
      new InputError({
        message: "That Git remote URL is not an admitted OpenAgents repository URL.",
      }),
  });
  if (
    decodedOwner.includes("/") ||
    decodedRepository.includes("/") ||
    decodedOwner.length === 0 ||
    decodedRepository.length === 0
  ) {
    return yield* new InputError({
      message: "That Git remote URL is not an admitted OpenAgents repository URL.",
    });
  }
  return `${decodedOwner}/${decodedRepository}`;
});

export interface GitRemoteUrl {
  readonly name: string;
  readonly url: string;
}

/**
 * Reads `git remote -v` into one URL per remote.
 *
 * Each remote prints a fetch line and a push line. The fetch URL is the one a
 * repository is read from, so it wins; a remote configured for push alone
 * still contributes the URL it has.
 */
export const parseGitRemotes = (output: string): ReadonlyArray<GitRemoteUrl> => {
  const urls = new Map<string, string>();
  for (const rawLine of output.split("\n")) {
    const match = /^(\S+)\s+(.+)\s+\((fetch|push)\)$/u.exec(rawLine.trim());
    if (match === null) continue;
    const name = match[1];
    const url = match[2];
    if (name === undefined || url === undefined) continue;
    if (match[3] === "fetch" || !urls.has(name)) urls.set(name, url);
  }
  return [...urls].map(([name, url]) => ({ name, url }));
};

/**
 * Remote names tried first, in order.
 *
 * A name never admits a remote; only its URL does. This order decides between
 * remotes that already point at the API origin in use. `origin` comes first so
 * a checkout that resolved before this rule existed still resolves to the same
 * repository, then the forge name this project's own documentation uses. Any
 * other remote follows in the order `git remote -v` printed it, which git
 * emits by name, so the same checkout always answers the same way.
 */
const PREFERRED_REMOTE_NAMES: ReadonlyArray<string> = ["origin", "openagents"];

export const orderedRemoteNames = (names: ReadonlyArray<string>): ReadonlyArray<string> => [
  ...PREFERRED_REMOTE_NAMES.filter((preferred) => names.includes(preferred)),
  ...names.filter((name) => !PREFERRED_REMOTE_NAMES.includes(name)),
];

/** Why one remote is not the repository, in the words a reader can act on. */
const remoteRejection = (origin: string, url: string): string => {
  let remote: URL;
  try {
    remote = new URL(url);
  } catch {
    return "is not an HTTP URL";
  }
  if (remote.protocol !== "http:" && remote.protocol !== "https:") return "is not an HTTP URL";
  if (remote.origin !== origin) return `points at ${remote.origin}`;
  return "is not an OWNER/REPO.git path on that origin";
};

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

    /**
     * The repository this checkout belongs to, taken from its Git remotes.
     *
     * A remote's name is a local convention: this project's own contract names
     * the forge remote `openagents` and reserves `origin` for the GitHub
     * mirror, while other checkouts name the forge `origin`. The URL is the
     * fact, so a remote is admitted only when its URL is a repository URL on
     * the API origin this invocation is already talking to. A mirror is
     * therefore never inferred, whatever it is called.
     */
    const inferRepository = Effect.fn("GitRunner.inferRepository")(function* (
      origin: string,
      directory = ".",
    ) {
      const listed = yield* runGit("git remote lookup", ["remote", "-v"], directory);
      if (listed.exitCode !== 0) {
        return yield* new InputError({
          message: `The CLI could not read the Git remotes of ${directory}. Pass OWNER/REPO instead.`,
        });
      }

      const remotes = parseGitRemotes(listed.stdout);
      if (remotes.length === 0) {
        return yield* new InputError({
          message: `This checkout has no Git remotes. Pass OWNER/REPO, or add a remote for ${origin}.`,
        });
      }

      const byName = new Map(remotes.map((remote) => [remote.name, remote.url]));
      const rejected: Array<string> = [];
      for (const name of orderedRemoteNames([...byName.keys()])) {
        const url = byName.get(name);
        if (url === undefined) continue;
        const repository = yield* repositoryFromRemoteUrl(origin, url).pipe(
          Effect.orElseSucceed((): string | undefined => undefined),
        );
        if (repository !== undefined) return repository;
        rejected.push(`${name} ${remoteRejection(origin, url)}`);
      }

      return yield* new InputError({
        message:
          `No Git remote of this checkout is a repository on ${origin}, the OpenAgents origin in use: ` +
          `${rejected.join("; ")}. A remote's name does not decide this; its URL does. ` +
          "Pass OWNER/REPO instead.",
      });
    });

    const configureCredentialHelper = Effect.fn("GitRunner.configureCredentialHelper")(function* (
      origin: string,
      scope: "local" | "global",
      directory?: string,
    ) {
      const scopeFlag = scope === "local" ? "--local" : "--global";
      const key = `credential.${origin}.helper`;
      const reset = yield* runGit(
        "Git credential helper reset",
        ["config", scopeFlag, "--replace-all", key, ""],
        directory,
      );
      if (reset.exitCode !== 0) {
        return yield* new GitExecutionError({
          operation: "git credential helper configuration",
          exitCode: reset.exitCode,
          message: `git config exited with status ${reset.exitCode}.`,
        });
      }
      const configured = yield* runGit(
        "Git credential helper configuration",
        ["config", scopeFlag, "--add", key, credentialHelperCommand(origin)],
        directory,
      );
      if (configured.exitCode !== 0) {
        return yield* new GitExecutionError({
          operation: "git credential helper configuration",
          exitCode: configured.exitCode,
          message: `git config exited with status ${configured.exitCode}.`,
        });
      }
    });

    const credentialHelperState = Effect.fn("GitRunner.credentialHelperState")(function* (
      origin: string,
    ) {
      const expected = credentialHelperCommand(origin);
      const configured = (output: string) => output.split("\n").includes(expected);
      const local = yield* runGit("local Git credential helper lookup", [
        "config",
        "--local",
        "--get-all",
        `credential.${origin}.helper`,
      ]);
      const global = yield* runGit("global Git credential helper lookup", [
        "config",
        "--global",
        "--get-all",
        `credential.${origin}.helper`,
      ]);
      return {
        local: local.exitCode === 0 && configured(local.stdout),
        global: global.exitCode === 0 && configured(global.stdout),
      };
    });
    return GitRunner.of({
      clone,
      attachRemote,
      inferRepository,
      configureCredentialHelper,
      credentialHelperState,
    });
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
      credentialHelperState: () => Effect.succeed({ local: false, global: false }),
    }),
  );

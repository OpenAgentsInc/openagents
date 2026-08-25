import { Effect, Redacted } from "effect";
import { execFile, spawn } from "node:child_process";
import { chmodSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

import { GitExecutionError, InputError } from "./errors.js";
import { repositoryFromRemoteUrl, validateRemoteName } from "./git-runner.js";
import { scrubbedEnvironment } from "./computer-executor.js";

export interface ForgeCredentials {
  readonly token: Redacted.Redacted<string>;
  readonly repository: string;
  readonly branch: string;
}

export interface DelegatedPushInput {
  readonly directory: string;
  readonly remote: string;
  readonly refspec: string;
  readonly repository: string;
  readonly branch: string;
  readonly credential: Redacted.Redacted<string>;
  readonly origin: string;
}

export const redactSecret = (value: string): string =>
  value
    .replaceAll(
      /(?:oa_(?:pat|agent|assignment)_[A-Za-z0-9._-]+|smct_[A-Za-z0-9._-]+)/gu,
      "[REDACTED]",
    )
    .replaceAll(/Bearer\s+\S+/giu, "Bearer [REDACTED]")
    .replaceAll(
      /(?:api[-_]?key|token|secret|password|authorization)\s*[=:]\s*\S+/giu,
      "[REDACTED]",
    );

const canonicalBranch = (branch: string): string =>
  branch.startsWith("refs/heads/") ? branch : `refs/heads/${branch}`;

const toCanonical = (value: string, branch: string): string | undefined => {
  const target = canonicalBranch(branch);
  if (value === branch || value === target) return target;
  return undefined;
};

export const validateRefspec = Effect.fn("DelegationPush.validateRefspec")(function* (
  refspec: string,
  branch: string,
) {
  if (refspec === "") {
    return yield* new InputError({ message: "The refspec is empty." });
  }
  if (/[\s,]/u.test(refspec)) {
    return yield* new InputError({
      message: `Multi-ref push is not allowed: ${redactSecret(refspec)}`,
    });
  }
  if (refspec.startsWith("+") || refspec.startsWith("-")) {
    return yield* new InputError({
      message: `Force or option refspecs are not allowed: ${redactSecret(refspec)}`,
    });
  }
  const target = canonicalBranch(branch);
  const colon = refspec.indexOf(":");
  if (colon >= 0) {
    const src = refspec.slice(0, colon);
    const dst = refspec.slice(colon + 1);
    if (dst === "") {
      return yield* new InputError({
        message: `A refspec with an empty destination is not allowed: ${redactSecret(refspec)}`,
      });
    }
    const srcCanonical = toCanonical(src, branch);
    const dstCanonical = toCanonical(dst, branch);
    if (srcCanonical === undefined || dstCanonical === undefined || srcCanonical !== dstCanonical) {
      return yield* new InputError({
        message: `Refspec ${redactSecret(refspec)} is not the assigned branch ${target}.`,
      });
    }
    return;
  }
  if (toCanonical(refspec, branch) === undefined) {
    return yield* new InputError({
      message: `Refspec ${redactSecret(refspec)} is not the assigned branch ${target}.`,
    });
  }
});

const getRemoteUrl = (
  directory: string,
  remote: string,
): Effect.Effect<string, GitExecutionError> =>
  Effect.tryPromise({
    try: () =>
      new Promise<string>((resolve, reject) => {
        execFile(
          "git",
          ["remote", "get-url", "--", remote],
          { cwd: directory },
          (error, stdout, stderr) => {
            if (error) {
              reject(
                new Error(
                  `git remote get-url failed: ${redactSecret(stderr.trim() || String(error))}`,
                ),
              );
              return;
            }
            resolve(stdout.trim());
          },
        );
      }),
    catch: (cause) =>
      new GitExecutionError({
        operation: "git remote get-url",
        message: redactSecret(cause instanceof Error ? cause.message : String(cause)),
      }),
  });

const runGit = (
  operation: string,
  args: ReadonlyArray<string>,
  directory: string,
  env: Record<string, string>,
): Effect.Effect<{ readonly exitCode: number; readonly stderr: string }, GitExecutionError> =>
  Effect.tryPromise({
    try: () =>
      new Promise<{ exitCode: number; stderr: string }>((resolve, reject) => {
        let stderr = "";
        const child = spawn("git", [...args], {
          cwd: directory,
          env,
          shell: false,
          stdio: ["ignore", "pipe", "pipe"],
        });
        child.on("error", (cause) =>
          reject(new Error(`git ${operation} could not start: ${redactSecret(cause.message)}`)),
        );
        child.stderr.on("data", (chunk: Buffer) => {
          if (stderr.length < 16_384) {
            stderr += chunk.toString("utf8");
          }
        });
        child.on("close", (code) =>
          resolve({
            exitCode: code ?? 1,
            stderr: redactSecret(stderr.trim()),
          }),
        );
      }),
    catch: (cause) =>
      new GitExecutionError({
        operation: `git ${operation}`,
        message: redactSecret(cause instanceof Error ? cause.message : String(cause)),
      }),
  });

const buildCredentialHelper = (
  directory: string,
  token: string,
  expectedHost: string,
  expectedPath: string,
): string => {
  const tokenPath = join(directory, "token");
  const helperPath = join(directory, "helper");
  writeFileSync(tokenPath, `${token}\n`, { mode: 0o600 });
  chmodSync(tokenPath, 0o600);

  const script = `#!/bin/sh
if [ "$1" != "get" ]; then
  exit 0
fi
host=""
path=""
while IFS= read -r line; do
  [ -z "$line" ] && break
  case "$line" in
    host=*) host="\${line#host=}" ;;
    path=*) path="\${line#path=}" ;;
  esac
done
if [ "$host" != ${JSON.stringify(expectedHost)} ] || [ "$path" != ${JSON.stringify(expectedPath)} ]; then
  exit 0
fi
PASSWORD=$(tr -d '\\n' < ${JSON.stringify(tokenPath)})
printf 'username=openagents\\npassword=%s\\n\\n' "$PASSWORD"
`;
  writeFileSync(helperPath, script, { mode: 0o700 });
  chmodSync(helperPath, 0o700);
  return helperPath;
};

export const pushDelegated = (
  input: DelegatedPushInput,
): Effect.Effect<void, GitExecutionError | InputError> =>
  Effect.tryPromise({
    try: async () => {
      await Effect.runPromise(validateRefspec(input.refspec, input.branch));

      const remote = await Effect.runPromise(validateRemoteName(input.remote));
      const rawUrl = await Effect.runPromise(getRemoteUrl(input.directory, remote));
      const actualRepository = await Effect.runPromise(
        repositoryFromRemoteUrl(input.origin, rawUrl),
      );
      if (actualRepository !== input.repository) {
        throw new InputError({
          message: `The remote repository is ${actualRepository}, not the assigned ${input.repository}.`,
        });
      }

      const token = Redacted.value(input.credential);
      const tempDir = mkdtempSync(join(tmpdir(), "oa-delegation-push-"));
      chmodSync(tempDir, 0o700);

      try {
        const remoteUrl = new URL(rawUrl);
        const helperPath = buildCredentialHelper(
          tempDir,
          token,
          remoteUrl.host,
          remoteUrl.pathname.replace(/^\//u, ""),
        );
        const helperConfig = `!${helperPath}`;
        const gitArgs = [
          "-c",
          "credential.helper=",
          "-c",
          `credential.${remoteUrl.origin}.helper=${helperConfig}`,
          "push",
          "--",
          remote,
          input.refspec,
        ];
        const result = await Effect.runPromise(
          runGit("push", gitArgs, input.directory, {
            ...scrubbedEnvironment(process.env),
            GIT_TERMINAL_PROMPT: "0",
          }),
        );
        if (result.exitCode !== 0) {
          throw new GitExecutionError({
            operation: "delegated git push",
            exitCode: result.exitCode,
            message: `git push failed: ${result.stderr}`,
          });
        }
      } finally {
        try {
          rmSync(tempDir, { recursive: true, force: true });
        } catch {
          // Cleanup is best-effort; the token file is already as protected as the
          // temporary directory allows.
        }
      }
    },
    catch: (cause) => {
      if (cause instanceof InputError) return cause;
      if (cause instanceof GitExecutionError) return cause;
      return new GitExecutionError({
        operation: "delegated git push",
        message: redactSecret(String(cause)),
      });
    },
  });

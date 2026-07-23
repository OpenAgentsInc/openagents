import { exec } from "node:child_process";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, isAbsolute, join } from "node:path";
import { Effect } from "effect";
import type { HarnessBootstrap } from "./bootstrap.ts";
import {
  HarnessSandboxError,
  type HarnessSandboxProvider,
  type HarnessSandboxRunResult,
  type HarnessSandboxSession,
} from "./sandbox.ts";

/**
 * A REAL local sandbox provider backed by the host filesystem and
 * `child_process`, distinct from the in-memory reference double. It composes a
 * per-session working directory under a base, materializes bootstrap files,
 * runs bootstrap commands and `run` requests as real host processes, and does
 * genuine file I/O. This is the HARN-07 owner-local provider — the cheap
 * isolation rung (a per-session directory, and per-run a git worktree in the
 * desktop wiring). It deliberately omits `getPortUrl` (no port infrastructure),
 * exercising the fail-closed capability-absence path.
 *
 * Safety: paths are resolved beneath the session working directory; an absolute
 * or parent-escaping path is refused. This is an owner-local convenience
 * provider, NOT a multi-tenant isolation boundary — that is the managed-sandbox
 * provider's job.
 */
export interface LocalProcessSandboxProviderOptions {
  /** Absolute base directory under which each session gets `<base>/<sessionId>`. */
  readonly baseDir: string;
}

const runCommand = (
  command: string,
  cwd: string,
  env?: Record<string, string | undefined>,
): Promise<HarnessSandboxRunResult> =>
  new Promise((resolve) => {
    exec(command, { cwd, env }, (error, stdout, stderr) => {
      const exitCode = error === null ? 0 : typeof error.code === "number" ? error.code : 1;
      resolve({ stdout: stdout.toString(), stderr: stderr.toString(), exitCode });
    });
  });

const resolveBeneath = (root: string, path: string): string => {
  if (isAbsolute(path) || path.split("/").includes("..")) {
    throw new Error(`path escapes the session workspace: ${path}`);
  }
  return join(root, path);
};

export const makeLocalProcessSandboxProvider = (
  providerOptions: LocalProcessSandboxProviderOptions,
): HarnessSandboxProvider => {
  const seenIdentities = new Set<string>();
  const sessions = new Map<string, Readonly<Record<string, string>>>();

  const sandboxError = (operation: string, cause: unknown) =>
    new HarnessSandboxError({
      operation,
      detail: cause instanceof Error ? cause.message : String(cause),
      cause,
    });

  const makeSession = (
    sessionId: string,
    workingDirectory: string,
    sessionEnv: Readonly<Record<string, string>>,
  ): HarnessSandboxSession => ({
    id: sessionId,
    workingDirectory,
    env: sessionEnv,
    writeTextFile: ({ path, content }) =>
      Effect.tryPromise({
        try: async () => {
          const target = resolveBeneath(workingDirectory, path);
          await mkdir(dirname(target), { recursive: true });
          await writeFile(target, content, "utf8");
        },
        catch: (cause) => sandboxError("writeTextFile", cause),
      }),
    readTextFile: ({ path }) =>
      Effect.tryPromise({
        try: () => readFile(resolveBeneath(workingDirectory, path), "utf8"),
        catch: (cause) => sandboxError("readTextFile", cause),
      }),
    run: ({ command, cwd, env: runEnv }) =>
      Effect.tryPromise({
        try: () =>
          runCommand(
            command,
            cwd === undefined ? workingDirectory : resolveBeneath(workingDirectory, cwd),
            {
              ...process.env,
              ...sessionEnv,
              ...(runEnv ?? {}),
            },
          ),
        catch: (cause) => sandboxError("run", cause),
      }),
    // No port infrastructure: `getPortUrl` is intentionally omitted so callers
    // treat this provider as port-incapable (fail-closed capability absence).
    stop: () =>
      // Framework-owned teardown removes the session directory. The adapter
      // never calls this.
      Effect.tryPromise({
        try: async () => {
          await rm(workingDirectory, { recursive: true, force: true });
          sessions.delete(sessionId);
        },
        catch: (cause) => sandboxError("stop", cause),
      }),
  });

  const applyBootstrap = (session: HarnessSandboxSession, bootstrap: HarnessBootstrap) =>
    Effect.gen(function* () {
      for (const file of bootstrap.files ?? []) {
        yield* session.writeTextFile({ path: file.path, content: file.content });
      }
      for (const command of bootstrap.commands ?? []) {
        yield* session.run({
          command: command.command,
          ...(command.cwd === undefined ? {} : { cwd: command.cwd }),
        });
      }
    });

  return {
    specificationVersion: "harness-sandbox-v1",
    providerId: "local-process",
    createSession: (options) =>
      Effect.gen(function* () {
        const workingDirectory = join(providerOptions.baseDir, options.sessionId);
        yield* Effect.tryPromise({
          try: () => mkdir(workingDirectory, { recursive: true }),
          catch: (cause) => sandboxError("createSession", cause),
        });

        // Compute active environment variables
        let baseEnv: Readonly<Record<string, string>> = {};
        if (options.environmentId !== undefined && options.environments !== undefined) {
          const found = options.environments.find((e) => e.id === options.environmentId);
          if (found !== undefined) {
            baseEnv = found.env;
          }
        }
        const env = {
          ...baseEnv,
          ...(options.env ?? {}),
        };
        sessions.set(options.sessionId, env);

        const session = makeSession(options.sessionId, workingDirectory, env);

        const identityKey = options.identity ?? options.sessionId;
        const firstForIdentity = !seenIdentities.has(identityKey);
        if (firstForIdentity) {
          seenIdentities.add(identityKey);
          if (options.bootstrap !== undefined) {
            yield* applyBootstrap(session, options.bootstrap);
          }
          if (options.onFirstCreate !== undefined) {
            yield* options.onFirstCreate(session);
          }
        }
        return session;
      }),
    resumeSession: ({ sessionId }) =>
      Effect.sync(() => {
        const env = sessions.get(sessionId) ?? {};
        return makeSession(sessionId, join(providerOptions.baseDir, sessionId), env);
      }),
  };
};

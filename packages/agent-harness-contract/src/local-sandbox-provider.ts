import { Effect } from "effect";
import type { HarnessBootstrapFile } from "./bootstrap.ts";
import {
  HarnessSandboxError,
  type HarnessSandboxProvider,
  type HarnessSandboxRunResult,
  type HarnessSandboxSession,
} from "./sandbox.ts";

/** A virtual in-memory filesystem for one session workspace: absolute path -> text. */
type Vfs = Map<string, string>;

/** Resolve a possibly-relative path against the session working directory. */
const resolvePath = (workingDirectory: string, path: string): string =>
  path.startsWith("/") ? path : `${workingDirectory}/${path}`;

/** Config for {@link makeLocalSandboxProvider}. */
export interface LocalSandboxProviderConfig {
  readonly providerId?: string;
  /** Base directory the per-session working directory is composed under. */
  readonly baseDirectory?: string;
}

/**
 * An in-memory reference {@link HarnessSandboxProvider} used to exercise the
 * sandbox contract in hermetic conformance tests. It needs no real processes and
 * no real filesystem.
 *
 * This is a TEST DOUBLE, not a real executor. `run` records nothing observable
 * and returns a fixed successful result — the real local provider over
 * `child_process` and the managed-sandbox provider from
 * `@openagentsinc/managed-sandbox-contract` land in the desktop cutover. The
 * double exists only to prove the port shape and the fail-closed capability
 * posture.
 *
 * Capability posture proven here: `getPortUrl` is OMITTED on every session (this
 * double has no ports), so a caller sees `session.getPortUrl === undefined` and
 * treats the port capability as absent. `resumeSession` IS provided and rebinds a
 * previously-created session to the same virtual filesystem.
 */
export const makeLocalSandboxProvider = (
  config: LocalSandboxProviderConfig = {},
): HarnessSandboxProvider => {
  const providerId = config.providerId ?? "local-memory";
  const base = config.baseDirectory ?? "/harness";

  // Provider-scoped state, stable across createSession / resumeSession calls.
  const sessions = new Map<string, Vfs>();
  const firstCreatedIdentities = new Set<string>();

  const buildSession = (sessionId: string, vfs: Vfs): HarnessSandboxSession => {
    const workingDirectory = `${base}/${sessionId}`;

    const writeTextFile = (params: { readonly path: string; readonly content: string }) =>
      Effect.sync(() => {
        vfs.set(resolvePath(workingDirectory, params.path), params.content);
      });

    const readTextFile = (params: { readonly path: string }) =>
      Effect.suspend(() => {
        const key = resolvePath(workingDirectory, params.path);
        const content = vfs.get(key);
        return content === undefined
          ? Effect.fail(
              new HarnessSandboxError({
                operation: "readTextFile",
                detail: `no such file in session workspace: ${key}`,
              }),
            )
          : Effect.succeed(content);
      });

    // Honest test double: the double does not execute anything. It returns a
    // fixed successful result so hermetic tests do not depend on a real process.
    const run = (_params: {
      readonly command: string;
      readonly cwd?: string;
    }): Effect.Effect<HarnessSandboxRunResult, HarnessSandboxError> =>
      Effect.succeed({ stdout: "", stderr: "", exitCode: 0 });

    // Framework-owned teardown. The adapter never calls this.
    const stop = () =>
      Effect.sync(() => {
        sessions.delete(sessionId);
      });

    // NOTE: `getPortUrl` and `setNetworkPolicy` are intentionally OMITTED. This
    // double has no ports and no network-policy control, so callers see them as
    // absent capabilities (the fail-closed posture).
    return {
      id: sessionId,
      workingDirectory,
      writeTextFile,
      readTextFile,
      run,
      stop,
    };
  };

  const applyBootstrapFiles = (
    session: HarnessSandboxSession,
    files: ReadonlyArray<HarnessBootstrapFile>,
  ) =>
    Effect.forEach(files, (file) =>
      session.writeTextFile({ path: file.path, content: file.content }),
    );

  const createSession: HarnessSandboxProvider["createSession"] = (options) =>
    Effect.gen(function* () {
      const wasFresh = !sessions.has(options.sessionId);
      const vfs: Vfs = sessions.get(options.sessionId) ?? new Map<string, string>();
      if (wasFresh) {
        sessions.set(options.sessionId, vfs);
      }
      const session = buildSession(options.sessionId, vfs);

      // Bootstrap + onFirstCreate run exactly once per identity (snapshot reuse).
      // Without an identity, they run on fresh create of the session workspace.
      const runFirstTime =
        options.identity !== undefined ? !firstCreatedIdentities.has(options.identity) : wasFresh;
      if (options.identity !== undefined) {
        firstCreatedIdentities.add(options.identity);
      }

      if (runFirstTime) {
        if (options.bootstrap?.files !== undefined) {
          yield* applyBootstrapFiles(session, options.bootstrap.files);
        }
        if (options.onFirstCreate !== undefined) {
          yield* options.onFirstCreate(session);
        }
      }

      return session;
    });

  const resumeSession: NonNullable<HarnessSandboxProvider["resumeSession"]> = (options) =>
    Effect.suspend(() => {
      const vfs = sessions.get(options.sessionId);
      return vfs === undefined
        ? Effect.fail(
            new HarnessSandboxError({
              operation: "resumeSession",
              detail: `no session to resume: ${options.sessionId}`,
            }),
          )
        : Effect.succeed(buildSession(options.sessionId, vfs));
    });

  return {
    specificationVersion: "harness-sandbox-v1",
    providerId,
    createSession,
    resumeSession,
  };
};

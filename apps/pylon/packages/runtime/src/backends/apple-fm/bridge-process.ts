/**
 * One-shot Apple FM bridge process launcher for the CLI and any caller that
 * wants "start the local Swift Foundation Models bridge, run one bounded thing,
 * stop it" without standing up the long-running heartbeat supervisor that the
 * Pylon node/desktop hosts use (`apps/pylon/src/node/apple-fm-bridge-*`).
 *
 * This lives in the runtime package (not the app `src/node` layer) so the
 * runtime CLI can consume it directly without a reverse dependency on the app.
 * It is deliberately self-contained: helper discovery, free-port selection,
 * spawn, and readiness polling, all behind injectable seams so tests stay
 * deterministic with no real child process, no real socket, and no clock.
 *
 * Two modes:
 *   - ADOPT: if a healthy bridge is already listening on the resolved base URL,
 *     adopt it and never stop it (an operator/desktop-owned bridge is not ours
 *     to kill).
 *   - LAUNCH: otherwise resolve the helper binary, bind a free loopback port,
 *     spawn `foundation-bridge --port <port>`, and poll `/health` until ready or
 *     a typed timeout. The returned handle owns `stop()`.
 *
 * It never reads prompts, file contents, tokens, or model output; the child's
 * stdio is ignored. Only loopback URLs are used.
 */

import { Runtime } from "@openagentsinc/runtime-platform";
import { Effect, Schema as S } from "effect";
import { existsSync } from "node:fs";
import { createServer } from "node:net";
import { dirname, join, resolve } from "node:path";
import { APPLE_FM_DEFAULT_BASE_URL, AppleFmHealthResponse } from "./contract.js";

export const APPLE_FM_BRIDGE_PATH_ENV = "OPENAGENTS_APPLE_FM_BRIDGE_PATH" as const;
export const APPLE_FM_BRIDGE_DEFAULT_PORT = 11435 as const;

export type AppleFmBridgeLaunchFailureClass =
  | "helper_not_found"
  | "unsupported_platform"
  | "spawn_failed"
  | "health_timeout";

export class AppleFmBridgeLaunchError extends S.TaggedErrorClass<AppleFmBridgeLaunchError>()(
  "AppleFmBridgeLaunchError",
  {
    reason: S.String,
    failureClass: S.Literals([
      "helper_not_found",
      "unsupported_platform",
      "spawn_failed",
      "health_timeout",
    ]),
  },
) {}

/** Where the helper binary came from, for diagnostics (never a secret). */
export type AppleFmBridgeHelperSource = "env" | "source-wrapper" | "source-build";

export interface DiscoveredAppleFmBridgeHelper {
  readonly path: string;
  readonly source: AppleFmBridgeHelperSource;
}

/** A running-or-adopted bridge. `stop()` is a no-op for an adopted bridge. */
export interface AppleFmBridgeHandle {
  readonly baseUrl: string;
  readonly adopted: boolean;
  readonly helperPath?: string;
  readonly port?: number;
  readonly stop: () => void;
}

/** Minimal child-process shape the launcher needs; `Runtime.spawn` satisfies it. */
export interface AppleFmBridgeSpawnedProcess {
  readonly kill: (signal?: NodeJS.Signals | number) => void;
  readonly exited: Promise<number>;
}

export interface LaunchAppleFmBridgeOptions {
  /** Explicit helper path; otherwise env + local discovery is used. */
  readonly helperPath?: string;
  /**
   * Base URL to probe/adopt. Defaults to `OPENAGENTS_APPLE_FM_BASE_URL` /
   * `PROBE_APPLE_FM_BASE_URL` / the loopback default. When launching, the port
   * is chosen fresh unless `port` is set, and the returned base URL reflects it.
   */
  readonly baseUrl?: string;
  /** Fixed loopback port to bind; otherwise a free ephemeral port is chosen. */
  readonly port?: number;
  /** Adopt an already-healthy bridge instead of launching one. Default true. */
  readonly adoptIfHealthy?: boolean;
  /** Readiness poll ceiling. Default 15000ms. */
  readonly readinessTimeoutMs?: number;
  /** Readiness poll interval. Default 150ms. */
  readonly readinessIntervalMs?: number;
  readonly env?: Readonly<Record<string, string | undefined>>;
  readonly cwd?: string;
  /** Injectable seams (defaults are the live implementations). */
  readonly fetch?: typeof fetch;
  readonly spawn?: (command: ReadonlyArray<string>) => AppleFmBridgeSpawnedProcess;
  readonly pickFreePort?: () => Promise<number>;
  readonly sleep?: (ms: number) => Promise<void>;
  readonly fileExists?: (path: string) => boolean;
  readonly platform?: NodeJS.Platform;
}

/**
 * Resolve the helper binary path from the explicit env var first, then by
 * walking up from `cwd` for a built `bin/foundation-bridge` wrapper or the
 * SwiftPM release build output. Returns `null` when nothing is found (the
 * expected outcome on a host that has not built the bridge).
 */
export function discoverAppleFmBridgeHelper(options: {
  readonly env?: Readonly<Record<string, string | undefined>>;
  readonly cwd?: string;
  readonly fileExists?: (path: string) => boolean;
} = {}): DiscoveredAppleFmBridgeHelper | null {
  const env = options.env ?? process.env;
  const fileExists = options.fileExists ?? existsSync;

  const explicit = env[APPLE_FM_BRIDGE_PATH_ENV];
  if (explicit !== undefined && explicit.trim().length > 0) {
    const path = resolve(explicit);
    if (fileExists(path)) {
      return { path, source: "env" };
    }
  }

  for (const root of candidatePylonRoots(resolve(options.cwd ?? process.cwd()))) {
    const wrapper = join(root, "bin", "foundation-bridge");
    if (fileExists(wrapper)) {
      return { path: wrapper, source: "source-wrapper" };
    }
    const build = join(root, "swift", "foundation-bridge", ".build", "release", "foundation-bridge");
    if (fileExists(build)) {
      return { path: build, source: "source-build" };
    }
  }

  return null;
}

function candidatePylonRoots(cwd: string): ReadonlyArray<string> {
  const roots: string[] = [];
  const seen = new Set<string>();
  let current = cwd;
  while (true) {
    for (const candidate of [current, join(current, "apps", "pylon")]) {
      if (!seen.has(candidate)) {
        roots.push(candidate);
        seen.add(candidate);
      }
    }
    const next = dirname(current);
    if (next === current) {
      return roots;
    }
    current = next;
  }
}

function resolveBaseUrlPreference(
  options: LaunchAppleFmBridgeOptions,
): string {
  const env = options.env ?? {};
  return (
    options.baseUrl ??
    env.OPENAGENTS_APPLE_FM_BASE_URL ??
    env.PROBE_APPLE_FM_BASE_URL ??
    APPLE_FM_DEFAULT_BASE_URL
  );
}

/** Probe `/health` once; returns `ready` or a non-ready/unreachable verdict. */
function probeHealthOnce(
  baseUrl: string,
  fetchImpl: typeof fetch,
): Effect.Effect<"ready" | "not_ready", never> {
  // The async is total: every failure path (network refusal, non-2xx, malformed
  // body) resolves to "not_ready", so this never enters the error channel.
  return Effect.promise(async () => {
    try {
      const response = await fetchImpl(`${baseUrl.replace(/\/$/u, "")}/health`, {
        method: "GET",
        headers: { accept: "application/json" },
      });
      if (!response.ok) {
        return "not_ready" as const;
      }
      const body: unknown = await response.json();
      const decoded = S.decodeUnknownOption(AppleFmHealthResponse)(body);
      return decoded._tag === "Some" && decoded.value.ready ? ("ready" as const) : ("not_ready" as const);
    } catch {
      return "not_ready" as const;
    }
  });
}

async function defaultPickFreePort(): Promise<number> {
  return await new Promise<number>((resolvePort, rejectPort) => {
    const server = createServer();
    server.on("error", rejectPort);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (address === null || typeof address === "string") {
        server.close();
        rejectPort(new Error("could not resolve a free loopback port"));
        return;
      }
      const { port } = address;
      server.close(() => resolvePort(port));
    });
  });
}

/**
 * Launch (or adopt) the local Apple FM bridge and return a handle whose
 * `baseUrl` is ready for the runtime client. The caller owns `stop()`.
 */
export function launchAppleFmBridge(
  options: LaunchAppleFmBridgeOptions = {},
): Effect.Effect<AppleFmBridgeHandle, AppleFmBridgeLaunchError> {
  return Effect.gen(function* () {
    const fetchImpl = options.fetch ?? fetch;
    const sleep = options.sleep ?? ((ms: number) => new Promise<void>((r) => setTimeout(r, ms)));
    const platform = options.platform ?? process.platform;
    const adoptIfHealthy = options.adoptIfHealthy ?? true;
    const preferredBaseUrl = resolveBaseUrlPreference(options);

    // ADOPT: a healthy operator/desktop-owned bridge is not ours to kill.
    if (adoptIfHealthy) {
      const verdict = yield* probeHealthOnce(preferredBaseUrl, fetchImpl);
      if (verdict === "ready") {
        return { baseUrl: preferredBaseUrl, adopted: true, stop: () => {} };
      }
    }

    // LAUNCH: the helper is a macOS binary; refuse on other platforms with a
    // typed reason rather than a spawn crash (unless the caller injected a path
    // for tests).
    if (platform !== "darwin" && options.helperPath === undefined) {
      return yield* Effect.fail(
        new AppleFmBridgeLaunchError({
          reason: `Apple FM bridge launch is only supported on macOS, not ${platform}.`,
          failureClass: "unsupported_platform",
        }),
      );
    }

    const helperPath =
      options.helperPath ??
      discoverAppleFmBridgeHelper({ env: options.env, cwd: options.cwd, fileExists: options.fileExists })?.path;
    if (helperPath === undefined) {
      return yield* Effect.fail(
        new AppleFmBridgeLaunchError({
          reason:
            `Apple FM bridge helper not found. Build it with ` +
            `\`bash apps/pylon/swift/foundation-bridge/build.sh\` or set ${APPLE_FM_BRIDGE_PATH_ENV}.`,
          failureClass: "helper_not_found",
        }),
      );
    }

    const port =
      options.port ??
      (yield* Effect.tryPromise({
        try: options.pickFreePort ?? defaultPickFreePort,
        catch: (error) =>
          new AppleFmBridgeLaunchError({
            reason: `could not select a free loopback port: ${String(error)}`,
            failureClass: "spawn_failed",
          }),
      }));
    const baseUrl = `http://127.0.0.1:${port}`;

    const spawnImpl = options.spawn ?? ((command: ReadonlyArray<string>) => Runtime.spawn(command, {
      stdout: "ignore",
      stderr: "ignore",
      stdin: "ignore",
    }));
    const child = yield* Effect.try({
      try: () => spawnImpl([helperPath, "--port", String(port)]),
      catch: (error) =>
        new AppleFmBridgeLaunchError({
          reason: `failed to spawn Apple FM bridge helper: ${String(error)}`,
          failureClass: "spawn_failed",
        }),
    });

    const stop = (() => {
      let stopped = false;
      return () => {
        if (stopped) return;
        stopped = true;
        try {
          child.kill();
        } catch {
          // best-effort; the process may already be gone
        }
      };
    })();

    const timeoutMs = options.readinessTimeoutMs ?? 15_000;
    const intervalMs = options.readinessIntervalMs ?? 150;
    const deadline = timeoutMs;
    let waited = 0;
    while (waited <= deadline) {
      const verdict = yield* probeHealthOnce(baseUrl, fetchImpl);
      if (verdict === "ready") {
        return { baseUrl, adopted: false, helperPath, port, stop };
      }
      yield* Effect.promise(() => sleep(intervalMs));
      waited += intervalMs;
    }

    stop();
    return yield* Effect.fail(
      new AppleFmBridgeLaunchError({
        reason: `Apple FM bridge did not report ready health within ${timeoutMs}ms.`,
        failureClass: "health_timeout",
      }),
    );
  });
}

import { Clock, Context, Effect, Fiber, Layer, Ref, Schedule } from "effect";
import fs from "node:fs";
import https from "node:https";
import path from "node:path";

import type { LndBinaryTarget } from "./lndBinaryResolver";
import { LndBinaryResolverError, resolveAndVerifyLndBinary } from "./lndBinaryResolver";
import {
  type LndNeutrinoNetwork,
  materializeLndRuntimeConfig,
  type LndRuntimePaths,
} from "./lndRuntimeConfig";
import {
  LndProcessTransportService,
  type LndProcessExit,
  type LndProcessHandle,
} from "./lndProcessTransport";

export type LndRuntimeLifecycle = "stopped" | "starting" | "running" | "stopping" | "backoff" | "failed";

export type LndRuntimeHealth = "unknown" | "starting" | "healthy" | "unhealthy";

export type LndRuntimeLogLevel = "info" | "warn" | "error";

export type LndRuntimeLogEvent = Readonly<{
  readonly atMs: number;
  readonly level: LndRuntimeLogLevel;
  readonly event: string;
  readonly message: string;
}>;

export type LndRuntimeSyncSnapshot = Readonly<{
  readonly source: "none" | "rest_getinfo";
  readonly blockHeight: number | null;
  readonly numPeers: number | null;
  readonly bestHeaderTimestamp: number | null;
  readonly syncedToChain: boolean | null;
  readonly syncedToGraph: boolean | null;
  readonly walletSynced: boolean | null;
  readonly lastUpdatedAtMs: number | null;
  readonly lastError: string | null;
}>;

export type LndRuntimeStatus = Readonly<{
  readonly lifecycle: LndRuntimeLifecycle;
  readonly health: LndRuntimeHealth;
  readonly target: LndBinaryTarget | null;
  readonly source: "bundled" | "dev_override" | null;
  readonly pid: number | null;
  readonly restartCount: number;
  readonly crashCount: number;
  readonly consecutiveCrashes: number;
  readonly nextRestartAtMs: number | null;
  readonly lastStartedAtMs: number | null;
  readonly lastStoppedAtMs: number | null;
  readonly lastHealthCheckAtMs: number | null;
  readonly lastExitCode: number | null;
  readonly lastExitSignal: NodeJS.Signals | null;
  readonly lastError: string | null;
  readonly configPath: string | null;
  readonly runtimeDir: string | null;
  readonly binaryPath: string | null;
  readonly sync: LndRuntimeSyncSnapshot;
}>;

export type LndRuntimeManagerErrorCode =
  | "binary_resolution_failed"
  | "config_materialization_failed"
  | "spawn_failed"
  | "kill_failed"
  | "restart_backoff_exhausted";

export class LndRuntimeManagerError extends Error {
  readonly code: LndRuntimeManagerErrorCode;

  constructor(code: LndRuntimeManagerErrorCode, message: string) {
    super(message);
    this.name = "LndRuntimeManagerError";
    this.code = code;
  }
}

export type LndRuntimeManagerConfig = Readonly<{
  readonly appPath: string;
  readonly resourcesPath: string;
  readonly userDataPath: string;
  readonly isPackaged: boolean;
  readonly env: NodeJS.ProcessEnv;
  readonly network: LndNeutrinoNetwork;
  readonly alias: string;
  readonly rpcListen: string;
  readonly restListen: string;
  readonly p2pListen: string;
  readonly debugLevel: string;
  readonly neutrinoPeers: ReadonlyArray<string>;
  readonly maxCrashRestarts: number;
  readonly restartBackoffBaseMs: number;
  readonly restartBackoffMaxMs: number;
  readonly healthProbeIntervalMs: number;
  readonly logHistoryLimit: number;
}>;

export const defaultLndRuntimeManagerConfig = (input: {
  readonly appPath: string;
  readonly resourcesPath: string;
  readonly userDataPath: string;
  readonly isPackaged: boolean;
  readonly env: NodeJS.ProcessEnv;
}): LndRuntimeManagerConfig => ({
  appPath: input.appPath,
  resourcesPath: input.resourcesPath,
  userDataPath: input.userDataPath,
  isPackaged: input.isPackaged,
  env: input.env,
  network: "testnet",
  alias: "openagents-desktop",
  rpcListen: "127.0.0.1:10009",
  restListen: "127.0.0.1:8080",
  p2pListen: "0.0.0.0:9735",
  debugLevel: "info",
  neutrinoPeers: [],
  maxCrashRestarts: 3,
  restartBackoffBaseMs: 500,
  restartBackoffMaxMs: 10_000,
  healthProbeIntervalMs: 2_000,
  logHistoryLimit: 200,
});

export class LndRuntimeManagerConfigService extends Context.Tag(
  "@openagents/desktop/LndRuntimeManagerConfigService",
)<LndRuntimeManagerConfigService, LndRuntimeManagerConfig>() {}

export const LndRuntimeManagerConfigLive = (config: LndRuntimeManagerConfig) =>
  Layer.succeed(LndRuntimeManagerConfigService, config);

const runtimeStatusInitial = (): LndRuntimeStatus => ({
  lifecycle: "stopped",
  health: "unknown",
  target: null,
  source: null,
  pid: null,
  restartCount: 0,
  crashCount: 0,
  consecutiveCrashes: 0,
  nextRestartAtMs: null,
  lastStartedAtMs: null,
  lastStoppedAtMs: null,
  lastHealthCheckAtMs: null,
  lastExitCode: null,
  lastExitSignal: null,
  lastError: null,
  configPath: null,
  runtimeDir: null,
  binaryPath: null,
  sync: {
    source: "none",
    blockHeight: null,
    numPeers: null,
    bestHeaderTimestamp: null,
    syncedToChain: null,
    syncedToGraph: null,
    walletSynced: null,
    lastUpdatedAtMs: null,
    lastError: null,
  },
});

const parseRestAddress = (
  value: string,
): Readonly<{
  readonly hostname: string;
  readonly port: number;
}> => {
  const trimmed = value.trim();
  if (!trimmed) {
    throw new Error("restlisten_empty");
  }

  if (trimmed.startsWith("[")) {
    const close = trimmed.indexOf("]");
    if (close === -1) throw new Error("restlisten_invalid_ipv6");
    const hostname = trimmed.slice(1, close).trim();
    const portValue = trimmed.slice(close + 1).trim();
    if (!hostname) throw new Error("restlisten_invalid_ipv6_host");
    if (!portValue.startsWith(":")) throw new Error("restlisten_invalid_ipv6_port");
    const port = Number(portValue.slice(1));
    if (!Number.isInteger(port) || port <= 0 || port > 65_535) {
      throw new Error("restlisten_invalid_port");
    }
    return {
      hostname,
      port,
    };
  }

  const separator = trimmed.lastIndexOf(":");
  if (separator <= 0) throw new Error("restlisten_invalid_host_port");
  const hostname = trimmed.slice(0, separator).trim();
  const port = Number(trimmed.slice(separator + 1));
  if (!hostname) throw new Error("restlisten_invalid_host");
  if (!Number.isInteger(port) || port <= 0 || port > 65_535) {
    throw new Error("restlisten_invalid_port");
  }
  return {
    hostname,
    port,
  };
};

const readMacaroonHex = (macaroonPath: string): string => fs.readFileSync(macaroonPath).toString("hex");

const asNumberFromUnknown = (value: unknown): number | null => {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim().length > 0) {
    const parsed = Number(value.trim());
    if (Number.isFinite(parsed)) return parsed;
  }
  return null;
};

const asBooleanFromUnknown = (value: unknown): boolean | null => {
  if (typeof value === "boolean") return value;
  return null;
};

const requestLndGetInfo = (input: {
  readonly hostname: string;
  readonly port: number;
  readonly tlsCertPath: string;
  readonly macaroonHex: string;
  readonly timeoutMs: number;
}): Promise<unknown> =>
  new Promise((resolve, reject) => {
    const req = https.request(
      {
        protocol: "https:",
        hostname: input.hostname,
        port: input.port,
        method: "GET",
        path: "/v1/getinfo",
        headers: {
          accept: "application/json",
          "Grpc-Metadata-macaroon": input.macaroonHex,
        },
        ca: fs.readFileSync(input.tlsCertPath),
        rejectUnauthorized: true,
      },
      (res) => {
        const chunks: Array<Buffer> = [];
        res.on("data", (chunk) => {
          if (Buffer.isBuffer(chunk)) {
            chunks.push(chunk);
            return;
          }
          chunks.push(Buffer.from(String(chunk)));
        });
        res.on("error", reject);
        res.on("end", () => {
          const bodyText = Buffer.concat(chunks).toString("utf8");
          let parsed: unknown = {};
          try {
            parsed = bodyText.trim().length === 0 ? {} : JSON.parse(bodyText);
          } catch {
            reject(new Error("lnd_rest_getinfo_invalid_json"));
            return;
          }
          if ((res.statusCode ?? 500) >= 400) {
            reject(new Error(`lnd_rest_getinfo_status_${res.statusCode ?? "unknown"}`));
            return;
          }
          resolve(parsed);
        });
      },
    );

    req.setTimeout(input.timeoutMs, () => {
      req.destroy(new Error("lnd_rest_getinfo_timeout"));
    });
    req.on("error", reject);
    req.end();
  });

const sanitizeLogLine = (line: string): string => {
  const lowered = line.toLowerCase();
  if (
    lowered.includes("macaroon") ||
    lowered.includes("password") ||
    lowered.includes("passphrase") ||
    lowered.includes("seed") ||
    lowered.includes("private key")
  ) {
    return "[REDACTED_SENSITIVE_OUTPUT]";
  }
  return line;
};

const backoffDelayMs = (config: LndRuntimeManagerConfig, consecutiveCrashCount: number): number => {
  const multiplier = Math.max(1, consecutiveCrashCount);
  const delay = config.restartBackoffBaseMs * 2 ** (multiplier - 1);
  return Math.min(config.restartBackoffMaxMs, Math.max(config.restartBackoffBaseMs, delay));
};

export type LndRuntimeManagerApi = Readonly<{
  readonly start: () => Effect.Effect<void, LndRuntimeManagerError>;
  readonly stop: () => Effect.Effect<void, LndRuntimeManagerError>;
  readonly restart: () => Effect.Effect<void, LndRuntimeManagerError>;
  readonly checkHealth: () => Effect.Effect<void, never>;
  readonly snapshot: () => Effect.Effect<LndRuntimeStatus>;
  readonly logs: () => Effect.Effect<ReadonlyArray<LndRuntimeLogEvent>>;
}>;

export class LndRuntimeManagerService extends Context.Tag("@openagents/desktop/LndRuntimeManagerService")<
  LndRuntimeManagerService,
  LndRuntimeManagerApi
>() {}

export const LndRuntimeManagerLive = Layer.effect(
  LndRuntimeManagerService,
  Effect.gen(function* () {
    const config = yield* LndRuntimeManagerConfigService;
    const transport = yield* LndProcessTransportService;

    const statusRef = yield* Ref.make(runtimeStatusInitial());
    const logsRef = yield* Ref.make<ReadonlyArray<LndRuntimeLogEvent>>([]);
    const processRef = yield* Ref.make<LndProcessHandle | null>(null);
    const runtimePathsRef = yield* Ref.make<LndRuntimePaths | null>(null);
    const stopRequestedRef = yield* Ref.make(false);
    const healthFiberRef = yield* Ref.make<Fiber.RuntimeFiber<unknown, unknown> | null>(null);
    const restartFiberRef = yield* Ref.make<Fiber.RuntimeFiber<unknown, unknown> | null>(null);
    const exitFiberRef = yield* Ref.make<Fiber.RuntimeFiber<unknown, unknown> | null>(null);

    const appendLog = Effect.fn("LndRuntimeManager.appendLog")(function* (
      level: LndRuntimeLogLevel,
      event: string,
      message: string,
    ) {
      const now = yield* Clock.currentTimeMillis;
      const entry: LndRuntimeLogEvent = {
        atMs: now,
        level,
        event,
        message: sanitizeLogLine(message),
      };
      yield* Ref.update(logsRef, (current) => {
        const next = [...current, entry];
        if (next.length <= config.logHistoryLimit) return next;
        return next.slice(next.length - config.logHistoryLimit);
      });
      const prefix = `[desktop:lnd:${event}]`;
      if (level === "error") {
        console.error(prefix, entry.message);
      } else if (level === "warn") {
        console.warn(prefix, entry.message);
      } else {
        console.info(prefix, entry.message);
      }
    });

    const setStatus = Effect.fn("LndRuntimeManager.setStatus")(function* (
      update: (current: LndRuntimeStatus, now: number) => LndRuntimeStatus,
    ) {
      const now = yield* Clock.currentTimeMillis;
      yield* Ref.update(statusRef, (current) => update(current, now));
    });

    const interruptFiber = (fiberRef: Ref.Ref<Fiber.RuntimeFiber<unknown, unknown> | null>) =>
      Effect.gen(function* () {
        const current = yield* Ref.get(fiberRef);
        if (!current) return;
        yield* Fiber.interrupt(current);
        yield* Ref.set(fiberRef, null);
      });

    const materializeConfig = Effect.fn("LndRuntimeManager.materializeConfig")(function* () {
      return yield* Effect.try({
        try: () =>
          materializeLndRuntimeConfig({
            userDataPath: config.userDataPath,
            network: config.network,
            alias: config.alias,
            rpcListen: config.rpcListen,
            restListen: config.restListen,
            p2pListen: config.p2pListen,
            debugLevel: config.debugLevel,
            neutrinoPeers: config.neutrinoPeers,
          }),
        catch: (error) =>
          new LndRuntimeManagerError(
            "config_materialization_failed",
            `Failed to materialize lnd config: ${String(error)}`,
          ),
      });
    });

    const resolveBinary = Effect.fn("LndRuntimeManager.resolveBinary")(function* () {
      return yield* Effect.try({
        try: () =>
          resolveAndVerifyLndBinary({
            appPath: config.appPath,
            resourcesPath: config.resourcesPath,
            isPackaged: config.isPackaged,
            env: config.env,
          }),
        catch: (error) => {
          if (error instanceof LndBinaryResolverError) {
            return new LndRuntimeManagerError(
              "binary_resolution_failed",
              `Binary resolution failed (${error.code}): ${error.message}`,
            );
          }
          return new LndRuntimeManagerError(
            "binary_resolution_failed",
            `Binary resolution failed: ${String(error)}`,
          );
        },
      });
    });

    const readSyncSnapshot = Effect.fn("LndRuntimeManager.readSyncSnapshot")(function* (
      paths: LndRuntimePaths,
      previous: LndRuntimeSyncSnapshot,
    ) {
      const now = yield* Clock.currentTimeMillis;

      const walletDir = path.join(paths.runtimeDir, "data", "chain", "bitcoin", config.network);
      const macaroonPath = path.join(walletDir, "admin.macaroon");
      if (!fs.existsSync(macaroonPath)) {
        return {
          ...previous,
          source: "none" as const,
          lastUpdatedAtMs: now,
          lastError: "admin_macaroon_unavailable",
        } satisfies LndRuntimeSyncSnapshot;
      }

      const listen = parseRestAddress(config.restListen);
      const macaroonHex = readMacaroonHex(macaroonPath);
      const raw = yield* Effect.tryPromise({
        try: () =>
          requestLndGetInfo({
            hostname: listen.hostname,
            port: listen.port,
            tlsCertPath: paths.tlsCertPath,
            macaroonHex,
            timeoutMs: Math.max(1_000, config.healthProbeIntervalMs),
          }),
        catch: (error) => new Error(String(error)),
      }).pipe(
        Effect.catchAll((error) =>
          Effect.succeed({
            _tag: "sync_error",
            message: String(error.message),
          } as const),
        ),
      );

      if (typeof raw === "object" && raw !== null && "_tag" in raw) {
        const message =
          "message" in raw && typeof raw.message === "string" ? raw.message : "sync_probe_failed";
        return {
          ...previous,
          source: "none" as const,
          lastUpdatedAtMs: now,
          lastError: message,
        } satisfies LndRuntimeSyncSnapshot;
      }

      const payload = raw as Record<string, unknown>;
      return {
        source: "rest_getinfo",
        blockHeight: asNumberFromUnknown(payload.block_height),
        numPeers: asNumberFromUnknown(payload.num_peers),
        bestHeaderTimestamp: asNumberFromUnknown(payload.best_header_timestamp),
        syncedToChain: asBooleanFromUnknown(payload.synced_to_chain),
        syncedToGraph: asBooleanFromUnknown(payload.synced_to_graph),
        walletSynced: asBooleanFromUnknown(payload.wallet_synced),
        lastUpdatedAtMs: now,
        lastError: null,
      } satisfies LndRuntimeSyncSnapshot;
    });

    const updateHealth: () => Effect.Effect<void, never> = Effect.fn(
      "LndRuntimeManager.updateHealth",
    )(function* () {
      const current = yield* Ref.get(statusRef);
      if (current.lifecycle !== "running") {
        yield* setStatus((value, now) => ({
          ...value,
          lastHealthCheckAtMs: now,
          health: value.lifecycle === "starting" ? "starting" : "unknown",
        }));
        return;
      }

      const handle = yield* Ref.get(processRef);
      if (!handle) {
        yield* setStatus((value, now) => ({
          ...value,
          health: "unhealthy",
          lastHealthCheckAtMs: now,
          lastError: value.lastError ?? "process_handle_missing",
        }));
        return;
      }

      const alive = yield* handle.isAlive().pipe(Effect.orElseSucceed(() => false));
      const paths = yield* Ref.get(runtimePathsRef);
      const currentSync = current.sync;
      const nextSync =
        alive && paths
          ? yield* readSyncSnapshot(paths, currentSync).pipe(
              Effect.catchAll((error) =>
                Effect.succeed({
                  ...currentSync,
                  source: "none" as const,
                  lastUpdatedAtMs: Date.now(),
                  lastError: String(error),
                } satisfies LndRuntimeSyncSnapshot),
              ),
            )
          : currentSync;

      yield* setStatus((value, now) => ({
        ...value,
        health: alive ? "healthy" : "unhealthy",
        lastHealthCheckAtMs: now,
        sync: nextSync,
        ...(alive ? {} : { lastError: value.lastError ?? "process_not_alive" }),
      }));
    });

    // eslint-disable-next-line prefer-const -- initialized after helper closures that reference it.
    let start: () => Effect.Effect<void, LndRuntimeManagerError>;

    const onUnexpectedExit: (exit: LndProcessExit) => Effect.Effect<void, never> = Effect.fn(
      "LndRuntimeManager.onUnexpectedExit",
    )(function* (exit: LndProcessExit) {
      yield* appendLog(
        "warn",
        "process_exit",
        `LND process exited unexpectedly code=${exit.code ?? "null"} signal=${exit.signal ?? "null"}`,
      );
      yield* Ref.set(processRef, null);
      yield* Ref.set(runtimePathsRef, null);
      yield* interruptFiber(healthFiberRef);

      const snapshot = yield* Ref.get(statusRef);
      const nextCrashCount = snapshot.crashCount + 1;
      const nextConsecutive = snapshot.consecutiveCrashes + 1;

      if (nextConsecutive > config.maxCrashRestarts) {
        yield* setStatus((current, now) => ({
          ...current,
          lifecycle: "failed",
          health: "unhealthy",
          crashCount: nextCrashCount,
          consecutiveCrashes: nextConsecutive,
          lastStoppedAtMs: now,
          nextRestartAtMs: null,
          lastExitCode: exit.code,
          lastExitSignal: exit.signal,
          lastError: "crash_loop_exhausted",
          pid: null,
        }));
        yield* appendLog("error", "crash_loop", "Crash restart policy exhausted; runtime marked failed");
        return;
      }

      const delayMs = backoffDelayMs(config, nextConsecutive);
      yield* setStatus((current, now) => ({
        ...current,
        lifecycle: "backoff",
        health: "unhealthy",
        crashCount: nextCrashCount,
        consecutiveCrashes: nextConsecutive,
        nextRestartAtMs: now + delayMs,
        lastStoppedAtMs: now,
        lastExitCode: exit.code,
        lastExitSignal: exit.signal,
        lastError: `unexpected_exit_${exit.code ?? "null"}`,
        pid: null,
      }));

      yield* appendLog("warn", "restart_backoff", `Scheduling restart in ${delayMs}ms`);

      const restartFiber = yield* Effect.forkDaemon(
        Effect.sleep(`${delayMs} millis`).pipe(
          Effect.zipRight(
            Effect.gen(function* () {
              yield* Ref.set(restartFiberRef, null);
              const state = yield* Ref.get(statusRef);
              if (state.lifecycle !== "backoff") return;
              yield* start();
            }),
          ),
          Effect.catchAll(() => Effect.void),
        ),
      );
      yield* Ref.set(restartFiberRef, restartFiber);
    });

    const onProcessExit: (exit: LndProcessExit) => Effect.Effect<void, never> = Effect.fn(
      "LndRuntimeManager.onProcessExit",
    )(function* (exit: LndProcessExit) {
      const stopRequested = yield* Ref.get(stopRequestedRef);
      yield* Ref.set(stopRequestedRef, false);

      if (stopRequested) {
        yield* Ref.set(processRef, null);
        yield* Ref.set(runtimePathsRef, null);
        yield* interruptFiber(healthFiberRef);
        yield* interruptFiber(restartFiberRef);
        yield* setStatus((current, now) => ({
          ...current,
          lifecycle: "stopped",
          health: "unknown",
          lastStoppedAtMs: now,
          nextRestartAtMs: null,
          lastExitCode: exit.code,
          lastExitSignal: exit.signal,
          lastError: null,
          pid: null,
          consecutiveCrashes: 0,
          sync: {
            source: "none",
            blockHeight: null,
            numPeers: null,
            bestHeaderTimestamp: null,
            syncedToChain: null,
            syncedToGraph: null,
            walletSynced: null,
            lastUpdatedAtMs: now,
            lastError: null,
          },
        }));
        yield* appendLog("info", "stopped", "LND process stopped");
        return;
      }

      yield* onUnexpectedExit(exit);
    });

    const watchExit = (waitForExit: Effect.Effect<LndProcessExit>): Effect.Effect<void, never> =>
      waitForExit.pipe(
        Effect.flatMap(onProcessExit),
        Effect.catchAll((error) =>
          setStatus((current) => ({
            ...current,
            lifecycle: "failed",
            health: "unhealthy",
            lastError: String(error),
          })),
        ),
      );

    start = Effect.fn("LndRuntimeManager.start")(function* () {
      const current = yield* Ref.get(statusRef);
      if (current.lifecycle === "running" || current.lifecycle === "starting") {
        return;
      }

      if (current.lifecycle === "failed") {
        yield* setStatus((value) => ({
          ...value,
          lifecycle: "stopped",
          health: "unknown",
          lastError: null,
          nextRestartAtMs: null,
          consecutiveCrashes: 0,
        }));
      }

      yield* interruptFiber(restartFiberRef);
      yield* setStatus((value, now) => ({
        ...value,
        lifecycle: "starting",
        health: "starting",
        lastError: null,
        nextRestartAtMs: null,
        lastStartedAtMs: now,
      }));

      const resolvedBinary = yield* resolveBinary();
      const runtimeConfig = yield* materializeConfig();
      yield* Ref.set(runtimePathsRef, runtimeConfig.paths);

      yield* appendLog(
        "info",
        "starting",
        `Launching lnd target=${resolvedBinary.target} network=${config.network} conf=${path.basename(runtimeConfig.paths.configPath)}`,
      );

      const processHandle = yield* transport
        .spawn({
          command: resolvedBinary.binaryPath,
          args: runtimeConfig.launchArgs,
          cwd: runtimeConfig.paths.runtimeDir,
          env: config.env,
        })
        .pipe(
          Effect.mapError(
            (error) => new LndRuntimeManagerError("spawn_failed", `Failed to spawn lnd: ${String(error)}`),
          ),
        );

      yield* processHandle.onStdout((line) => {
        const sanitized = sanitizeLogLine(line);
        if (sanitized.length > 0) {
          console.info("[desktop:lnd:stdout]", sanitized);
        }
      });
      yield* processHandle.onStderr((line) => {
        const sanitized = sanitizeLogLine(line);
        if (sanitized.length > 0) {
          console.warn("[desktop:lnd:stderr]", sanitized);
        }
      });

      const alive = yield* processHandle.isAlive().pipe(Effect.orElseSucceed(() => false));

      yield* Ref.set(processRef, processHandle);
      yield* Ref.set(stopRequestedRef, false);
      yield* setStatus((value) => ({
        ...value,
        lifecycle: "running",
        health: alive ? "healthy" : "starting",
        target: resolvedBinary.target,
        source: resolvedBinary.source,
        pid: processHandle.pid,
        configPath: runtimeConfig.paths.configPath,
        runtimeDir: runtimeConfig.paths.runtimeDir,
        binaryPath: resolvedBinary.binaryPath,
        restartCount: value.restartCount + 1,
      }));

      const healthFiber = yield* Effect.forkDaemon(
        Effect.repeat(
          updateHealth().pipe(
            Effect.catchAll(() => Effect.void),
            Effect.catchAllDefect(() => Effect.void),
          ),
          Schedule.spaced(`${config.healthProbeIntervalMs} millis`),
        ),
      );
      yield* Ref.set(healthFiberRef, healthFiber);

      const exitFiber = yield* Effect.forkDaemon(watchExit(processHandle.waitForExit));
      yield* Ref.set(exitFiberRef, exitFiber);

      yield* appendLog("info", "running", `LND process running pid=${processHandle.pid}`);
    });

    const stop: () => Effect.Effect<void, LndRuntimeManagerError> = Effect.fn(
      "LndRuntimeManager.stop",
    )(function* () {
      const current = yield* Ref.get(statusRef);
      if (current.lifecycle === "stopped") {
        yield* interruptFiber(restartFiberRef);
        return;
      }

      yield* interruptFiber(restartFiberRef);

      const handle = yield* Ref.get(processRef);
      if (!handle) {
        yield* Ref.set(runtimePathsRef, null);
        yield* setStatus((value, now) => ({
          ...value,
          lifecycle: "stopped",
          health: "unknown",
          pid: null,
          nextRestartAtMs: null,
          consecutiveCrashes: 0,
          lastStoppedAtMs: now,
          sync: {
            source: "none",
            blockHeight: null,
            numPeers: null,
            bestHeaderTimestamp: null,
            syncedToChain: null,
            syncedToGraph: null,
            walletSynced: null,
            lastUpdatedAtMs: now,
            lastError: null,
          },
        }));
        return;
      }

      yield* appendLog("info", "stopping", "Stopping LND process");
      yield* Ref.set(stopRequestedRef, true);
      yield* setStatus((value) => ({
        ...value,
        lifecycle: "stopping",
        health: "unknown",
        nextRestartAtMs: null,
      }));

      yield* handle.kill("SIGTERM").pipe(
        Effect.mapError(
          (error) => new LndRuntimeManagerError("kill_failed", `Failed to stop lnd process: ${String(error)}`),
        ),
      );

      const exitFiber = yield* Ref.get(exitFiberRef);
      if (exitFiber) {
        yield* Fiber.join(exitFiber).pipe(
          Effect.catchAll(() => Effect.void),
          Effect.catchAllDefect(() => Effect.void),
        );
      }
      yield* Ref.set(exitFiberRef, null);
    });

    const restart: () => Effect.Effect<void, LndRuntimeManagerError> = Effect.fn(
      "LndRuntimeManager.restart",
    )(function* () {
      yield* stop();
      yield* start();
    });

    return LndRuntimeManagerService.of({
      start,
      stop,
      restart,
      checkHealth: () =>
        updateHealth().pipe(
          Effect.catchAll(() => Effect.void),
          Effect.catchAllDefect(() => Effect.void),
        ),
      snapshot: () => Ref.get(statusRef),
      logs: () => Ref.get(logsRef),
    });
  }),
);

export type LndRuntimeSnapshotForRenderer = Readonly<{
  readonly lifecycle: LndRuntimeLifecycle;
  readonly health: LndRuntimeHealth;
  readonly target: LndBinaryTarget | null;
  readonly pid: number | null;
  readonly restartCount: number;
  readonly crashCount: number;
  readonly nextRestartAtMs: number | null;
  readonly lastHealthCheckAtMs: number | null;
  readonly lastError: string | null;
  readonly sync: LndRuntimeSyncSnapshot;
}>;

export const projectLndRuntimeSnapshotForRenderer = (
  snapshot: LndRuntimeStatus,
): LndRuntimeSnapshotForRenderer => ({
  lifecycle: snapshot.lifecycle,
  health: snapshot.health,
  target: snapshot.target,
  pid: snapshot.pid,
  restartCount: snapshot.restartCount,
  crashCount: snapshot.crashCount,
  nextRestartAtMs: snapshot.nextRestartAtMs,
  lastHealthCheckAtMs: snapshot.lastHealthCheckAtMs,
  lastError: snapshot.lastError,
  sync: snapshot.sync,
});

export const toRuntimeManagerError = (error: unknown): LndRuntimeManagerError => {
  if (error instanceof LndRuntimeManagerError) return error;
  return new LndRuntimeManagerError("spawn_failed", String(error));
};

export const readRuntimeConfigFile = (paths: LndRuntimePaths): string =>
  fs.readFileSync(paths.configPath, "utf8");

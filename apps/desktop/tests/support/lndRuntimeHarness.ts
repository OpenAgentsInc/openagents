import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { Effect, Layer } from "effect";

import {
  defaultLndRuntimeManagerConfig,
  LndRuntimeManagerConfigLive,
  LndRuntimeManagerLive,
  type LndRuntimeManagerConfig,
} from "../../src/main/lndRuntimeManager";
import {
  LndProcessTransportService,
  type LndProcessExit,
  type LndSpawnInput,
} from "../../src/main/lndProcessTransport";

export type FakeLndProcessController = Readonly<{
  readonly pid: number;
  readonly input: LndSpawnInput;
  readonly emitStdout: (line: string) => void;
  readonly emitStderr: (line: string) => void;
  readonly exitUnexpected: (exit?: Partial<LndProcessExit>) => void;
  readonly wasKilled: () => boolean;
}>;

export type LndRuntimeHarness = Readonly<{
  readonly rootDir: string;
  readonly binaryPath: string;
  readonly binarySha256: string;
  readonly spawnCalls: Array<LndSpawnInput>;
  readonly controllers: Array<FakeLndProcessController>;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  readonly layer: Layer.Layer<any, never, never>;
  readonly cleanup: () => void;
}>;

const makeFakeBinary = (): { rootDir: string; binaryPath: string; binarySha256: string } => {
  const rootDir = fs.mkdtempSync(path.join(os.tmpdir(), "oa-lnd-runtime-"));
  const binaryDir = path.join(rootDir, "bin");
  fs.mkdirSync(binaryDir, { recursive: true });
  const binaryPath = path.join(binaryDir, "lnd");
  const contents = Buffer.from("#!/bin/sh\necho openagents-lnd-test\n", "utf8");
  fs.writeFileSync(binaryPath, contents);
  fs.chmodSync(binaryPath, 0o755);
  const binarySha256 = crypto.createHash("sha256").update(contents).digest("hex");
  return { rootDir, binaryPath, binarySha256 };
};

export const makeLndRuntimeHarness = (
  override?: Partial<
    Omit<
      LndRuntimeManagerConfig,
      "appPath" | "resourcesPath" | "userDataPath" | "isPackaged" | "env"
    >
  >,
): LndRuntimeHarness => {
  const { rootDir, binaryPath, binarySha256 } = makeFakeBinary();
  const controllers: Array<FakeLndProcessController> = [];
  const spawnCalls: Array<LndSpawnInput> = [];

  let nextPid = 6_000;

  const transportLayer = Layer.succeed(
    LndProcessTransportService,
    LndProcessTransportService.of({
      spawn: (input) =>
        Effect.sync(() => {
          spawnCalls.push(input);

          let killed = false;
          let alive = true;
          const stdoutListeners: Array<(line: string) => void> = [];
          const stderrListeners: Array<(line: string) => void> = [];
          let resolveExit: ((exit: LndProcessExit) => void) | null = null;
          const exitPromise = new Promise<LndProcessExit>((resolve) => {
            resolveExit = resolve;
          });

          const exit = (exitInput: LndProcessExit): void => {
            if (!alive) return;
            alive = false;
            resolveExit?.(exitInput);
            resolveExit = null;
          };

          const controller: FakeLndProcessController = {
            pid: nextPid,
            input,
            emitStdout: (line) => {
              for (const listener of stdoutListeners) listener(line);
            },
            emitStderr: (line) => {
              for (const listener of stderrListeners) listener(line);
            },
            exitUnexpected: (exitInput) =>
              exit({
                code: exitInput?.code ?? 1,
                signal: exitInput?.signal ?? null,
              }),
            wasKilled: () => killed,
          };
          controllers.push(controller);

          const pid = nextPid;
          nextPid += 1;

          return {
            pid,
            command: input.command,
            args: input.args,
            waitForExit: Effect.promise(() => exitPromise),
            kill: (signal = "SIGTERM" as const) =>
              Effect.sync(() => {
                killed = true;
                exit({
                  code: 0,
                  signal,
                });
              }),
            isAlive: () => Effect.succeed(alive),
            onStdout: (listener: (line: string) => void) =>
              Effect.sync(() => {
                stdoutListeners.push(listener);
              }),
            onStderr: (listener: (line: string) => void) =>
              Effect.sync(() => {
                stderrListeners.push(listener);
              }),
          };
        }),
    }),
  );

  const config = {
    ...defaultLndRuntimeManagerConfig({
      appPath: rootDir,
      resourcesPath: rootDir,
      userDataPath: rootDir,
      isPackaged: false,
      env: {
        OA_DESKTOP_LND_DEV_BINARY_PATH: binaryPath,
        OA_DESKTOP_LND_DEV_BINARY_SHA256: binarySha256,
        OA_DESKTOP_LND_TARGET: "darwin-arm64",
      },
    }),
    ...override,
  } satisfies LndRuntimeManagerConfig;

  const configLayer = LndRuntimeManagerConfigLive(config);
  const managerLayer = Layer.provideMerge(
    LndRuntimeManagerLive,
    Layer.provideMerge(transportLayer, configLayer),
  ) as Layer.Layer<any, never, never>;

  return {
    rootDir,
    binaryPath,
    binarySha256,
    spawnCalls,
    controllers,
    layer: managerLayer,
    cleanup: () => {
      fs.rmSync(rootDir, { recursive: true, force: true });
    },
  };
};

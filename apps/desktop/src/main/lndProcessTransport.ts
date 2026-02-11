import { spawn } from "node:child_process";
import type { ChildProcessWithoutNullStreams } from "node:child_process";

import { Context, Effect, Layer } from "effect";

export type LndProcessExit = Readonly<{
  readonly code: number | null;
  readonly signal: NodeJS.Signals | null;
}>;

export type LndProcessHandle = Readonly<{
  readonly pid: number;
  readonly command: string;
  readonly args: ReadonlyArray<string>;
  readonly waitForExit: Effect.Effect<LndProcessExit>;
  readonly kill: (signal?: NodeJS.Signals) => Effect.Effect<void>;
  readonly isAlive: () => Effect.Effect<boolean>;
  readonly onStdout: (listener: (line: string) => void) => Effect.Effect<void>;
  readonly onStderr: (listener: (line: string) => void) => Effect.Effect<void>;
}>;

export type LndSpawnInput = Readonly<{
  readonly command: string;
  readonly args: ReadonlyArray<string>;
  readonly cwd: string;
  readonly env: NodeJS.ProcessEnv;
}>;

export type LndProcessTransportApi = Readonly<{
  readonly spawn: (input: LndSpawnInput) => Effect.Effect<LndProcessHandle, unknown>;
}>;

export class LndProcessTransportService extends Context.Tag("@openagents/desktop/LndProcessTransportService")<
  LndProcessTransportService,
  LndProcessTransportApi
>() {}

const attachLineEmitter = (
  stream: NodeJS.ReadableStream,
  listener: (line: string) => void,
): (() => void) => {
  let pending = "";
  const onData = (chunk: Buffer | string): void => {
    pending += chunk.toString();
    const lines = pending.split(/\r?\n/);
    pending = lines.pop() ?? "";
    for (const line of lines) {
      listener(line);
    }
  };

  stream.on("data", onData);

  return () => {
    stream.off("data", onData);
    if (pending.length > 0) listener(pending);
    pending = "";
  };
};

const makeHandle = (
  child: ChildProcessWithoutNullStreams,
  command: string,
  args: ReadonlyArray<string>,
): LndProcessHandle => {
  let resolveExit: ((exit: LndProcessExit) => void) | null = null;
  const exitPromise = new Promise<LndProcessExit>((resolve) => {
    resolveExit = resolve;
  });

  child.once("exit", (code, signal) => {
    resolveExit?.({
      code,
      signal,
    });
    resolveExit = null;
  });

  const onStdout = (listener: (line: string) => void): Effect.Effect<void> =>
    Effect.sync(() => {
      attachLineEmitter(child.stdout, listener);
    });

  const onStderr = (listener: (line: string) => void): Effect.Effect<void> =>
    Effect.sync(() => {
      attachLineEmitter(child.stderr, listener);
    });

  return {
    pid: child.pid ?? -1,
    command,
    args,
    waitForExit: Effect.promise(() => exitPromise),
    kill: (signal = "SIGTERM") =>
      Effect.sync(() => {
        if (child.killed || child.exitCode !== null) return;
        child.kill(signal);
      }),
    isAlive: () => Effect.sync(() => child.exitCode === null && !child.killed),
    onStdout,
    onStderr,
  };
};

export const LndProcessTransportNodeLive = Layer.succeed(
  LndProcessTransportService,
  LndProcessTransportService.of({
    spawn: (input) =>
      Effect.try({
        try: () => {
          const child = spawn(input.command, [...input.args], {
            cwd: input.cwd,
            env: input.env,
            stdio: "pipe",
            windowsHide: true,
          });
          return makeHandle(child, input.command, input.args);
        },
        catch: (error) => error,
      }),
  }),
);

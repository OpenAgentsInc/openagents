import { spawn, type ChildProcess } from "node:child_process";

export interface ComputerExecutionLimits {
  readonly timeoutMillis: number;
  readonly maximumOutputBytes: number;
}

export interface ComputerExecutionOutcome {
  readonly exitCode: number | null;
  readonly truncated: boolean;
  readonly timedOut: boolean;
  readonly cancelled: boolean;
  readonly durationMillis: number;
}

export interface RunningComputerExecution {
  readonly done: Promise<ComputerExecutionOutcome>;
  readonly cancel: () => void;
}

export const computerExecutionDefaults: ComputerExecutionLimits = {
  timeoutMillis: 30_000,
  maximumOutputBytes: 64 * 1024,
};

const environmentNames = ["PATH", "HOME", "LANG", "LC_ALL", "TMPDIR", "SHELL", "USER", "TERM"];

const scrubEnvironment = (source: NodeJS.ProcessEnv): NodeJS.ProcessEnv =>
  Object.fromEntries(
    environmentNames.flatMap((name) => {
      const value = source[name];
      return value === undefined ? [] : [[name, value]];
    }),
  );

const scrubOutput = (text: string): string =>
  text
    .replaceAll(
      /(?:oa_(?:pat|agent|assignment)_[A-Za-z0-9._-]+|smct_[A-Za-z0-9._-]+)/gu,
      "[REDACTED]",
    )
    .replaceAll(/Bearer\s+\S+/giu, "Bearer [REDACTED]");

const terminateGroup = (child: ChildProcess): void => {
  if (child.pid === undefined) return;
  try {
    if (process.platform === "win32") {
      child.kill("SIGTERM");
    } else {
      process.kill(-child.pid, "SIGTERM");
    }
  } catch {
    child.kill("SIGTERM");
  }
  const escalation = setTimeout(() => {
    try {
      if (process.platform === "win32") child.kill("SIGKILL");
      else if (child.pid !== undefined) process.kill(-child.pid, "SIGKILL");
    } catch {
      child.kill("SIGKILL");
    }
  }, 2_000);
  escalation.unref();
};

export const executeComputerCommand = (
  argv: ReadonlyArray<string>,
  cwd: string,
  limits: ComputerExecutionLimits,
  onChunk: (text: string) => void,
): RunningComputerExecution => {
  const startedAt = Date.now();
  let settled = false;
  let cancelled = false;
  let timedOut = false;
  let bytes = 0;
  let truncated = false;
  let resolveDone: (outcome: ComputerExecutionOutcome) => void = () => undefined;
  const done = new Promise<ComputerExecutionOutcome>((resolve) => {
    resolveDone = resolve;
  });
  const [command, ...args] = argv;
  if (command === undefined) {
    queueMicrotask(() => {
      settled = true;
      resolveDone({
        exitCode: 127,
        truncated: false,
        timedOut: false,
        cancelled: false,
        durationMillis: Date.now() - startedAt,
      });
    });
    return { done, cancel: () => undefined };
  }

  let child: ChildProcess;
  try {
    child = spawn(command, args, {
      cwd,
      detached: process.platform !== "win32",
      env: scrubEnvironment(process.env),
      shell: false,
      stdio: ["ignore", "pipe", "pipe"],
    });
  } catch {
    queueMicrotask(() => {
      settled = true;
      resolveDone({
        exitCode: 127,
        truncated: false,
        timedOut: false,
        cancelled: false,
        durationMillis: Date.now() - startedAt,
      });
    });
    return { done, cancel: () => undefined };
  }

  const finish = (exitCode: number | null): void => {
    if (settled) return;
    settled = true;
    resolveDone({
      exitCode,
      truncated,
      timedOut,
      cancelled,
      durationMillis: Date.now() - startedAt,
    });
  };
  const emit = (chunk: Buffer): void => {
    const text = scrubOutput(chunk.toString("utf8"));
    const encoded = Buffer.from(text, "utf8");
    if (bytes >= limits.maximumOutputBytes) {
      truncated = true;
      return;
    }
    const remaining = limits.maximumOutputBytes - bytes;
    const bounded = encoded.subarray(0, remaining);
    bytes += bounded.byteLength;
    truncated ||= bounded.byteLength < encoded.byteLength;
    if (bounded.byteLength > 0) onChunk(bounded.toString("utf8"));
  };
  child.stdout?.on("data", emit);
  child.stderr?.on("data", emit);
  const timeout = setTimeout(() => {
    timedOut = true;
    terminateGroup(child);
  }, limits.timeoutMillis);
  timeout.unref();
  child.once("error", () => {
    clearTimeout(timeout);
    finish(127);
  });
  child.once("close", (code) => {
    clearTimeout(timeout);
    finish(code);
  });
  return {
    done,
    cancel: () => {
      if (settled) return;
      cancelled = true;
      terminateGroup(child);
    },
  };
};

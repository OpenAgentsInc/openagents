import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";

import {
  DapResponseSchema,
  DapTransportFailure,
  dapTransportFailure,
  encodeDapProtocolMessage,
  makeDapMessageDecoder,
  makeDapRequestBroker,
  type DapEvent,
  type DapJson,
  type DapRequest,
  type DapRequestOptions,
  type DapResponse,
  type DapTimeoutScheduler,
} from "./dap-transport.ts";

const DAP_DEFAULT_STDERR_BYTES = 64 * 1_024;
const DAP_DEFAULT_QUEUED_EVENTS = 512;
const DAP_DEFAULT_REVERSE_REQUESTS = 64;
const DAP_DEFAULT_TERMINATE_GRACE_MS = 1_500;
const DAP_DEFAULT_KILL_GRACE_MS = 1_000;

export interface DapClientLaunch {
  readonly executable: string;
  readonly argv: ReadonlyArray<string>;
  readonly cwd: string;
  readonly environment: Readonly<Record<string, string>>;
  readonly timeoutMs: number;
}

export interface DapClientOptions {
  readonly launch: DapClientLaunch;
  readonly onEvent: (event: DapEvent) => void | Promise<void>;
  readonly onExit: (
    exit: Readonly<{
      code: number | null;
      signal: NodeJS.Signals | null;
      stderr: string;
    }>,
  ) => void | Promise<void>;
  readonly maxHeaderBytes?: number;
  readonly maxBodyBytes?: number;
  readonly maxBufferedBytes?: number;
  readonly maxPendingRequests?: number;
  readonly maxStderrBytes?: number;
  readonly maxQueuedEvents?: number;
  readonly maxReverseRequests?: number;
  readonly terminateGraceMs?: number;
  readonly killGraceMs?: number;
  readonly scheduleRequestTimeout?: DapTimeoutScheduler;
  readonly spawnProcess?: typeof spawn;
}

export interface DapClient {
  readonly pid: number | null;
  readonly request: (
    command: string,
    argumentsValue?: DapJson,
    options?: DapRequestOptions,
  ) => Promise<DapResponse>;
  readonly dispose: (reason: string) => Promise<void>;
  readonly drainEvents: () => Promise<void>;
  readonly pendingRequestCount: () => number;
  readonly isExited: () => boolean;
}

const redactAdapterText = (text: string): string =>
  text
    .replace(/(?:github_pat|gh[pousr]_|sk-|AKIA|xox[baprs]-)[A-Za-z0-9_-]{8,}/gu, "«redacted»")
    .replace(
      /\b(?:token|authorization|password|passwd|cookie|mnemonic)\s*[=:]\s*\S+/giu,
      "credential=«redacted»",
    );

const boundedUtf8Tail = (previous: Uint8Array, chunk: Uint8Array, limit: number): Uint8Array => {
  const combined = Buffer.concat([previous, Buffer.from(chunk)]);
  if (combined.byteLength <= limit) return combined;
  return combined.subarray(combined.byteLength - limit);
};

const signalProcessGroup = (
  child: ChildProcessWithoutNullStreams,
  signal: NodeJS.Signals,
): void => {
  if (typeof child.pid !== "number") return;
  try {
    if (process.platform === "win32") {
      child.kill(signal);
    } else {
      process.kill(-child.pid, signal);
    }
  } catch {
    // The process group is already gone.
  }
};

const validatePositiveLimit = (value: number, label: string): number => {
  if (!Number.isInteger(value) || value < 1) {
    throw dapTransportFailure("request", `${label} must be a positive integer.`, false);
  }
  return value;
};

const ignoreVoid = (): void => undefined;

const makeVoidDeferred = (): Readonly<{
  promise: Promise<void>;
  resolve: () => void;
}> => {
  let resolveValue = ignoreVoid;
  const promise = new Promise<void>((resolve) => {
    resolveValue = resolve;
  });
  return { promise, resolve: resolveValue };
};

const waitFor = (
  milliseconds: number,
): Readonly<{
  promise: Promise<void>;
  cancel: () => void;
}> => {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const promise = new Promise<void>((resolve) => {
    timer = setTimeout(resolve, milliseconds);
    timer.unref?.();
  });
  return {
    promise,
    cancel: () => {
      if (timer !== undefined) clearTimeout(timer);
    },
  };
};

export const openDapClient = (options: DapClientOptions): DapClient => {
  const maxStderrBytes = validatePositiveLimit(
    options.maxStderrBytes ?? DAP_DEFAULT_STDERR_BYTES,
    "DAP stderr limit",
  );
  const maxQueuedEvents = validatePositiveLimit(
    options.maxQueuedEvents ?? DAP_DEFAULT_QUEUED_EVENTS,
    "DAP queued event limit",
  );
  const maxReverseRequests = validatePositiveLimit(
    options.maxReverseRequests ?? DAP_DEFAULT_REVERSE_REQUESTS,
    "DAP reverse request limit",
  );
  const terminateGraceMs = validatePositiveLimit(
    options.terminateGraceMs ?? DAP_DEFAULT_TERMINATE_GRACE_MS,
    "DAP terminate grace",
  );
  const killGraceMs = validatePositiveLimit(
    options.killGraceMs ?? DAP_DEFAULT_KILL_GRACE_MS,
    "DAP kill grace",
  );

  const child = (options.spawnProcess ?? spawn)(
    options.launch.executable,
    [...options.launch.argv],
    {
      cwd: options.launch.cwd,
      env: { ...options.launch.environment },
      detached: true,
      stdio: ["pipe", "pipe", "pipe"],
    },
  );
  const decoder = makeDapMessageDecoder({
    ...(options.maxHeaderBytes === undefined ? {} : { maxHeaderBytes: options.maxHeaderBytes }),
    ...(options.maxBodyBytes === undefined ? {} : { maxBodyBytes: options.maxBodyBytes }),
    ...(options.maxBufferedBytes === undefined
      ? {}
      : { maxBufferedBytes: options.maxBufferedBytes }),
  });
  let exited = false;
  let disposed = false;
  let disposePromise: Promise<void> | null = null;
  let terminalFailure: DapTransportFailure | null = null;
  let stderrTail: Uint8Array = new Uint8Array(0);
  let eventTail = Promise.resolve();
  let queuedEvents = 0;
  let reverseRequests = 0;
  let nextReverseResponseSequence = 1_000_000_000;

  const write = (message: DapRequest | DapResponse): void => {
    if (exited || child.stdin.destroyed || !child.stdin.writable) {
      throw dapTransportFailure("request", "DAP adapter stdin is no longer available.", true);
    }
    child.stdin.write(encodeDapProtocolMessage(message));
  };

  const broker = makeDapRequestBroker({
    timeoutMs: validatePositiveLimit(options.launch.timeoutMs, "DAP request timeout"),
    ...(options.maxPendingRequests === undefined
      ? {}
      : { maxPendingRequests: options.maxPendingRequests }),
    ...(options.scheduleRequestTimeout === undefined
      ? {}
      : { scheduleTimeout: options.scheduleRequestTimeout }),
    onSend: write,
  });

  const stopForFailure = (failure: DapTransportFailure): void => {
    if (terminalFailure !== null || exited) return;
    terminalFailure = failure;
    broker.failAll(failure.detail);
    signalProcessGroup(child, "SIGTERM");
  };

  const queueEvent = (event: DapEvent): void => {
    if (queuedEvents >= maxQueuedEvents) {
      stopForFailure(dapTransportFailure("message", "DAP queued event limit was reached.", false));
      return;
    }
    queuedEvents += 1;
    eventTail = eventTail
      .then(() => options.onEvent(event))
      .catch((cause: unknown) => {
        const detail = cause instanceof Error ? cause.message : String(cause);
        stopForFailure(
          dapTransportFailure("message", `DAP event handler failed: ${detail}`, false),
        );
      })
      .then(() => {
        queuedEvents -= 1;
      });
  };

  child.stderr.on("data", (chunk: Buffer) => {
    stderrTail = boundedUtf8Tail(stderrTail, chunk, maxStderrBytes);
  });
  child.stdout.on("data", (chunk: Buffer) => {
    if (terminalFailure !== null || exited) return;
    try {
      for (const message of decoder.push(chunk)) {
        if (message.type === "response") {
          broker.accept(message);
        } else if (message.type === "event") {
          queueEvent(message);
        } else {
          reverseRequests += 1;
          if (reverseRequests > maxReverseRequests) {
            throw dapTransportFailure("message", "DAP reverse request limit was reached.", false);
          }
          const sequence = nextReverseResponseSequence;
          nextReverseResponseSequence = sequence >= 2_000_000_000 ? 1_000_000_000 : sequence + 1;
          write(
            DapResponseSchema.make({
              seq: sequence,
              type: "response",
              request_seq: message.seq,
              success: false,
              command: message.command,
              message: `Adapter reverse request ${message.command} is not admitted.`,
            }),
          );
        }
      }
    } catch (cause) {
      stopForFailure(
        cause instanceof DapTransportFailure
          ? cause
          : dapTransportFailure(
              "message",
              cause instanceof Error ? cause.message : "DAP adapter emitted an invalid message.",
              false,
            ),
      );
    }
  });

  const exitDeferred = makeVoidDeferred();
  const onExit = (code: number | null, signal: NodeJS.Signals | null): void => {
    if (exited) return;
    exited = true;
    broker.failAll(disposed ? "DAP adapter was disposed." : "DAP adapter exited.");
    try {
      decoder.finish();
    } catch (cause) {
      const detail =
        cause instanceof Error ? cause.message : "DAP transport ended with invalid data.";
      stderrTail = boundedUtf8Tail(stderrTail, Buffer.from(`\n${detail}`, "utf8"), maxStderrBytes);
    }
    const stderr = redactAdapterText(new TextDecoder("utf-8").decode(stderrTail));
    void eventTail
      .then(() => options.onExit({ code, signal, stderr }))
      .catch(() => undefined)
      .finally(exitDeferred.resolve);
  };
  child.once("exit", onExit);
  child.once("error", (cause) => {
    stderrTail = boundedUtf8Tail(
      stderrTail,
      Buffer.from(`\n${cause.message}`, "utf8"),
      maxStderrBytes,
    );
    onExit(null, null);
  });
  child.stdin.on("error", (cause: Error) => {
    stopForFailure(
      dapTransportFailure("request", `DAP adapter stdin failed: ${cause.message}`, true),
    );
  });

  return {
    pid: child.pid ?? null,
    request: async (command, argumentsValue, requestOptions) => {
      const pending = broker.request(command, argumentsValue, requestOptions);
      const response = await pending.response;
      if (!response.success) {
        throw dapTransportFailure(
          "response",
          response.message ?? `DAP ${command} request failed.`,
          false,
        );
      }
      return response;
    },
    dispose: (reason) => {
      if (disposePromise !== null) return disposePromise;
      disposed = true;
      disposePromise = (async () => {
        broker.failAll(redactAdapterText(reason) || "DAP adapter was disposed.");
        if (exited) return exitDeferred.promise;

        signalProcessGroup(child, "SIGTERM");
        const terminateWait = waitFor(terminateGraceMs);
        await Promise.race([exitDeferred.promise, terminateWait.promise]);
        terminateWait.cancel();
        if (exited) return exitDeferred.promise;

        signalProcessGroup(child, "SIGKILL");
        const killWait = waitFor(killGraceMs);
        await Promise.race([exitDeferred.promise, killWait.promise]);
        killWait.cancel();
      })();
      return disposePromise;
    },
    drainEvents: () => eventTail,
    pendingRequestCount: broker.pendingCount,
    isExited: () => exited,
  };
};

import { spawn, spawnSync, type ChildProcessWithoutNullStreams } from "node:child_process";
import { createHash } from "node:crypto";
import { accessSync, constants, realpathSync } from "node:fs";
import { isAbsolute, delimiter, resolve } from "node:path";

import {
  decodeStableAcpJsonRpcEnvelope,
  STABLE_METHOD_MANIFEST,
} from "@openagentsinc/agent-client-protocol/stable";

export type AgentStdioTransportState =
  | "startup"
  | "running"
  | "draining"
  | "exited"
  | "failed"
  | "disposed";

export type AgentStdioTransportLimits = Readonly<{
  maxLineBytes: number;
  maxBufferedBytes: number;
  maxInboundQueue: number;
  maxOutboundQueue: number;
  maxInFlightRequests: number;
  maxReverseConcurrency: number;
  maxNotificationsPerSecond: number;
  maxStderrBytes: number;
  maxEvidenceEntries: number;
  maxEvidenceBytes: number;
  requestTimeoutMs: number;
  reverseRequestTimeoutMs: number;
  shutdownGraceMs: number;
  terminateGraceMs: number;
}>;

export const DEFAULT_AGENT_STDIO_LIMITS: AgentStdioTransportLimits = Object.freeze({
  maxLineBytes: 1_048_576,
  maxBufferedBytes: 2_097_152,
  maxInboundQueue: 256,
  maxOutboundQueue: 256,
  maxInFlightRequests: 64,
  maxReverseConcurrency: 16,
  maxNotificationsPerSecond: 512,
  maxStderrBytes: 65_536,
  maxEvidenceEntries: 128,
  maxEvidenceBytes: 1_048_576,
  requestTimeoutMs: 60_000,
  reverseRequestTimeoutMs: 30_000,
  shutdownGraceMs: 1_000,
  terminateGraceMs: 1_000,
});

export type AgentStdioTransportCounters = Readonly<{
  requestsStarted: number;
  requestsCompleted: number;
  requestsTimedOut: number;
  requestsCancelled: number;
  reverseRequests: number;
  reverseTimeouts: number;
  notifications: number;
  parseFailures: number;
  protocolViolations: number;
  unknownOrLateResponses: number;
  duplicateIds: number;
  overloads: number;
  stderrBytes: number;
  stderrDroppedBytes: number;
  requestLatencyMsTotal: number;
  requestLatencyMsMax: number;
  reverseLatencyMsTotal: number;
  reverseLatencyMsMax: number;
  currentInFlight: number;
  peakInFlight: number;
  currentReverse: number;
  peakReverse: number;
  currentInboundQueue: number;
  peakInboundQueue: number;
  currentOutboundQueue: number;
  peakOutboundQueue: number;
}>;

export type AgentStdioTransportTrace = Readonly<{
  at: string;
  generation: number;
  kind:
    | "request_complete"
    | "request_timeout"
    | "request_cancel"
    | "reverse_complete"
    | "reverse_timeout"
    | "overload"
    | "protocol_violation";
  method?: string;
  durationMs?: number;
}>;

export type AgentStdioTransportReceipt = Readonly<{
  generation: number;
  executable: string;
  resolvedExecutable: string;
  sanitizedArgs: ReadonlyArray<string>;
  envKeys: ReadonlyArray<string>;
  cwd: string | null;
  pid: number | null;
  state: AgentStdioTransportState;
  startedAt: string;
  endedAt: string | null;
  exitCode: number | null;
  signal: NodeJS.Signals | null;
  terminalOutcome: "clean_exit" | "crash" | "protocol_violation" | "forced_termination" | null;
  versionProbe: Readonly<{
    status: "not_requested" | "ok" | "failed" | "timed_out";
    sanitizedOutput: string;
  }>;
  stderrExcerpt: string;
  counters: AgentStdioTransportCounters;
}>;

export type AgentStdioNativeEvidence = Readonly<{
  generation: number;
  direction: "inbound" | "outbound";
  at: string;
  bytes: number;
  sha256: string;
  raw?: unknown;
}>;

export class AgentStdioTransportError extends Error {
  constructor(
    readonly kind:
      | "not_running"
      | "overload"
      | "timeout"
      | "cancelled"
      | "process_exit"
      | "protocol_violation"
      | "remote_error"
      | "disposed",
    message: string,
    readonly code?: number,
  ) {
    super(message);
    this.name = "AgentStdioTransportError";
  }
}

export class AgentStdioHandlerError extends Error {
  constructor(
    readonly code: number,
    message: string,
    readonly data?: unknown,
  ) {
    super(message);
    this.name = "AgentStdioHandlerError";
  }
}

export type AgentStdioReverseHandler = (
  params: unknown,
  context: Readonly<{
    method: string;
    requestId: string | number | null;
    signal: AbortSignal;
    generation: number;
  }>,
) => unknown | Promise<unknown>;

export type AgentStdioTransportOptions = Readonly<{
  executable: string;
  args: ReadonlyArray<string>;
  cwd?: string;
  env?: Readonly<Record<string, string | undefined>>;
  limits?: Partial<AgentStdioTransportLimits>;
  versionProbeArgs?: ReadonlyArray<string>;
  methodKinds?: ReadonlyArray<Readonly<{ method: string; kind: "request" | "notification" }>>;
}>;

type Pending = {
  generation: number;
  method: string;
  startedAt: number;
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
  abortCleanup?: () => void;
};

type ReverseActive = {
  controller: AbortController;
  timer: ReturnType<typeof setTimeout>;
  startedAt: number;
  method: string;
};
type Message = Record<string, unknown>;

let nextGeneration = 0;
const sensitiveFlag = /(?:api[-_]?key|token|secret|password|authorization|cookie)/i;
const redactText = (value: string): string =>
  value
    .replace(/\b(?:xai|sk|bearer)[-_][A-Za-z0-9._-]{8,}\b/gi, "[REDACTED]")
    .replace(
      /((?:api[-_]?key|token|secret|password|authorization|prompt|file[-_]?content|provider[-_]?metadata|login[-_]?state)\s*[=:]\s*)\S+/gi,
      "$1[REDACTED]",
    )
    .slice(0, 4_096);

const sanitizeArgs = (args: ReadonlyArray<string>): ReadonlyArray<string> => {
  let redactNext = false;
  return args.map((arg) => {
    if (redactNext) {
      redactNext = false;
      return "[REDACTED]";
    }
    const [flag] = arg.split("=", 1);
    if (sensitiveFlag.test(flag ?? "")) {
      if (!arg.includes("=")) redactNext = true;
      return arg.includes("=") ? `${flag}=[REDACTED]` : arg;
    }
    return redactText(arg);
  });
};

const resolveExecutable = (
  executable: string,
  cwd: string | undefined,
  env: NodeJS.ProcessEnv,
): string => {
  const candidates = isAbsolute(executable)
    ? [executable]
    : executable.includes("/")
      ? [resolve(cwd ?? process.cwd(), executable)]
      : (env.PATH ?? "")
          .split(delimiter)
          .filter(Boolean)
          .map((directory) => resolve(directory, executable));
  for (const candidate of candidates) {
    try {
      accessSync(candidate, constants.X_OK);
      return realpathSync(candidate);
    } catch {
      // Continue through the trusted PATH candidates.
    }
  }
  throw new TypeError(`executable not found or not executable: ${executable}`);
};

const validId = (value: unknown): value is string | number | null =>
  value === null ||
  typeof value === "string" ||
  (typeof value === "number" && Number.isSafeInteger(value));
const idKey = (id: string | number | null): string => `${typeof id}:${String(id)}`;
const own = (value: Message, key: string): boolean =>
  Object.prototype.hasOwnProperty.call(value, key);
const isMessage = (value: unknown): value is Message =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const jsonBytes = (value: unknown): { encoded: string; bytes: number; sha256: string } => {
  const encoded = JSON.stringify(value);
  if (encoded === undefined) throw new TypeError("JSON-RPC value is not serializable");
  const bytes = Buffer.byteLength(encoded);
  return { encoded, bytes, sha256: createHash("sha256").update(encoded).digest("hex") };
};

export class AgentStdioTransport {
  readonly generation = ++nextGeneration;
  readonly limits: AgentStdioTransportLimits;
  private stateValue: AgentStdioTransportState = "startup";
  private child: ChildProcessWithoutNullStreams | null = null;
  private stdoutBuffer = Buffer.alloc(0);
  private inboundQueue: Array<Message> = [];
  private outboundQueue: Array<string> = [];
  private drainingInbound = false;
  private blockedOutbound = false;
  private nextRequestId = 1;
  private pending = new Map<string, Pending>();
  private reverseActive = new Map<string, ReverseActive>();
  private reverseHandlers = new Map<string, AgentStdioReverseHandler>();
  private notificationHandlers = new Map<string, Set<(params: unknown) => void>>();
  private methodKinds: Map<string, "request" | "notification">;
  private stableAgentMethods: Set<string>;
  private stableClientMethods: Set<string>;
  private evidence: Array<AgentStdioNativeEvidence> = [];
  private evidenceBytes = 0;
  private traces: Array<AgentStdioTransportTrace> = [];
  private stderrRaw = Buffer.alloc(0);
  private notificationWindowAt = Date.now();
  private notificationsInWindow = 0;
  private terminalOutcome: AgentStdioTransportReceipt["terminalOutcome"] = null;
  private startedAt = new Date().toISOString();
  private endedAt: string | null = null;
  private exitCode: number | null = null;
  private signal: NodeJS.Signals | null = null;
  private closePromise: Promise<void> = Promise.resolve();
  private closeResolve: (() => void) | null = null;
  private countersValue = {
    requestsStarted: 0,
    requestsCompleted: 0,
    requestsTimedOut: 0,
    requestsCancelled: 0,
    reverseRequests: 0,
    reverseTimeouts: 0,
    notifications: 0,
    parseFailures: 0,
    protocolViolations: 0,
    unknownOrLateResponses: 0,
    duplicateIds: 0,
    overloads: 0,
    stderrBytes: 0,
    stderrDroppedBytes: 0,
    requestLatencyMsTotal: 0,
    requestLatencyMsMax: 0,
    reverseLatencyMsTotal: 0,
    reverseLatencyMsMax: 0,
    currentInFlight: 0,
    peakInFlight: 0,
    currentReverse: 0,
    peakReverse: 0,
    currentInboundQueue: 0,
    peakInboundQueue: 0,
    currentOutboundQueue: 0,
    peakOutboundQueue: 0,
  };
  private versionProbe: AgentStdioTransportReceipt["versionProbe"] = {
    status: "not_requested",
    sanitizedOutput: "",
  };
  private resolvedExecutable = "";

  private constructor(private readonly options: AgentStdioTransportOptions) {
    this.limits = Object.freeze({ ...DEFAULT_AGENT_STDIO_LIMITS, ...options.limits });
    for (const [name, value] of Object.entries(this.limits)) {
      if (!Number.isSafeInteger(value) || value <= 0) {
        throw new TypeError(`transport limit ${name} must be a positive safe integer`);
      }
    }
    const stableKinds = STABLE_METHOD_MANIFEST.members
      .filter((member) => member.direction === "agent-to-client" || member.direction === "protocol")
      .map((member) => [member.method, member.kind] as const);
    this.methodKinds = new Map([
      ...stableKinds,
      ...(options.methodKinds ?? []).map((entry) => [entry.method, entry.kind] as const),
    ]);
    this.stableAgentMethods = new Set(
      STABLE_METHOD_MANIFEST.members
        .filter(
          (member) => member.direction === "agent-to-client" || member.direction === "protocol",
        )
        .map((member) => member.method),
    );
    this.stableClientMethods = new Set(
      STABLE_METHOD_MANIFEST.members
        .filter((member) => member.direction === "client-to-agent")
        .map((member) => member.method),
    );
  }

  static async start(options: AgentStdioTransportOptions): Promise<AgentStdioTransport> {
    const transport = new AgentStdioTransport(options);
    await transport.startProcess();
    return transport;
  }

  get state(): AgentStdioTransportState {
    return this.stateValue;
  }

  get pid(): number | undefined {
    return this.child?.pid;
  }

  registerReverseHandler(method: string, handler: AgentStdioReverseHandler): () => void {
    this.reverseHandlers.set(method, handler);
    return () => this.reverseHandlers.delete(method);
  }

  onNotification(method: string, handler: (params: unknown) => void): () => void {
    const handlers = this.notificationHandlers.get(method) ?? new Set();
    handlers.add(handler);
    this.notificationHandlers.set(method, handlers);
    return () => handlers.delete(handler);
  }

  getReceipt(): AgentStdioTransportReceipt {
    return Object.freeze({
      generation: this.generation,
      executable: this.options.executable,
      resolvedExecutable: this.resolvedExecutable,
      sanitizedArgs: sanitizeArgs(this.options.args),
      envKeys: Object.keys(this.options.env ?? {}).toSorted(),
      cwd: this.options.cwd ?? null,
      pid: this.child?.pid ?? null,
      state: this.stateValue,
      startedAt: this.startedAt,
      endedAt: this.endedAt,
      exitCode: this.exitCode,
      signal: this.signal,
      terminalOutcome: this.terminalOutcome,
      versionProbe: this.versionProbe,
      stderrExcerpt: redactText(this.stderrRaw.toString("utf8")),
      counters: Object.freeze({ ...this.countersValue }),
    });
  }

  getTraces(): ReadonlyArray<AgentStdioTransportTrace> {
    return structuredClone(this.traces);
  }

  getResourceDiagnostics(): Readonly<{
    pending: number;
    reverse: number;
    inboundQueue: number;
    outboundQueue: number;
    stdoutBufferBytes: number;
    nativeEvidenceEntries: number;
    processListeners: number;
    streamListeners: number;
  }> {
    return {
      pending: this.pending.size,
      reverse: this.reverseActive.size,
      inboundQueue: this.inboundQueue.length,
      outboundQueue: this.outboundQueue.length,
      stdoutBufferBytes: this.stdoutBuffer.length,
      nativeEvidenceEntries: this.evidence.length,
      processListeners:
        this.child?.eventNames().reduce((sum, name) => sum + this.child!.listenerCount(name), 0) ??
        0,
      streamListeners: [this.child?.stdin, this.child?.stdout, this.child?.stderr].reduce(
        (sum, stream) =>
          sum +
          (stream?.eventNames().reduce((count, name) => count + stream.listenerCount(name), 0) ??
            0),
        0,
      ),
    };
  }

  readNativeEvidence(access: object): ReadonlyArray<AgentStdioNativeEvidence> {
    if (access !== this.evidenceAccess) throw new TypeError("native evidence access denied");
    return structuredClone(this.evidence);
  }

  private readonly evidenceAccess = Object.freeze({ generation: this.generation });

  authorizeNativeEvidence(): object {
    return this.evidenceAccess;
  }

  async request(
    method: string,
    params: unknown = {},
    options: Readonly<{ timeoutMs?: number; signal?: AbortSignal }> = {},
  ): Promise<unknown> {
    if (this.stateValue !== "running") throw this.stateError();
    if (this.pending.size >= this.limits.maxInFlightRequests) {
      this.countersValue.overloads += 1;
      this.trace("overload", method);
      throw new AgentStdioTransportError("overload", "in-flight request limit reached");
    }
    const id = this.nextRequestId++;
    const key = idKey(id);
    const timeoutMs = options.timeoutMs ?? this.limits.requestTimeoutMs;
    this.countersValue.requestsStarted += 1;
    return new Promise((resolvePromise, rejectPromise) => {
      const timer = setTimeout(() => {
        const pending = this.pending.get(key);
        if (pending === undefined) return;
        this.pending.delete(key);
        this.syncPressure();
        pending.abortCleanup?.();
        this.countersValue.requestsTimedOut += 1;
        this.recordRequestLatency(pending, "request_timeout");
        this.cancelOutboundRequest(id);
        rejectPromise(
          new AgentStdioTransportError("timeout", `${method} timed out after ${timeoutMs}ms`),
        );
      }, timeoutMs);
      const pending: Pending = {
        generation: this.generation,
        method,
        startedAt: Date.now(),
        resolve: resolvePromise,
        reject: rejectPromise,
        timer,
      };
      this.pending.set(key, pending);
      this.syncPressure();
      if (options.signal !== undefined) {
        const cancel = () => {
          if (!this.pending.delete(key)) return;
          this.syncPressure();
          clearTimeout(timer);
          this.countersValue.requestsCancelled += 1;
          this.recordRequestLatency(pending, "request_cancel");
          this.cancelOutboundRequest(id);
          rejectPromise(new AgentStdioTransportError("cancelled", `${method} cancelled`));
        };
        if (options.signal.aborted) {
          this.pending.delete(key);
          this.syncPressure();
          clearTimeout(timer);
          this.countersValue.requestsCancelled += 1;
          rejectPromise(new AgentStdioTransportError("cancelled", `${method} cancelled`));
          return;
        }
        options.signal.addEventListener("abort", cancel, { once: true });
        pending.abortCleanup = () => options.signal?.removeEventListener("abort", cancel);
      }
      try {
        this.enqueueEnvelope({ jsonrpc: "2.0", id, method, params });
      } catch (error) {
        this.pending.delete(key);
        this.syncPressure();
        clearTimeout(timer);
        pending.abortCleanup?.();
        rejectPromise(error instanceof Error ? error : new Error(String(error)));
      }
    });
  }

  notify(method: string, params: unknown = {}): void {
    if (this.stateValue !== "running") throw this.stateError();
    this.enqueueEnvelope({ jsonrpc: "2.0", method, params });
  }

  async shutdown(sessionIds: ReadonlyArray<string> = []): Promise<void> {
    if (["disposed", "exited"].includes(this.stateValue)) return;
    if (this.stateValue !== "failed") {
      this.stateValue = "draining";
      for (const sessionId of sessionIds) {
        try {
          this.enqueueEnvelope({ jsonrpc: "2.0", method: "session/cancel", params: { sessionId } });
        } catch {
          this.countersValue.overloads += 1;
        }
      }
    }
    this.rejectPending(new AgentStdioTransportError("process_exit", "transport is shutting down"));
    this.child?.stdin.end();
    if (await this.waitForClose(this.limits.shutdownGraceMs)) return;
    this.child?.kill("SIGTERM");
    if (await this.waitForClose(this.limits.terminateGraceMs)) return;
    this.terminalOutcome = "forced_termination";
    this.child?.kill("SIGKILL");
    await this.waitForClose(this.limits.terminateGraceMs);
  }

  async dispose(): Promise<void> {
    if (this.stateValue === "disposed") return;
    await this.shutdown();
    for (const active of this.reverseActive.values()) {
      clearTimeout(active.timer);
      active.controller.abort();
    }
    this.reverseActive.clear();
    this.pending.clear();
    this.reverseHandlers.clear();
    this.notificationHandlers.clear();
    this.inboundQueue = [];
    this.outboundQueue = [];
    this.stdoutBuffer = Buffer.alloc(0);
    this.blockedOutbound = false;
    this.evidence = [];
    this.evidenceBytes = 0;
    this.syncPressure();
    this.child?.removeAllListeners();
    this.child?.stdout.removeAllListeners();
    this.child?.stderr.removeAllListeners();
    this.child?.stdin.removeAllListeners();
    this.stateValue = "disposed";
  }

  private async startProcess(): Promise<void> {
    const env = Object.fromEntries(
      Object.entries(this.options.env ?? process.env).filter(
        (entry): entry is [string, string] => entry[1] !== undefined,
      ),
    );
    this.resolvedExecutable = resolveExecutable(this.options.executable, this.options.cwd, env);
    if (this.options.versionProbeArgs !== undefined) {
      const probe = spawnSync(this.resolvedExecutable, [...this.options.versionProbeArgs], {
        cwd: this.options.cwd,
        env,
        encoding: "utf8",
        timeout: Math.min(this.limits.requestTimeoutMs, 10_000),
        shell: false,
      });
      this.versionProbe = {
        status:
          (probe.error as NodeJS.ErrnoException | undefined)?.code === "ETIMEDOUT"
            ? "timed_out"
            : probe.status === 0
              ? "ok"
              : "failed",
        sanitizedOutput: redactText(`${probe.stdout ?? ""}${probe.stderr ?? ""}`),
      };
    }
    this.closePromise = new Promise((resolveClose) => {
      this.closeResolve = resolveClose;
    });
    const child = spawn(this.resolvedExecutable, [...this.options.args], {
      cwd: this.options.cwd,
      env,
      stdio: ["pipe", "pipe", "pipe"],
      shell: false,
    });
    this.child = child;
    child.stdout.on("data", (chunk: Buffer) => this.consumeStdout(chunk));
    child.stdout.on("end", () => {
      if (this.stdoutBuffer.length > 0) this.consumeStdout(Buffer.from("\n"));
    });
    child.stderr.on("data", (chunk: Buffer) => this.consumeStderr(chunk));
    child.stdout.on("error", () => this.processFailure("stdout stream failed"));
    child.stderr.on("error", () => this.processFailure("stderr stream failed"));
    child.stdin.on("error", () => this.processFailure("stdin stream failed"));
    child.stdin.on("drain", () => {
      this.blockedOutbound = false;
      this.flushOutbound();
    });
    child.once("close", (code, signal) => this.handleClose(code, signal));
    child.on("error", () => this.processFailure("agent process failed"));
    await new Promise<void>((resolveStart, rejectStart) => {
      child.once("spawn", () => {
        this.stateValue = "running";
        resolveStart();
      });
      child.once("error", (error) => {
        this.stateValue = "failed";
        this.terminalOutcome = "crash";
        rejectStart(error);
      });
    });
  }

  private stateError(): AgentStdioTransportError {
    return new AgentStdioTransportError(
      this.stateValue === "disposed" ? "disposed" : "not_running",
      `transport is ${this.stateValue}`,
    );
  }

  private enqueueEnvelope(envelope: Message): void {
    const { encoded, bytes } = jsonBytes(envelope);
    if (bytes > this.limits.maxLineBytes)
      throw new AgentStdioTransportError("overload", "outbound line limit exceeded");
    if (this.outboundQueue.length >= this.limits.maxOutboundQueue) {
      this.countersValue.overloads += 1;
      this.trace("overload");
      throw new AgentStdioTransportError("overload", "outbound queue limit reached");
    }
    this.recordEvidence("outbound", envelope, bytes);
    this.outboundQueue.push(`${encoded}\n`);
    this.syncPressure();
    this.flushOutbound();
  }

  private cancelOutboundRequest(id: number): void {
    if (this.stateValue !== "running") return;
    try {
      this.enqueueEnvelope({
        jsonrpc: "2.0",
        method: "$/cancel_request",
        params: { requestId: id },
      });
    } catch {
      // The local cancellation outcome still wins if the peer write lane is overloaded.
    }
  }

  private flushOutbound(): void {
    const stdin = this.child?.stdin;
    if (stdin === undefined || this.blockedOutbound || stdin.destroyed) return;
    while (this.outboundQueue.length > 0) {
      const line = this.outboundQueue.shift();
      this.syncPressure();
      if (line === undefined) return;
      if (!stdin.write(line, "utf8")) {
        this.blockedOutbound = true;
        return;
      }
    }
  }

  private consumeStdout(chunk: Buffer): void {
    if (!["running", "draining"].includes(this.stateValue)) return;
    let offset = 0;
    while (offset < chunk.length) {
      const newline = chunk.indexOf(0x0a, offset);
      if (newline < 0) {
        this.stdoutBuffer = Buffer.concat([this.stdoutBuffer, chunk.subarray(offset)]);
        if (this.stdoutBuffer.length > this.limits.maxBufferedBytes) {
          this.protocolFailure("stdout buffer limit exceeded");
        } else if (this.stdoutBuffer.length > this.limits.maxLineBytes) {
          this.protocolFailure("stdout line limit exceeded");
        }
        return;
      }
      let line = Buffer.concat([this.stdoutBuffer, chunk.subarray(offset, newline)]);
      this.stdoutBuffer = Buffer.alloc(0);
      offset = newline + 1;
      if (line.at(-1) === 0x0d) line = line.subarray(0, -1);
      if (line.length === 0) continue;
      if (!this.acceptLine(line)) return;
    }
  }

  private acceptLine(line: Buffer): boolean {
    if (line.length > this.limits.maxLineBytes) {
      this.protocolFailure("stdout line limit exceeded");
      return false;
    }
    let parsed: unknown;
    try {
      const text = new TextDecoder("utf-8", { fatal: true }).decode(line);
      parsed = JSON.parse(text);
    } catch {
      this.countersValue.parseFailures += 1;
      this.protocolFailure("malformed or non-UTF-8 JSON-RPC frame");
      return false;
    }
    if (!isMessage(parsed)) {
      this.protocolFailure("JSON-RPC frame must be an object");
      return false;
    }
    if (this.inboundQueue.length >= this.limits.maxInboundQueue) {
      this.countersValue.overloads += 1;
      this.trace("overload");
      this.protocolFailure("inbound queue limit exceeded");
      return false;
    }
    this.recordEvidence("inbound", parsed, line.length);
    this.inboundQueue.push(parsed);
    this.syncPressure();
    this.scheduleInbound();
    return true;
  }

  private scheduleInbound(): void {
    if (this.drainingInbound) return;
    this.drainingInbound = true;
    queueMicrotask(() => {
      try {
        for (
          let message = this.inboundQueue.shift();
          message !== undefined && ["running", "draining"].includes(this.stateValue);
          message = this.inboundQueue.shift()
        ) {
          this.syncPressure();
          this.routeMessage(message);
        }
        if (!["running", "draining"].includes(this.stateValue)) this.inboundQueue = [];
      } finally {
        this.drainingInbound = false;
        if (this.inboundQueue.length > 0) this.scheduleInbound();
      }
    });
  }

  private routeMessage(message: Message): void {
    if (message.jsonrpc !== "2.0") {
      this.protocolFailure("jsonrpc must equal 2.0");
      return;
    }
    if (own(message, "method")) {
      if (typeof message.method !== "string" || message.method.length === 0) {
        this.protocolFailure("method must be a non-empty string");
        return;
      }
      const kind = this.methodKinds.get(message.method);
      const carriesId = own(message, "id");
      if ((kind === "request" && !carriesId) || (kind === "notification" && carriesId)) {
        this.protocolFailure("method id presence contradicts method manifest");
        return;
      }
      if (this.stableAgentMethods.has(message.method)) {
        const decoded = decodeStableAcpJsonRpcEnvelope({
          direction: message.method === "$/cancel_request" ? "protocol" : "agent-to-client",
          message,
        });
        if (decoded["_tag"] === "DecodeFailure") {
          if (decoded.reason === "invalid_payload" && carriesId && validId(message.id)) {
            this.sendError(message.id, -32_602, "invalid params");
          } else {
            this.protocolFailure(`ACP envelope rejected: ${decoded.reason}`);
          }
          return;
        }
      }
      if (carriesId) {
        if (!validId(message.id)) {
          this.protocolFailure("request id is invalid");
          return;
        }
        if (own(message, "result") || own(message, "error")) {
          this.protocolFailure("request carries result or error");
          return;
        }
        this.handleReverseRequest(message.method, message.id, message.params);
      } else {
        if (own(message, "result") || own(message, "error")) {
          this.protocolFailure("notification carries result or error");
          return;
        }
        this.handleNotification(message.method, message.params);
      }
      return;
    }
    if (!own(message, "id") || !validId(message.id)) {
      this.protocolFailure("response id is invalid");
      return;
    }
    const hasResult = own(message, "result");
    const hasError = own(message, "error");
    if (hasResult === hasError) {
      this.protocolFailure("response must carry exactly one of result or error");
      return;
    }
    if (
      hasError &&
      (!isMessage(message.error) ||
        !Number.isInteger(message.error.code) ||
        typeof message.error.message !== "string")
    ) {
      this.protocolFailure("error response is malformed");
      return;
    }
    const key = idKey(message.id);
    const pending = this.pending.get(key);
    if (pending === undefined || pending.generation !== this.generation) {
      this.countersValue.unknownOrLateResponses += 1;
      return;
    }
    if (this.stableClientMethods.has(pending.method)) {
      const decoded = decodeStableAcpJsonRpcEnvelope({
        direction: "client-to-agent",
        expectedMethod: pending.method,
        message,
      });
      if (decoded["_tag"] === "DecodeFailure") {
        this.protocolFailure(`ACP response rejected: ${decoded.reason}`);
        return;
      }
    }
    this.pending.delete(key);
    this.syncPressure();
    clearTimeout(pending.timer);
    pending.abortCleanup?.();
    this.countersValue.requestsCompleted += 1;
    this.recordRequestLatency(pending, "request_complete");
    if (hasError) {
      const error = message.error as { code: number; message: string };
      pending.reject(
        new AgentStdioTransportError("remote_error", redactText(error.message), error.code),
      );
    } else {
      pending.resolve(message.result);
    }
  }

  private handleNotification(method: string, params: unknown): void {
    const now = Date.now();
    if (now - this.notificationWindowAt >= 1_000) {
      this.notificationWindowAt = now;
      this.notificationsInWindow = 0;
    }
    this.notificationsInWindow += 1;
    if (this.notificationsInWindow > this.limits.maxNotificationsPerSecond) {
      this.countersValue.overloads += 1;
      this.protocolFailure("notification rate limit exceeded");
      return;
    }
    this.countersValue.notifications += 1;
    if (method === "$/cancel_request" && isMessage(params) && validId(params.requestId)) {
      this.reverseActive.get(idKey(params.requestId))?.controller.abort();
      return;
    }
    for (const handler of this.notificationHandlers.get(method) ?? []) {
      queueMicrotask(() => {
        try {
          handler(params);
        } catch {
          // Notification consumers are isolated from the protocol reader.
        }
      });
    }
  }

  private handleReverseRequest(method: string, id: string | number | null, params: unknown): void {
    const key = idKey(id);
    if (this.reverseActive.has(key)) {
      this.countersValue.duplicateIds += 1;
      this.sendError(id, -32_600, "duplicate request id");
      return;
    }
    if (this.reverseActive.size >= this.limits.maxReverseConcurrency) {
      this.countersValue.overloads += 1;
      this.trace("overload", method);
      this.sendError(id, -32_005, "reverse request overloaded");
      return;
    }
    const handler = this.reverseHandlers.get(method);
    if (handler === undefined) {
      this.sendError(id, -32_601, "method not found");
      return;
    }
    this.countersValue.reverseRequests += 1;
    const controller = new AbortController();
    const timer = setTimeout(() => {
      const active = this.reverseActive.get(key);
      if (active === undefined) return;
      this.reverseActive.delete(key);
      this.syncPressure();
      controller.abort();
      this.countersValue.reverseTimeouts += 1;
      this.recordReverseLatency(active, "reverse_timeout");
      this.sendError(id, -32_001, "reverse request timed out");
    }, this.limits.reverseRequestTimeoutMs);
    this.reverseActive.set(key, { controller, timer, startedAt: Date.now(), method });
    this.syncPressure();
    void Promise.resolve()
      .then(() =>
        handler(params, {
          method,
          requestId: id,
          signal: controller.signal,
          generation: this.generation,
        }),
      )
      .then((result) => {
        const active = this.reverseActive.get(key);
        if (active === undefined) return;
        this.reverseActive.delete(key);
        this.syncPressure();
        clearTimeout(timer);
        this.recordReverseLatency(active, "reverse_complete");
        this.sendEnvelopeSafely({ jsonrpc: "2.0", id, result: result ?? null });
      })
      .catch((error: unknown) => {
        const active = this.reverseActive.get(key);
        if (active === undefined) return;
        this.reverseActive.delete(key);
        this.syncPressure();
        clearTimeout(timer);
        this.recordReverseLatency(active, "reverse_complete");
        if (error instanceof AgentStdioHandlerError)
          this.sendError(id, error.code, redactText(error.message), error.data);
        else if (controller.signal.aborted)
          this.sendError(id, -32_800, "reverse request cancelled");
        else this.sendError(id, -32_603, "reverse request failed");
      });
  }

  private sendError(
    id: string | number | null,
    code: number,
    message: string,
    data?: unknown,
  ): void {
    this.sendEnvelopeSafely({
      jsonrpc: "2.0",
      id,
      error: { code, message, ...(data === undefined ? {} : { data }) },
    });
  }

  private sendEnvelopeSafely(envelope: Message): void {
    try {
      this.enqueueEnvelope(envelope);
    } catch {
      this.protocolFailure("outbound response queue failed");
    }
  }

  private consumeStderr(chunk: Buffer): void {
    this.countersValue.stderrBytes += chunk.length;
    const remaining = Math.max(0, this.limits.maxStderrBytes - this.stderrRaw.length);
    if (remaining === 0) {
      this.countersValue.stderrDroppedBytes += chunk.length;
      return;
    }
    const retained = chunk.subarray(0, remaining);
    this.stderrRaw = Buffer.concat([this.stderrRaw, retained]);
    this.countersValue.stderrDroppedBytes += chunk.length - retained.length;
  }

  private recordEvidence(
    direction: "inbound" | "outbound",
    raw: unknown,
    knownBytes?: number,
  ): void {
    const { bytes, sha256 } = jsonBytes(raw);
    const size = knownBytes ?? bytes;
    const entry: AgentStdioNativeEvidence = {
      generation: this.generation,
      direction,
      at: new Date().toISOString(),
      bytes: size,
      sha256,
      ...(this.evidenceBytes + size <= this.limits.maxEvidenceBytes
        ? { raw: structuredClone(raw) }
        : {}),
    };
    this.evidence.push(entry);
    if (entry.raw !== undefined) this.evidenceBytes += size;
    while (this.evidence.length > this.limits.maxEvidenceEntries) {
      const removed = this.evidence.shift();
      if (removed?.raw !== undefined) this.evidenceBytes -= removed.bytes;
    }
  }

  private syncPressure(): void {
    this.countersValue.currentInFlight = this.pending.size;
    this.countersValue.peakInFlight = Math.max(this.countersValue.peakInFlight, this.pending.size);
    this.countersValue.currentReverse = this.reverseActive.size;
    this.countersValue.peakReverse = Math.max(
      this.countersValue.peakReverse,
      this.reverseActive.size,
    );
    this.countersValue.currentInboundQueue = this.inboundQueue.length;
    this.countersValue.peakInboundQueue = Math.max(
      this.countersValue.peakInboundQueue,
      this.inboundQueue.length,
    );
    this.countersValue.currentOutboundQueue = this.outboundQueue.length;
    this.countersValue.peakOutboundQueue = Math.max(
      this.countersValue.peakOutboundQueue,
      this.outboundQueue.length,
    );
  }

  private recordRequestLatency(
    pending: Pending,
    kind: "request_complete" | "request_timeout" | "request_cancel",
  ): void {
    const durationMs = Date.now() - pending.startedAt;
    this.countersValue.requestLatencyMsTotal += durationMs;
    this.countersValue.requestLatencyMsMax = Math.max(
      this.countersValue.requestLatencyMsMax,
      durationMs,
    );
    this.trace(kind, pending.method, durationMs);
  }

  private recordReverseLatency(
    active: ReverseActive,
    kind: "reverse_complete" | "reverse_timeout",
  ): void {
    const durationMs = Date.now() - active.startedAt;
    this.countersValue.reverseLatencyMsTotal += durationMs;
    this.countersValue.reverseLatencyMsMax = Math.max(
      this.countersValue.reverseLatencyMsMax,
      durationMs,
    );
    this.trace(kind, active.method, durationMs);
  }

  private trace(
    kind: AgentStdioTransportTrace["kind"],
    method?: string,
    durationMs?: number,
  ): void {
    this.traces.push({
      at: new Date().toISOString(),
      generation: this.generation,
      kind,
      ...(method === undefined ? {} : { method }),
      ...(durationMs === undefined ? {} : { durationMs }),
    });
    while (this.traces.length > this.limits.maxEvidenceEntries) this.traces.shift();
  }

  private protocolFailure(detail: string): void {
    if (["failed", "disposed", "exited"].includes(this.stateValue)) return;
    this.countersValue.protocolViolations += 1;
    this.trace("protocol_violation");
    this.stateValue = "failed";
    this.terminalOutcome = "protocol_violation";
    this.endedAt = new Date().toISOString();
    this.inboundQueue = [];
    this.outboundQueue = [];
    this.syncPressure();
    this.abortReverseRequests();
    this.rejectPending(new AgentStdioTransportError("protocol_violation", detail));
    this.child?.kill("SIGTERM");
  }

  private processFailure(detail: string): void {
    if (["failed", "disposed", "exited"].includes(this.stateValue)) return;
    this.stateValue = "failed";
    this.terminalOutcome = "crash";
    this.endedAt = new Date().toISOString();
    this.inboundQueue = [];
    this.abortReverseRequests();
    this.rejectPending(new AgentStdioTransportError("process_exit", detail));
    this.child?.kill("SIGTERM");
  }

  private rejectPending(error: Error): void {
    for (const [key, pending] of this.pending) {
      this.pending.delete(key);
      clearTimeout(pending.timer);
      pending.abortCleanup?.();
      pending.reject(error);
    }
    this.syncPressure();
  }

  private abortReverseRequests(): void {
    for (const active of this.reverseActive.values()) {
      clearTimeout(active.timer);
      active.controller.abort();
    }
    this.reverseActive.clear();
    this.syncPressure();
  }

  private handleClose(code: number | null, signal: NodeJS.Signals | null): void {
    this.exitCode = code;
    this.signal = signal;
    this.endedAt ??= new Date().toISOString();
    if (this.terminalOutcome === null) {
      this.terminalOutcome = code === 0 ? "clean_exit" : "crash";
    }
    if (this.stateValue !== "failed" && this.stateValue !== "disposed") {
      this.stateValue = code === 0 ? "exited" : "failed";
    }
    this.rejectPending(
      new AgentStdioTransportError(
        "process_exit",
        `agent process exited (${code ?? signal ?? "unknown"})`,
      ),
    );
    this.abortReverseRequests();
    this.closeResolve?.();
    this.closeResolve = null;
  }

  private async waitForClose(timeoutMs: number): Promise<boolean> {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const timedOut = new Promise<false>((resolveTimeout) => {
      timer = setTimeout(() => resolveTimeout(false), timeoutMs);
    });
    const closed = this.closePromise.then(() => true);
    const result = await Promise.race([closed, timedOut]);
    if (timer !== undefined) clearTimeout(timer);
    return result;
  }
}

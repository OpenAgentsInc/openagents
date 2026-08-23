import { Effect, Layer, Schema } from "effect";
import * as Context from "effect/Context";
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { resolve } from "node:path";
import { StringDecoder } from "node:string_decoder";

import type { AgentConfigEntry } from "./computer-config.js";
import { defaultCuratedExecute, type Tier, withinRoot } from "./computer-policy.js";
import { VERSION } from "./version.js";

export type AgentCatalogSource = "local" | "configured" | "remote";

export interface AgentCatalogEntry {
  readonly id: string;
  readonly argv: ReadonlyArray<string>;
  readonly source: AgentCatalogSource;
  readonly version: string;
  readonly env: ReadonlyArray<string>;
}

export interface AcpAgentInventoryEntry {
  readonly id: string;
  readonly source: AgentCatalogSource;
  readonly version: string;
}

export type AgentRemoteResolver = (requestedId: string) => AgentCatalogEntry | undefined;

export const buildAgentCatalog = (
  config: ComputerConfigurationValuesLike,
  discovered: ReadonlyArray<{
    readonly name: string;
    readonly present: boolean;
    readonly version: string;
  }>,
): ReadonlyArray<AgentCatalogEntry> => {
  const local: Array<AgentCatalogEntry> = discovered
    .filter((tool) => tool.present && tool.name === "opencode")
    .map((tool) => ({
      id: "opencode",
      argv: ["opencode", "acp"],
      source: "local" as const,
      version: tool.version,
      env: [],
    }));
  const configured: Array<AgentCatalogEntry> = (config.agents ?? []).map((entry) => ({
    id: entry.id,
    argv: entry.argv,
    source: "configured" as const,
    version: "",
    env: entry.env,
  }));
  const byId = new Map(local.map((entry) => [entry.id, entry]));
  for (const entry of configured) byId.set(entry.id, entry);
  // Remote resolution is deliberately not performed here. The flag is retained
  // in the catalog input so a future resolver can remain explicitly opt-in.
  void config.registryAgents;
  return [...byId.values()].slice(0, 64);
};

interface ComputerConfigurationValuesLike {
  readonly agents?: ReadonlyArray<AgentConfigEntry>;
  readonly registryAgents?: boolean;
}

export const resolveAgent = (
  catalog: ReadonlyArray<AgentCatalogEntry>,
  requestedId: string,
  options: Readonly<{
    readonly registryAgents?: boolean;
    readonly resolveRemote?: AgentRemoteResolver;
  }> = {},
):
  | { readonly _tag: "resolved"; readonly entry: AgentCatalogEntry }
  | {
      readonly _tag: "unavailable";
      readonly requestedId: string;
      readonly availableIds: ReadonlyArray<string>;
    } => {
  const entry =
    catalog.find((candidate) => candidate.id === requestedId) ??
    (options.registryAgents === true ? options.resolveRemote?.(requestedId) : undefined);
  return entry === undefined
    ? { _tag: "unavailable", requestedId, availableIds: catalog.map((candidate) => candidate.id) }
    : { _tag: "resolved", entry };
};

export interface PermissionQuery {
  readonly kind: string;
  readonly title: string;
  readonly rawInput: unknown;
}

export const permissionAllowed = (
  tier: Tier,
  query: PermissionQuery,
  roots: ReadonlyArray<string>,
  curatedExecute: ReadonlyArray<string> = defaultCuratedExecute,
  cwd = process.cwd(),
): boolean => {
  const input = record(query.rawInput);
  const material = [
    query.title,
    ...Object.values(input).filter((value): value is string => typeof value === "string"),
  ];
  if (
    material.some(
      (value) =>
        /(?:^|\s)(?:sudo|doas|su|chmod|chown|dd|shutdown|reboot|ssh-add|ssh-keygen|gpg|crontab|systemctl|launchctl|nc|telnet)(?:\s|$)/u.test(
          value,
        ) ||
        /(?:\.ssh|\.aws|\.gnupg|\.kube|\.netrc|\.npmrc|\.pypirc|\.git-credentials|id_rsa|id_ed25519|\.env|credentials\.json|Keychains)/iu.test(
          value,
        ),
    )
  )
    return false;
  if (tier === "probe") return false;
  if (tier === "shell") return true;
  if (["read", "search", "fetch", "think"].includes(query.kind)) return true;
  if (query.kind === "edit" || query.kind === "write") {
    const candidate = firstString(input, ["path", "file_path", "filePath", "file"]);
    return (
      candidate !== undefined && roots.some((root) => withinRoot(resolve(cwd, candidate), root))
    );
  }
  if (query.kind === "execute") {
    const command = firstString(input, ["command"]) ?? query.title;
    const segments = command.split(/&&|\|\||;|\||\n/u);
    return (
      command !== "" &&
      segments.every((segment) => {
        const first = segment.trim().split(/\s+/u)[0] ?? "";
        return first === "cd" || curatedExecute.includes(first);
      })
    );
  }
  return false;
};

const record = (value: unknown): Record<string, unknown> => (isRecord(value) ? { ...value } : {});

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const firstString = (
  value: Record<string, unknown>,
  keys: ReadonlyArray<string>,
): string | undefined => {
  for (const key of keys) {
    const candidate = value[key];
    if (typeof candidate === "string" && candidate !== "") return candidate;
  }
  return undefined;
};

export interface AgentProcess {
  readonly request: (method: string, params: unknown) => Promise<unknown>;
  readonly notify: (method: string, params: unknown) => void;
  readonly onNotification: (method: string, handler: (params: unknown) => void) => () => void;
  readonly onRequest: (method: string, handler: AgentReverseHandler) => () => void;
  readonly terminate: (sessionId?: string) => Promise<void>;
}

type AgentReverseHandler = (
  params: unknown,
  context: Readonly<{
    method: string;
    requestId: string | number | null;
    signal: AbortSignal;
    generation: number;
    bindSession?(sessionId: string): boolean;
  }>,
) => unknown | Promise<unknown>;

export interface AgentProcessFactory {
  readonly start: (
    entry: AgentCatalogEntry,
    cwd: string,
    env: Readonly<Record<string, string>>,
  ) => Effect.Effect<AgentProcess, AgentProcessError>;
}

export class AgentProcessError extends Schema.TaggedErrorClass<AgentProcessError>()(
  "OpenAgentsCli.AgentProcessError",
  { message: Schema.String },
) {}

export class ComputerAgentProcess extends Context.Service<
  ComputerAgentProcess,
  AgentProcessFactory
>()("@openagentsinc/cli/ComputerAgentProcess") {}

const scrub = (value: string): string =>
  value
    .replaceAll(
      /(?:oa_(?:pat|agent|assignment)_[A-Za-z0-9._-]+|smct_[A-Za-z0-9._-]+)/gu,
      "[REDACTED]",
    )
    .replaceAll(
      /(?:api[-_]?key|token|secret|password|authorization)\s*[=:]\s*\S+/giu,
      "[REDACTED]",
    );

export interface AgentDelegationRequest {
  readonly entry: AgentCatalogEntry;
  readonly prompt: string;
  readonly cwd: string;
  readonly resumeSessionId?: string;
  readonly tier: Tier;
  readonly roots: ReadonlyArray<string>;
  readonly curatedExecute: ReadonlyArray<string>;
  readonly env: Readonly<Record<string, string>>;
  readonly timeoutMs: number;
  readonly maximumOutputBytes: number;
  readonly onChunk: (text: string) => void;
  readonly onSession: (sessionId: string) => void;
  readonly onPermission: (allowed: boolean, detail: string) => void;
}

export interface AgentDelegationOutcome {
  readonly status: "completed" | "failed" | "cancelled" | "timeout" | "truncated";
  readonly sessionId: string;
  readonly output: string;
  readonly truncated: boolean;
  readonly detail: string;
  readonly durationMs: number;
}

export interface AgentDelegationJob {
  readonly done: Promise<AgentDelegationOutcome>;
  readonly cancel: () => void;
}

interface ResolverSlot<T> {
  resolve: (value: T) => void;
}

const captureResolver =
  <T>(slot: ResolverSlot<T>) =>
  (resolver: (value: T) => void): void => {
    slot.resolve = resolver;
  };

const deferred = <T>(): {
  readonly promise: Promise<T>;
  readonly resolve: (value: T) => void;
} => {
  const slot: ResolverSlot<T> = { resolve: () => undefined };
  const promise = new Promise<T>(captureResolver(slot));
  return { promise, resolve: slot.resolve };
};

export const startAgentDelegation = (
  factory: AgentProcessFactory,
  request: AgentDelegationRequest,
): AgentDelegationJob => {
  const startedAt = Date.now();
  let process: AgentProcess | undefined;
  let sessionId = request.resumeSessionId ?? "";
  let settled = false;
  let cancelled = false;
  let timedOut = false;
  let bytes = 0;
  let output = "";
  let truncated = false;
  const completion = deferred<AgentDelegationOutcome>();
  const done = completion.promise;
  const settle = (status: AgentDelegationOutcome["status"], detail: string): void => {
    if (settled) return;
    settled = true;
    completion.resolve({
      status,
      sessionId,
      output,
      truncated,
      detail,
      durationMs: Date.now() - startedAt,
    });
    void process?.terminate(sessionId);
  };
  const emit = (value: string): void => {
    const safe = scrub(value);
    const remaining = request.maximumOutputBytes - bytes;
    if (remaining <= 0) {
      truncated = true;
      return;
    }
    const bounded = safe.slice(0, remaining);
    bytes += bounded.length;
    output += bounded;
    if (bounded.length < safe.length) truncated = true;
    request.onChunk(bounded);
  };
  const run = async (): Promise<void> => {
    process = await Effect.runPromise(factory.start(request.entry, request.cwd, request.env));
    process.onNotification("session/update", (params) => {
      const text = extractText(params);
      if (text !== "") emit(text);
    });
    process.onRequest("session/request_permission", async (params: unknown) => {
      const value = record(params);
      const toolCall = record(value.toolCall);
      const kind = typeof toolCall.kind === "string" ? toolCall.kind : "other";
      const title = typeof toolCall.title === "string" ? toolCall.title : "";
      const allowed = permissionAllowed(
        request.tier,
        { kind, title, rawInput: toolCall.rawInput },
        request.roots,
        request.curatedExecute,
        request.cwd,
      );
      const options = Array.isArray(value.options) ? value.options : [];
      const option = options.find((candidate) => {
        const item = record(candidate);
        const optionKind = item.kind;
        return optionKind === (allowed ? "allow_once" : "reject_once");
      });
      const detail = `${kind}${title === "" ? "" : `: ${scrub(title)}`}`;
      request.onPermission(allowed, detail);
      return option === undefined
        ? { outcome: { outcome: "cancelled" } }
        : { outcome: { outcome: "selected", optionId: String(record(option).optionId ?? "") } };
    });
    const initialized = record(
      await process.request("initialize", {
        protocolVersion: 1,
        clientCapabilities: { fs: { readTextFile: false, writeTextFile: false }, terminal: false },
        clientInfo: { name: "openagents-cli", title: "OpenAgents CLI", version: VERSION },
      }),
    );
    if (initialized.protocolVersion !== undefined && initialized.protocolVersion !== 1) {
      settle(
        "failed",
        `the agent negotiated unsupported ACP protocol version ${String(initialized.protocolVersion)}`,
      );
      return;
    }
    const capabilities = record(initialized.agentCapabilities);
    if (request.resumeSessionId !== undefined) {
      if (capabilities.loadSession !== true) {
        settle("failed", "the agent cannot reattach the requested ACP session");
        return;
      }
      await process.request("session/load", {
        sessionId: request.resumeSessionId,
        cwd: request.cwd,
        mcpServers: [],
      });
    } else {
      const created = record(
        await process.request("session/new", { cwd: request.cwd, mcpServers: [] }),
      );
      if (typeof created.sessionId !== "string" || created.sessionId === "") {
        settle("failed", "the agent did not return an ACP session id");
        return;
      }
      sessionId = created.sessionId;
      request.onSession(sessionId);
    }
    const result = record(
      await process.request("session/prompt", {
        sessionId,
        prompt: [{ type: "text", text: request.prompt.slice(0, 32_768) }],
      }),
    );
    const stopReason = result.stopReason;
    settle(
      stopReason === "cancelled"
        ? timedOut
          ? "timeout"
          : "cancelled"
        : stopReason === "refusal"
          ? "failed"
          : truncated
            ? "truncated"
            : "completed",
      "",
    );
  };
  void run().catch((cause: unknown) =>
    settle(
      cancelled ? "cancelled" : timedOut ? "timeout" : "failed",
      scrub(cause instanceof Error ? cause.message : String(cause)),
    ),
  );
  const timer = setTimeout(() => {
    timedOut = true;
    process?.notify("session/cancel", { sessionId });
    setTimeout(() => settle("timeout", "delegation timed out"), 1_000).unref();
  }, request.timeoutMs);
  timer.unref();
  done.finally(() => clearTimeout(timer));
  return {
    done,
    cancel: () => {
      cancelled = true;
      process?.notify("session/cancel", { sessionId });
      setTimeout(() => settle("cancelled", "cancelled by request"), 1_000).unref();
    },
  };
};

const extractText = (value: unknown): string => {
  const texts: string[] = [];
  const visit = (current: unknown): void => {
    if (typeof current === "string") return;
    if (Array.isArray(current)) {
      for (const item of current) visit(item);
      return;
    }
    if (typeof current !== "object" || current === null) return;
    const object = current as Record<string, unknown>;
    if (typeof object.text === "string") texts.push(object.text);
    for (const [key, item] of Object.entries(object)) if (key !== "text") visit(item);
  };
  visit(value);
  return texts.join("");
};

const maximumAgentLineBytes = 1_048_576;
const maximumAgentBufferedBytes = 2_097_152;
const agentRequestTimeoutMs = 60_000;
const agentShutdownGraceMs = 1_000;

type AgentRpcId = string | number | null;

interface PendingAgentRequest {
  readonly resolve: (value: unknown) => void;
  readonly reject: (error: Error) => void;
  readonly timer: ReturnType<typeof setTimeout>;
}

const agentRpcIdKey = (id: AgentRpcId): string => `${typeof id}:${String(id)}`;

const isAgentRpcId = (value: unknown): value is AgentRpcId =>
  value === null ||
  typeof value === "string" ||
  (typeof value === "number" && Number.isSafeInteger(value));

const isAgentRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const writeAgentMessage = (
  child: ChildProcessWithoutNullStreams,
  message: Record<string, unknown>,
): boolean => {
  if (child.stdin.destroyed || child.stdin.writableEnded) return false;
  const line = `${JSON.stringify(message)}\n`;
  if (Buffer.byteLength(line, "utf8") > maximumAgentLineBytes) return false;
  try {
    child.stdin.write(line);
    return true;
  } catch {
    return false;
  }
};

const killAgentProcess = (child: ChildProcessWithoutNullStreams, signal: NodeJS.Signals): void => {
  if (child.pid === undefined) return;
  try {
    if (process.platform === "win32") child.kill(signal);
    else process.kill(-child.pid, signal);
  } catch {
    try {
      child.kill(signal);
    } catch {
      // The process may have exited between the two kill attempts.
    }
  }
};

const startLocalAgentProcess = async (
  entry: AgentCatalogEntry,
  cwd: string,
  env: Readonly<Record<string, string>>,
): Promise<AgentProcess> => {
  const executable = entry.argv[0] ?? "";
  const child = spawn(executable, entry.argv.slice(1), {
    cwd,
    env: { ...env },
    shell: false,
    detached: process.platform !== "win32",
    stdio: ["pipe", "pipe", "pipe"],
  });
  const pending = new Map<string, PendingAgentRequest>();
  const notificationHandlers = new Map<string, Set<(params: unknown) => void>>();
  const requestHandlers = new Map<string, AgentReverseHandler>();
  const decoder = new StringDecoder("utf8");
  let lineBuffer = "";
  let disposed = false;
  let nextRequestId = 0;
  let closeResolve: (() => void) | undefined;
  const closed = new Promise<void>((resolveClosed) => {
    closeResolve = resolveClosed;
  });

  const rejectPending = (message: string): void => {
    for (const request of pending.values()) {
      clearTimeout(request.timer);
      request.reject(new Error(message));
    }
    pending.clear();
  };

  const dispose = (message: string): void => {
    if (disposed) return;
    disposed = true;
    rejectPending(message);
    closeResolve?.();
  };

  const sendResponse = (id: AgentRpcId, result: unknown): void => {
    writeAgentMessage(child, { jsonrpc: "2.0", id, result });
  };

  const sendError = (id: AgentRpcId, code: number, message: string): void => {
    writeAgentMessage(child, {
      jsonrpc: "2.0",
      id,
      error: { code, message },
    });
  };

  const handleReverseRequest = (message: Record<string, unknown>): void => {
    const method = typeof message.method === "string" ? message.method : "";
    const id = isAgentRpcId(message.id) ? message.id : null;
    const handler = requestHandlers.get(method);
    if (handler === undefined) {
      sendError(id, -32601, "method not found");
      return;
    }
    const controller = new AbortController();
    Promise.resolve(
      handler(message.params, {
        method,
        requestId: id,
        signal: controller.signal,
        generation: 1,
        bindSession: () => true,
      }),
    )
      .then((result) => sendResponse(id, result))
      .catch(() => sendError(id, -32000, "request refused"));
  };

  const handleMessage = (message: unknown): void => {
    if (!isAgentRecord(message)) {
      dispose("ACP process returned an invalid message");
      killAgentProcess(child, "SIGTERM");
      return;
    }
    if (typeof message.method === "string") {
      if (Object.hasOwn(message, "id")) handleReverseRequest(message);
      else {
        for (const handler of notificationHandlers.get(message.method) ?? []) {
          handler(message.params);
        }
      }
      return;
    }
    if (!Object.hasOwn(message, "id") || !isAgentRpcId(message.id)) {
      dispose("ACP process returned an invalid response");
      killAgentProcess(child, "SIGTERM");
      return;
    }
    const request = pending.get(agentRpcIdKey(message.id));
    if (request === undefined) return;
    pending.delete(agentRpcIdKey(message.id));
    clearTimeout(request.timer);
    if (isAgentRecord(message.error)) request.reject(new Error("ACP request failed"));
    else request.resolve(message.result);
  };

  const handleStdout = (chunk: Buffer | string): void => {
    if (disposed) return;
    lineBuffer += decoder.write(chunk);
    if (Buffer.byteLength(lineBuffer, "utf8") > maximumAgentBufferedBytes) {
      dispose("ACP process output exceeded the buffer limit");
      killAgentProcess(child, "SIGTERM");
      return;
    }
    let newline = lineBuffer.indexOf("\n");
    while (newline >= 0) {
      const line = lineBuffer.slice(0, newline).replace(/\r$/u, "");
      lineBuffer = lineBuffer.slice(newline + 1);
      if (Buffer.byteLength(line, "utf8") > maximumAgentLineBytes) {
        dispose("ACP process line exceeded the size limit");
        killAgentProcess(child, "SIGTERM");
        return;
      }
      try {
        handleMessage(JSON.parse(line));
      } catch {
        dispose("ACP process returned malformed JSON");
        killAgentProcess(child, "SIGTERM");
        return;
      }
      newline = lineBuffer.indexOf("\n");
    }
  };

  child.stdout.on("data", handleStdout);
  child.stderr.on("data", () => undefined);
  child.on("error", () => dispose("ACP process failed"));
  child.on("close", () => {
    dispose("ACP process exited");
    closeResolve?.();
  });

  await new Promise<void>((resolveStarted, rejectStarted) => {
    const onSpawn = () => {
      child.off("error", onError);
      resolveStarted();
    };
    const onError = (cause: Error) => {
      child.off("spawn", onSpawn);
      rejectStarted(cause);
    };
    child.once("spawn", onSpawn);
    child.once("error", onError);
  });

  const request = (method: string, params: unknown): Promise<unknown> => {
    if (disposed) return Promise.reject(new Error("ACP process is not running"));
    const id = ++nextRequestId;
    const key = agentRpcIdKey(id);
    return new Promise((resolveRequest, rejectRequest) => {
      const timer = setTimeout(() => {
        pending.delete(key);
        rejectRequest(new Error("ACP request timed out"));
      }, agentRequestTimeoutMs);
      timer.unref();
      pending.set(key, { resolve: resolveRequest, reject: rejectRequest, timer });
      if (!writeAgentMessage(child, { jsonrpc: "2.0", id, method, params })) {
        clearTimeout(timer);
        pending.delete(key);
        rejectRequest(new Error("ACP process is not writable"));
      }
    });
  };

  const notify = (method: string, params: unknown): void => {
    writeAgentMessage(child, { jsonrpc: "2.0", method, params });
  };

  const terminate = async (): Promise<void> => {
    if (disposed) return;
    disposed = true;
    rejectPending("ACP process terminated");
    child.stdin.end();
    await Promise.race([
      closed,
      new Promise<void>((resolveTimeout) => {
        const timer = setTimeout(resolveTimeout, agentShutdownGraceMs);
        timer.unref();
      }),
    ]);
    if (!child.killed) killAgentProcess(child, "SIGTERM");
  };

  return {
    request,
    notify,
    onNotification: (method, handler) => {
      const handlers = notificationHandlers.get(method) ?? new Set();
      handlers.add(handler);
      notificationHandlers.set(method, handlers);
      return () => {
        handlers.delete(handler);
        if (handlers.size === 0) notificationHandlers.delete(method);
      };
    },
    onRequest: (method, handler) => {
      requestHandlers.set(method, handler);
      return () => {
        if (requestHandlers.get(method) === handler) requestHandlers.delete(method);
      };
    },
    terminate,
  };
};

export const computerAgentProcessNodeLayer = Layer.succeed(ComputerAgentProcess, {
  start: (entry, cwd, env) =>
    Effect.tryPromise({
      try: () => startLocalAgentProcess(entry, cwd, env),
      catch: (cause) =>
        new AgentProcessError({
          message: cause instanceof Error ? cause.message : String(cause),
        }),
    }),
});

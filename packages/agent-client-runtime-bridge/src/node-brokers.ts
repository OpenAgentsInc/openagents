import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { randomUUID } from "node:crypto";
import { constants } from "node:fs";
import { access, lstat, open, readFile, realpath, rename, rm, stat } from "node:fs/promises";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";

import type {
  CreateTerminalRequest,
  EnvVariable,
  TerminalExitStatus,
} from "@openagentsinc/agent-client-protocol/stable";

import type {
  AcpAuthorityHealth,
  AcpAuthorityLease,
  AcpFilesystemBrokerPort,
  AcpTerminalBrokerPort,
} from "./authority.js";

export type NodeBrokerFaultCode =
  | "aborted"
  | "outside_workspace"
  | "invalid_path"
  | "invalid_utf8"
  | "byte_limit"
  | "not_regular_file"
  | "terminal_denied"
  | "terminal_not_found"
  | "terminal_not_owned"
  | "terminal_running"
  | "terminal_released";

const faultMessage: Readonly<Record<NodeBrokerFaultCode, string>> = {
  aborted: "The broker operation was cancelled.",
  outside_workspace: "The requested path is outside the workspace.",
  invalid_path: "The requested path is unavailable.",
  invalid_utf8: "The requested file is not valid UTF-8 text.",
  byte_limit: "The requested data exceeds the configured byte limit.",
  not_regular_file: "The requested path is not a regular file.",
  terminal_denied: "The terminal command was denied by policy.",
  terminal_not_found: "The terminal is unavailable.",
  terminal_not_owned: "The terminal belongs to another authority lease.",
  terminal_running: "The terminal is still running.",
  terminal_released: "The terminal has already been released.",
};

export class NodeBrokerFault extends Error {
  override readonly name = "NodeBrokerFault";

  constructor(readonly code: NodeBrokerFaultCode) {
    super(faultMessage[code]);
  }
}

const throwIfAborted = (signal: AbortSignal | undefined): void => {
  if (signal?.aborted === true) throw new NodeBrokerFault("aborted");
};

const isInside = (root: string, candidate: string): boolean => {
  const child = relative(root, candidate);
  return child === "" || (!child.startsWith(`..${sep}`) && child !== ".." && !isAbsolute(child));
};

const lexicalCandidate = (root: string, rawPath: string, requestedRoot = root): string => {
  if (rawPath.trim().length === 0) throw new NodeBrokerFault("invalid_path");
  const requested = resolve(isAbsolute(rawPath) ? rawPath : resolve(root, rawPath));
  const candidate =
    isAbsolute(rawPath) && isInside(requestedRoot, requested)
      ? resolve(root, relative(requestedRoot, requested))
      : requested;
  if (!isInside(root, candidate)) throw new NodeBrokerFault("outside_workspace");
  return candidate;
};

const containedRealpath = async (root: string, candidate: string): Promise<string> => {
  let target: string;
  try {
    target = await realpath(candidate);
  } catch {
    throw new NodeBrokerFault("invalid_path");
  }
  if (!isInside(root, target)) throw new NodeBrokerFault("outside_workspace");
  return target;
};

const regularFile = async (path: string): Promise<Awaited<ReturnType<typeof stat>>> => {
  const info = await stat(path);
  if (!info.isFile()) throw new NodeBrokerFault("not_regular_file");
  return info;
};

const safeRef = (value: string): string =>
  /^[A-Za-z0-9][A-Za-z0-9._:-]{0,255}$/.test(value) ? value : "evidence.node_broker.redacted";

const evidence = <Operation extends string>(
  nextEvidenceRef: ((operation: Operation) => string) | undefined,
  operation: Operation,
): ReadonlyArray<string> =>
  Object.freeze([safeRef(nextEvidenceRef?.(operation) ?? `evidence.node_broker.${operation}`)]);

export type NodeFilesystemBrokerOptions = Readonly<{
  workspaceRoot: string;
  maxReadBytes?: number;
  maxWriteBytes?: number;
  maxReadLines?: number;
  nextEvidenceRef?: (operation: "read" | "write") => string;
}>;

export const createNodeFilesystemBroker = async (
  options: NodeFilesystemBrokerOptions,
): Promise<AcpFilesystemBrokerPort> => {
  const requestedRoot = resolve(options.workspaceRoot);
  const root = await realpath(requestedRoot);
  const rootInfo = await stat(root);
  if (!rootInfo.isDirectory()) throw new NodeBrokerFault("invalid_path");
  const maxReadBytes = options.maxReadBytes ?? 2 * 1024 * 1024;
  const maxWriteBytes = options.maxWriteBytes ?? 2 * 1024 * 1024;
  const maxReadLines = options.maxReadLines ?? 20_000;
  const health = async (): Promise<AcpAuthorityHealth> => {
    try {
      await access(root, constants.R_OK | constants.W_OK);
      return "healthy";
    } catch {
      return "unhealthy";
    }
  };

  const broker: AcpFilesystemBrokerPort = {
    health,
    async readTextFile(request, lease) {
      throwIfAborted(lease.signal);
      const candidate = lexicalCandidate(root, request.path, requestedRoot);
      const target = await containedRealpath(root, candidate);
      const info = await regularFile(target);
      if (info.size > maxReadBytes) throw new NodeBrokerFault("byte_limit");
      const bytes = await readFile(target, { signal: lease.signal });
      throwIfAborted(lease.signal);
      if (bytes.byteLength > maxReadBytes) throw new NodeBrokerFault("byte_limit");
      let text: string;
      try {
        text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
      } catch {
        throw new NodeBrokerFault("invalid_utf8");
      }
      const start = Math.max(0, (request.line ?? 1) - 1);
      const requestedLimit = request.limit ?? maxReadLines;
      const limit = Math.min(requestedLimit, maxReadLines);
      const content = text
        .split(/\r?\n/)
        .slice(start, start + limit)
        .join("\n");
      return {
        value: { content },
        evidenceRefs: evidence(options.nextEvidenceRef, "read"),
      };
    },
    async writeTextFile(request, lease) {
      throwIfAborted(lease.signal);
      const bytes = Buffer.from(request.content, "utf8");
      if (bytes.byteLength > maxWriteBytes) throw new NodeBrokerFault("byte_limit");
      const candidate = lexicalCandidate(root, request.path, requestedRoot);
      const parentCandidate = dirname(candidate);
      const parent = await containedRealpath(root, parentCandidate);
      const parentInfo = await stat(parent);
      if (!parentInfo.isDirectory()) throw new NodeBrokerFault("invalid_path");

      let target = resolve(parent, relative(parentCandidate, candidate));
      try {
        const existing = await realpath(candidate);
        if (!isInside(root, existing)) throw new NodeBrokerFault("outside_workspace");
        await regularFile(existing);
        target = existing;
      } catch (error) {
        if (error instanceof NodeBrokerFault) throw error;
        try {
          const link = await lstat(candidate);
          if (link.isSymbolicLink()) throw new NodeBrokerFault("invalid_path");
        } catch (linkError) {
          if (linkError instanceof NodeBrokerFault) throw linkError;
        }
      }
      if (!isInside(root, target) || dirname(target) !== parent)
        throw new NodeBrokerFault("outside_workspace");

      const temporary = resolve(parent, `.openagents-acp-${randomUUID()}.tmp`);
      let handle: Awaited<ReturnType<typeof open>> | undefined;
      try {
        throwIfAborted(lease.signal);
        handle = await open(temporary, "wx", 0o600);
        await handle.writeFile(bytes);
        await handle.sync();
        await handle.close();
        handle = undefined;
        throwIfAborted(lease.signal);
        if ((await realpath(parentCandidate)) !== parent)
          throw new NodeBrokerFault("outside_workspace");
        await rename(temporary, target);
      } finally {
        await handle?.close().catch(() => undefined);
        await rm(temporary, { force: true }).catch(() => undefined);
      }
      return { value: {}, evidenceRefs: evidence(options.nextEvidenceRef, "write") };
    },
  };
  return Object.freeze(broker);
};

export type NodeTerminalPolicyInput = Readonly<{
  executable: string;
  args: ReadonlyArray<string>;
  cwdRef: string;
  envNames: ReadonlyArray<string>;
  sessionId: string;
  generation: number;
  scopeRef: string;
}>;

export type NodeTerminalBrokerOptions = Readonly<{
  workspaceRoot: string;
  allow: (input: NodeTerminalPolicyInput) => boolean | Promise<boolean>;
  allowedEnvNames?: ReadonlyArray<string>;
  baseEnv?: Readonly<Record<string, string>>;
  maxOutputBytes?: number;
  redactOutput?: (output: string) => string;
  nextTerminalId?: () => string;
  nextEvidenceRef?: (operation: "create" | "output" | "wait" | "kill" | "release") => string;
}>;

type OwnedTerminal = {
  readonly terminalId: string;
  readonly child: ChildProcessWithoutNullStreams;
  readonly sessionId: string;
  readonly connectionRef: string;
  readonly generation: number;
  readonly scopeRef: string;
  readonly outputLimit: number;
  chunks: Buffer[];
  bytes: number;
  truncated: boolean;
  exitStatus?: TerminalExitStatus;
  released: boolean;
  done: Promise<TerminalExitStatus>;
  resolveDone: (status: TerminalExitStatus) => void;
  abortCleanup?: () => void;
};

export type NodeTerminalBrokerPort = AcpTerminalBrokerPort &
  Readonly<{
    dispose: () => Promise<void>;
    ownedTerminalCount: () => number;
  }>;

const sanitizeEnv = (
  requested: ReadonlyArray<EnvVariable> | null | undefined,
  allowedNames: ReadonlySet<string>,
  base: Readonly<Record<string, string>>,
): Readonly<Record<string, string>> => {
  const env: Record<string, string> = {};
  for (const [name, value] of Object.entries(base))
    if (allowedNames.has(name) && /^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) env[name] = value;
  for (const item of requested ?? [])
    if (
      item.name !== "PATH" &&
      allowedNames.has(item.name) &&
      /^[A-Za-z_][A-Za-z0-9_]*$/.test(item.name)
    )
      env[item.name] = item.value;
  return Object.freeze(env);
};

const appendBounded = (terminal: OwnedTerminal, chunk: Buffer): void => {
  terminal.chunks.push(chunk);
  terminal.bytes += chunk.byteLength;
  while (terminal.bytes > terminal.outputLimit && terminal.chunks.length > 0) {
    terminal.truncated = true;
    const excess = terminal.bytes - terminal.outputLimit;
    const first = terminal.chunks[0]!;
    if (first.byteLength <= excess) {
      terminal.chunks.shift();
      terminal.bytes -= first.byteLength;
    } else {
      terminal.chunks[0] = first.subarray(excess);
      terminal.bytes -= excess;
    }
  }
};

const outputText = (terminal: OwnedTerminal, redact: (output: string) => string): string => {
  const decoded = new TextDecoder("utf-8", { fatal: false }).decode(Buffer.concat(terminal.chunks));
  const redacted = redact(decoded);
  const bytes = Buffer.from(redacted, "utf8");
  if (bytes.byteLength <= terminal.outputLimit) return redacted;
  terminal.truncated = true;
  return new TextDecoder("utf-8", { fatal: false }).decode(
    bytes.subarray(bytes.byteLength - terminal.outputLimit),
  );
};

const owned = (terminal: OwnedTerminal, lease: AcpAuthorityLease): void => {
  if (terminal.released) throw new NodeBrokerFault("terminal_released");
  if (
    terminal.sessionId !== lease.sessionId ||
    terminal.connectionRef !== lease.connectionRef ||
    terminal.generation !== lease.generation ||
    terminal.scopeRef !== lease.scopeRef
  )
    throw new NodeBrokerFault("terminal_not_owned");
  throwIfAborted(lease.signal);
};

const waitForOwnedExit = (
  terminal: OwnedTerminal,
  signal: AbortSignal | undefined,
): Promise<TerminalExitStatus> => {
  if (terminal.exitStatus !== undefined) return Promise.resolve(terminal.exitStatus);
  if (signal === undefined) return terminal.done;
  return new Promise<TerminalExitStatus>((resolveWait, rejectWait) => {
    const cleanup = () => signal.removeEventListener("abort", abort);
    const abort = () => {
      cleanup();
      if (terminal.exitStatus === undefined) terminal.child.kill("SIGKILL");
      rejectWait(new NodeBrokerFault("aborted"));
    };
    signal.addEventListener("abort", abort, { once: true });
    void terminal.done.then(
      (status) => {
        cleanup();
        resolveWait(status);
      },
      () => {
        cleanup();
        rejectWait(new NodeBrokerFault("terminal_not_found"));
      },
    );
    if (signal.aborted) abort();
  });
};

export const createNodeTerminalBroker = async (
  options: NodeTerminalBrokerOptions,
): Promise<NodeTerminalBrokerPort> => {
  const requestedRoot = resolve(options.workspaceRoot);
  const root = await realpath(requestedRoot);
  const rootInfo = await stat(root);
  if (!rootInfo.isDirectory()) throw new NodeBrokerFault("invalid_path");
  const allowedEnvNames = new Set(options.allowedEnvNames ?? []);
  const baseEnv = options.baseEnv ?? {};
  const maxOutputBytes = options.maxOutputBytes ?? 256 * 1024;
  const redact =
    options.redactOutput ??
    ((output: string) =>
      output.replace(
        /(?:bearer\s+[A-Za-z0-9._-]+|(?:sk|xai|ghp|github_pat)[-_][A-Za-z0-9_-]{8,})/gi,
        "[redacted]",
      ));
  const terminals = new Map<string, OwnedTerminal>();
  let disposed = false;

  const find = (terminalId: string, lease: AcpAuthorityLease): OwnedTerminal => {
    const terminal = terminals.get(terminalId);
    if (terminal === undefined) throw new NodeBrokerFault("terminal_not_found");
    owned(terminal, lease);
    return terminal;
  };

  const finish = (terminal: OwnedTerminal, status: TerminalExitStatus): void => {
    if (terminal.exitStatus !== undefined) return;
    terminal.exitStatus = Object.freeze(status);
    terminal.abortCleanup?.();
    terminal.resolveDone(terminal.exitStatus);
  };

  const broker: NodeTerminalBrokerPort = {
    async health() {
      if (disposed) return "unhealthy";
      try {
        await access(root, constants.R_OK | constants.X_OK);
        return "healthy";
      } catch {
        return "unhealthy";
      }
    },
    async create(request: CreateTerminalRequest, lease: AcpAuthorityLease) {
      throwIfAborted(lease.signal);
      if (disposed) throw new NodeBrokerFault("terminal_released");
      const cwdCandidate = lexicalCandidate(root, request.cwd ?? root, requestedRoot);
      const cwd = await containedRealpath(root, cwdCandidate);
      if (!(await stat(cwd)).isDirectory()) throw new NodeBrokerFault("invalid_path");
      const args = Object.freeze([...(request.args ?? [])]);
      const env = sanitizeEnv(request.env, allowedEnvNames, baseEnv);
      const allowed = await options.allow({
        executable: request.command,
        args,
        cwdRef: safeRef(`cwd.${relative(root, cwd).replaceAll(sep, ".") || "root"}`),
        envNames: Object.freeze(Object.keys(env).sort()),
        sessionId: lease.sessionId,
        generation: lease.generation,
        scopeRef: lease.scopeRef,
      });
      if (!allowed) throw new NodeBrokerFault("terminal_denied");
      throwIfAborted(lease.signal);
      const terminalId = options.nextTerminalId?.() ?? `terminal.${randomUUID()}`;
      if (terminals.has(terminalId)) throw new NodeBrokerFault("terminal_denied");
      const child = spawn(request.command, args, {
        cwd,
        env: { ...env },
        stdio: ["pipe", "pipe", "pipe"],
        shell: false,
      });
      child.stdin.end();
      let resolveDone!: (status: TerminalExitStatus) => void;
      const done = new Promise<TerminalExitStatus>((resolveDonePromise) => {
        resolveDone = resolveDonePromise;
      });
      const terminal: OwnedTerminal = {
        terminalId,
        child,
        sessionId: lease.sessionId,
        connectionRef: lease.connectionRef,
        generation: lease.generation,
        scopeRef: lease.scopeRef,
        outputLimit: Math.min(request.outputByteLimit ?? maxOutputBytes, maxOutputBytes),
        chunks: [],
        bytes: 0,
        truncated: false,
        released: false,
        done,
        resolveDone,
      };
      terminals.set(terminalId, terminal);
      child.stdout.on("data", (chunk: Buffer) => appendBounded(terminal, chunk));
      child.stderr.on("data", (chunk: Buffer) => appendBounded(terminal, chunk));
      child.once("error", () => finish(terminal, { exitCode: null, signal: "spawn_error" }));
      child.once("exit", (code, signal) => finish(terminal, { exitCode: code, signal }));
      if (lease.signal !== undefined) {
        const abort = () => {
          if (terminal.exitStatus === undefined) terminal.child.kill("SIGKILL");
        };
        lease.signal.addEventListener("abort", abort, { once: true });
        terminal.abortCleanup = () => lease.signal?.removeEventListener("abort", abort);
        if (lease.signal.aborted) abort();
      }
      return {
        value: { terminalId },
        evidenceRefs: evidence(options.nextEvidenceRef, "create"),
      };
    },
    async output(request, lease) {
      const terminal = find(request.terminalId, lease);
      return {
        value: {
          output: outputText(terminal, redact),
          truncated: terminal.truncated,
          ...(terminal.exitStatus === undefined ? {} : { exitStatus: terminal.exitStatus }),
        },
        evidenceRefs: evidence(options.nextEvidenceRef, "output"),
      };
    },
    async waitForExit(request, lease) {
      const terminal = find(request.terminalId, lease);
      const status = await waitForOwnedExit(terminal, lease.signal);
      throwIfAborted(lease.signal);
      return { value: status, evidenceRefs: evidence(options.nextEvidenceRef, "wait") };
    },
    async kill(request, lease) {
      const terminal = find(request.terminalId, lease);
      if (terminal.exitStatus === undefined) terminal.child.kill("SIGKILL");
      return { value: {}, evidenceRefs: evidence(options.nextEvidenceRef, "kill") };
    },
    async release(request, lease) {
      const terminal = find(request.terminalId, lease);
      if (terminal.exitStatus === undefined) throw new NodeBrokerFault("terminal_running");
      terminal.released = true;
      terminals.delete(terminal.terminalId);
      terminal.chunks = [];
      terminal.bytes = 0;
      return { value: {}, evidenceRefs: evidence(options.nextEvidenceRef, "release") };
    },
    async dispose() {
      if (disposed) return;
      disposed = true;
      for (const terminal of terminals.values()) {
        terminal.abortCleanup?.();
        if (terminal.exitStatus === undefined) terminal.child.kill("SIGKILL");
        terminal.released = true;
        terminal.chunks = [];
      }
      await Promise.all([...terminals.values()].map((terminal) => terminal.done));
      terminals.clear();
    },
    ownedTerminalCount: () => terminals.size,
  };
  return Object.freeze(broker);
};

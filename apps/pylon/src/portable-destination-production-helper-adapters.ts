import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import { accessSync, constants, statSync, watch, type FSWatcher } from "node:fs";
import { pathToFileURL } from "node:url";

import {
  defaultKhalaProcessService,
  type KhalaProcessService,
  type KhalaProcessSessionResult,
} from "@openagentsinc/khala-tools";
import type { IdePortableDestinationHelperKind } from "@openagentsinc/portable-session-contract";
import { Effect } from "effect";

import type {
  PylonPortableDestinationHelperAdapter,
  PylonPortableDestinationHelperStartInput,
} from "./portable-destination-helper-supervisor.js";
import {
  PYLON_TYPESCRIPT_LSP_EXECUTABLE_PROFILE_REF,
  repositoryOwnedPylonPortableExecutableProfileCatalog,
} from "./portable-executable-profile-catalog.js";
import {
  type PylonPortableVerifiedExecutableProfile,
  verifyPylonPortableExecutableProfile,
} from "./portable-executable-profile-verifier.js";

const PYTHON_EXECUTABLE = "/usr/bin/python3";
const SHELL_EXECUTABLE = "/bin/sh";
const PTY_SESSION_TIMEOUT_MS = 7 * 24 * 60 * 60 * 1_000;
const PTY_CAPTURE_BYTES = 64 * 1_024;
const LSP_HANDSHAKE_TIMEOUT_MS = 10_000;
const LSP_SHUTDOWN_STAGE_TIMEOUT_MS = 2_000;
const LSP_MAX_HEADER_BYTES = 8 * 1_024;
const LSP_MAX_MESSAGE_BYTES = 1 * 1_024 * 1_024;
const LSP_MAX_QUEUED_MESSAGES = 64;

export const PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING =
  "omission.pylon.portable.installed-executable-profile-authority.missing";
export const PYLON_PORTABLE_EXACT_PTY_RUNTIME_UNAVAILABLE =
  "omission.pylon.portable.pty.exact-runtime-unavailable";

type WatchFactory = (path: string, options: Readonly<{ persistent: boolean }>) => FSWatcher;

export type PylonPortableDestinationProductionHelpers = Readonly<{
  adapters: ReadonlyArray<PylonPortableDestinationHelperAdapter>;
  unsupportedOmissionRefs: Partial<Record<IdePortableDestinationHelperKind, string>>;
}>;

const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex")}`;

const bindingValue = (input: PylonPortableDestinationHelperStartInput): string =>
  [
    input.destinationRunnerSessionReservationRef,
    input.sessionRef,
    input.destinationAttachmentRef,
    String(input.destinationGeneration),
    input.workspaceRef,
    input.workingDirectory,
    input.authorityEvidenceRef,
    input.authenticationPolicyRef,
    ...input.capabilityLeaseRefs,
  ].join("\n");

const ptyIsLive = (result: KhalaProcessSessionResult): boolean =>
  result.exitCode === null && result.signal === null && !result.cancelled && !result.timedOut;

const exactExecutableIsAvailable = (path: string): boolean => {
  try {
    accessSync(path, constants.X_OK);
    return statSync(path).isFile();
  } catch {
    return false;
  }
};

type LspMessage = Readonly<Record<string, unknown>>;
type MessageWaiter = Readonly<{
  accept: (message: LspMessage) => boolean;
  resolve: (message: LspMessage) => void;
  reject: (error: Error) => void;
  timer: NodeJS.Timeout;
}>;

const isMessage = (value: unknown): value is LspMessage =>
  value !== null && typeof value === "object" && !Array.isArray(value);

class LocalLspClient {
  readonly #child: ChildProcessWithoutNullStreams;
  #buffer = Buffer.alloc(0);
  readonly #messages: LspMessage[] = [];
  readonly #waiters = new Set<MessageWaiter>();
  #failed = false;
  #failure: Error | null = null;

  constructor(child: ChildProcessWithoutNullStreams) {
    this.#child = child;
    child.stdout.on("data", (chunk: Buffer) => {
      if (this.#failed) return;
      this.#buffer = Buffer.concat([this.#buffer, chunk]);
      if (this.#buffer.byteLength > LSP_MAX_HEADER_BYTES + LSP_MAX_MESSAGE_BYTES) {
        this.#fail(new Error("the LSP response buffer exceeded its bound"));
        return;
      }
      this.#drain();
    });
    const fail = () => this.#fail(new Error("the LSP process stopped before its response"));
    child.stdin.on("error", fail);
    child.once("error", fail);
    child.once("exit", fail);
  }

  send(message: LspMessage): void {
    if (this.#failure !== null) throw this.#failure;
    const body = Buffer.from(JSON.stringify(message), "utf8");
    this.#child.stdin.write(`Content-Length: ${body.byteLength}\r\n\r\n`);
    this.#child.stdin.write(body);
  }

  waitFor(accept: (message: LspMessage) => boolean, timeoutMs: number): Promise<LspMessage> {
    if (this.#failure !== null) return Promise.reject(this.#failure);
    const index = this.#messages.findIndex(accept);
    if (index >= 0) return Promise.resolve(this.#messages.splice(index, 1)[0]);
    return new Promise((resolve, reject) => {
      const waiter: MessageWaiter = {
        accept,
        resolve,
        reject,
        timer: setTimeout(() => {
          this.#waiters.delete(waiter);
          reject(new Error("the LSP response deadline expired"));
        }, timeoutMs),
      };
      this.#waiters.add(waiter);
    });
  }

  #accept(message: LspMessage): void {
    for (const waiter of this.#waiters) {
      if (!waiter.accept(message)) continue;
      clearTimeout(waiter.timer);
      this.#waiters.delete(waiter);
      waiter.resolve(message);
      return;
    }
    this.#messages.push(message);
    if (this.#messages.length > LSP_MAX_QUEUED_MESSAGES) {
      this.#fail(new Error("the LSP response queue exceeded its bound"));
    }
  }

  #drain(): void {
    while (true) {
      const boundary = this.#buffer.indexOf("\r\n\r\n");
      if (boundary < 0) {
        if (this.#buffer.byteLength > LSP_MAX_HEADER_BYTES) {
          this.#fail(new Error("the LSP response header exceeded its bound"));
        }
        return;
      }
      if (boundary > LSP_MAX_HEADER_BYTES) {
        this.#fail(new Error("the LSP response header exceeded its bound"));
        return;
      }
      const header = this.#buffer.subarray(0, boundary).toString("ascii");
      const length = /^Content-Length:\s*(\d+)$/imu.exec(header);
      if (length === null) {
        this.#fail(new Error("the LSP response header is invalid"));
        return;
      }
      const contentLength = Number.parseInt(length[1], 10);
      if (contentLength > LSP_MAX_MESSAGE_BYTES) {
        this.#fail(new Error("the LSP response body exceeded its bound"));
        return;
      }
      const bodyStart = boundary + 4;
      if (this.#buffer.byteLength < bodyStart + contentLength) return;
      const body = this.#buffer.subarray(bodyStart, bodyStart + contentLength);
      this.#buffer = this.#buffer.subarray(bodyStart + contentLength);
      try {
        const message: unknown = JSON.parse(body.toString("utf8"));
        if (!isMessage(message)) throw new Error("the LSP response body is invalid");
        this.#accept(message);
      } catch {
        this.#fail(new Error("the LSP response body is invalid"));
        return;
      }
    }
  }

  #fail(error: Error): void {
    if (this.#failed) return;
    this.#failed = true;
    this.#failure = error;
    this.#buffer = Buffer.alloc(0);
    this.#messages.length = 0;
    for (const waiter of this.#waiters) {
      clearTimeout(waiter.timer);
      waiter.reject(error);
    }
    this.#waiters.clear();
  }
}

const childIsLive = (child: ChildProcessWithoutNullStreams): boolean =>
  child.exitCode === null && child.signalCode === null && !child.killed;

const childExitWasObserved = (child: ChildProcessWithoutNullStreams): boolean =>
  child.exitCode !== null || child.signalCode !== null;

const waitForChildExit = (
  child: ChildProcessWithoutNullStreams,
  timeoutMs: number,
): Promise<boolean> => {
  if (childExitWasObserved(child)) return Promise.resolve(true);
  return new Promise((resolveExit) => {
    const timer = setTimeout(() => {
      child.off("exit", exited);
      resolveExit(false);
    }, timeoutMs);
    const exited = () => {
      clearTimeout(timer);
      resolveExit(true);
    };
    child.once("exit", exited);
  });
};

const messageHasId = (message: LspMessage, id: number): boolean => message.id === id;

const initializeSucceeded = (message: LspMessage): boolean => {
  if (!messageHasId(message, 1) || "error" in message || !isMessage(message.result)) {
    return false;
  }
  return isMessage(message.result.capabilities);
};

const isExpectedTypescriptVersion = (message: LspMessage): boolean =>
  message.method === "$/typescriptVersion" &&
  isMessage(message.params) &&
  message.params.version === "5.9.2";

const makeLspAdapter = (
  options: Readonly<{
    instanceNonce: () => string;
    profile: PylonPortableVerifiedExecutableProfile;
    startProcess: (
      profile: PylonPortableVerifiedExecutableProfile,
      workingDirectory: string,
    ) => ChildProcessWithoutNullStreams;
  }>,
): PylonPortableDestinationHelperAdapter => ({
  kind: "lsp",
  start: async (input) => {
    if (input.signal.aborted) throw input.signal.reason;
    const binding = bindingValue(input);
    const child = options.startProcess(options.profile, input.workingDirectory);
    child.stderr.resume();
    const client = new LocalLspClient(child);
    const stopDuringStart = () => child.kill("SIGTERM");
    input.signal.addEventListener("abort", stopDuringStart, { once: true });
    try {
      client.send({
        jsonrpc: "2.0",
        id: 1,
        method: "initialize",
        params: {
          processId: process.pid,
          rootUri: pathToFileURL(input.workingDirectory).href,
          capabilities: {},
          initializationOptions: {
            tsserver: { path: options.profile.typescriptServerPath },
          },
          workspaceFolders: [
            {
              name: "portable-workspace",
              uri: pathToFileURL(input.workingDirectory).href,
            },
          ],
        },
      });
      await client.waitFor(initializeSucceeded, LSP_HANDSHAKE_TIMEOUT_MS);
      client.send({ jsonrpc: "2.0", method: "initialized", params: {} });
      await client.waitFor(isExpectedTypescriptVersion, LSP_HANDSHAKE_TIMEOUT_MS);
      if (!childIsLive(child)) throw new Error("the LSP process stopped after initialize");
    } catch (error) {
      child.kill("SIGTERM");
      if (!(await waitForChildExit(child, LSP_SHUTDOWN_STAGE_TIMEOUT_MS))) {
        child.kill("SIGKILL");
        await waitForChildExit(child, LSP_SHUTDOWN_STAGE_TIMEOUT_MS);
      }
      throw error;
    } finally {
      input.signal.removeEventListener("abort", stopDuringStart);
    }

    let disposal: Promise<void> | undefined;
    const dispose = async (): Promise<void> => {
      if (!childIsLive(child)) return;
      client.send({ jsonrpc: "2.0", id: 2, method: "shutdown", params: null });
      await client
        .waitFor((message) => messageHasId(message, 2), LSP_SHUTDOWN_STAGE_TIMEOUT_MS)
        .then(
          () => client.send({ jsonrpc: "2.0", method: "exit", params: null }),
          () => undefined,
        );
      if (await waitForChildExit(child, LSP_SHUTDOWN_STAGE_TIMEOUT_MS)) return;
      child.kill("SIGTERM");
      if (await waitForChildExit(child, LSP_SHUTDOWN_STAGE_TIMEOUT_MS)) return;
      child.kill("SIGKILL");
      if (!(await waitForChildExit(child, LSP_SHUTDOWN_STAGE_TIMEOUT_MS))) {
        throw new Error("the LSP process exit was not observed");
      }
    };
    const stopAfterReady = () => {
      disposal ??= dispose();
      void disposal.catch(() => undefined);
    };
    input.signal.addEventListener("abort", stopAfterReady, { once: true });

    return {
      instanceRef: stableRef(
        "instance.pylon.portable.lsp",
        `${binding}\n${options.profile.admission.executableProfileRef}\n${child.pid ?? "unknown"}\n${options.instanceNonce()}`,
      ),
      versionRef: options.profile.admission.versionRef,
      evidenceRefs: [
        stableRef(
          "evidence.pylon.portable.lsp.profile-verified",
          `${binding}\n${options.profile.admission.executableProfileRef}\n${options.profile.admission.installedArtifactRef}\n${options.profile.admission.signatureRef}`,
        ),
        stableRef(
          "evidence.pylon.portable.lsp.initialize-complete",
          `${binding}\n${options.profile.admission.versionRef}`,
        ),
      ],
      isLive: () => childIsLive(child),
      dispose: async () => {
        disposal ??= dispose();
        try {
          await disposal;
          input.signal.removeEventListener("abort", stopAfterReady);
        } catch (error) {
          disposal = undefined;
          throw error;
        }
      },
    };
  },
});

const makePtyAdapter = (
  options: Readonly<{
    instanceNonce: () => string;
    processService: KhalaProcessService;
  }>,
): PylonPortableDestinationHelperAdapter => ({
  kind: "pty",
  start: async (input) => {
    const binding = bindingValue(input);
    const khalaSessionId = stableRef("session.pylon.portable.pty", binding);
    const poll = (sessionId: string, chars?: string) =>
      Effect.runPromise(
        options.processService.writeStdin({
          ...(chars === undefined ? {} : { chars }),
          khalaSessionId,
          maxCaptureBytes: PTY_CAPTURE_BYTES,
          sessionId,
          yieldTimeMs: 0,
        }),
      );
    const terminate = (sessionId: string) =>
      Effect.runPromise(
        options.processService.terminateSession({
          khalaSessionId,
          sessionId,
        }),
      );
    const started = await Effect.runPromise(
      options.processService.startSession({
        argv: [SHELL_EXECUTABLE],
        command: SHELL_EXECUTABLE,
        cwd: input.workingDirectory,
        khalaSessionId,
        maxCaptureBytes: PTY_CAPTURE_BYTES,
        timeoutMs: PTY_SESSION_TIMEOUT_MS,
        workspaceRoot: input.workingDirectory,
        yieldTimeMs: 10,
      }),
    );
    try {
      if (!ptyIsLive(started) || !ptyIsLive(await poll(started.sessionId))) {
        throw new Error("the PTY session did not remain live after its start probe");
      }
    } catch (error) {
      await terminate(started.sessionId);
      throw error;
    }
    let disposed = false;
    let disposal: Promise<void> | undefined;
    return {
      instanceRef: stableRef(
        "instance.pylon.portable.pty",
        `${binding}\n${started.sessionId}\n${options.instanceNonce()}`,
      ),
      versionRef: "version.pylon.portable.pty.khala-tools-python-pty-fork.v2",
      evidenceRefs: [stableRef("evidence.pylon.portable.pty.live", binding)],
      isLive: async () => {
        if (disposed) return false;
        return poll(started.sessionId).then(ptyIsLive, () => false);
      },
      dispose: async () => {
        if (disposed) return;
        disposal ??= terminate(started.sessionId).then((result) => {
          if (
            result.sessionId !== started.sessionId ||
            result.khalaSessionId !== khalaSessionId ||
            result.exitObserved !== true
          ) {
            throw new Error("the PTY session termination receipt did not match the active session");
          }
          disposed = true;
        });
        try {
          await disposal;
        } catch (error) {
          disposal = undefined;
          throw error;
        }
      },
    };
  },
});

const makeWatcherAdapter = (
  options: Readonly<{
    instanceNonce: () => string;
    watchDirectory: WatchFactory;
  }>,
): PylonPortableDestinationHelperAdapter => ({
  kind: "watcher",
  start: async (input) => {
    const binding = bindingValue(input);
    const watcher = options.watchDirectory(input.workingDirectory, { persistent: false });
    let live = true;
    watcher.once("error", () => {
      live = false;
    });
    watcher.once("close", () => {
      live = false;
    });
    return {
      instanceRef: stableRef(
        "instance.pylon.portable.watcher",
        `${binding}\n${options.instanceNonce()}`,
      ),
      versionRef: "version.pylon.portable.watcher.node-fs-watch.v1",
      evidenceRefs: [stableRef("evidence.pylon.portable.watcher.live", binding)],
      isLive: () => live,
      dispose: () => {
        if (!live) return;
        live = false;
        watcher.close();
      },
    };
  },
});

export const makePylonPortableDestinationProductionHelpers = (
  options: Readonly<{
    exactExecutableIsAvailable?: (path: string) => boolean;
    instanceNonce?: () => string;
    processService?: KhalaProcessService;
    resolveLspProfile?: () => PylonPortableVerifiedExecutableProfile | null;
    startLspProcess?: (
      profile: PylonPortableVerifiedExecutableProfile,
      workingDirectory: string,
    ) => ChildProcessWithoutNullStreams;
    watchDirectory?: WatchFactory;
  }> = {},
): PylonPortableDestinationProductionHelpers => {
  const executableIsAvailable = options.exactExecutableIsAvailable ?? exactExecutableIsAvailable;
  const instanceNonce = options.instanceNonce ?? (() => randomBytes(24).toString("hex"));
  const adapters: PylonPortableDestinationHelperAdapter[] = [
    makeWatcherAdapter({
      instanceNonce,
      watchDirectory: options.watchDirectory ?? watch,
    }),
  ];
  const unsupportedOmissionRefs: Partial<Record<IdePortableDestinationHelperKind, string>> = {
    dap: PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING,
    native: PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING,
  };
  const lspProfile = (
    options.resolveLspProfile ??
    (() => {
      const admission = repositoryOwnedPylonPortableExecutableProfileCatalog.resolve(
        PYLON_TYPESCRIPT_LSP_EXECUTABLE_PROFILE_REF,
      );
      return admission === null ? null : verifyPylonPortableExecutableProfile(admission);
    })
  )();
  if (lspProfile === null) {
    unsupportedOmissionRefs.lsp = PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING;
  } else {
    adapters.unshift(
      makeLspAdapter({
        instanceNonce,
        profile: lspProfile,
        startProcess:
          options.startLspProcess ??
          ((profile, workingDirectory) =>
            spawn(process.execPath, [profile.nodeEntrypointPath, ...profile.fixedArgv], {
              cwd: workingDirectory,
              detached: false,
              env: { LANG: "C.UTF-8", LC_ALL: "C.UTF-8" },
              shell: false,
              stdio: ["pipe", "pipe", "pipe"],
            })),
      }),
    );
  }
  if (executableIsAvailable(PYTHON_EXECUTABLE) && executableIsAvailable(SHELL_EXECUTABLE)) {
    adapters.unshift(
      makePtyAdapter({
        instanceNonce,
        processService: options.processService ?? defaultKhalaProcessService,
      }),
    );
  } else {
    unsupportedOmissionRefs.pty = PYLON_PORTABLE_EXACT_PTY_RUNTIME_UNAVAILABLE;
  }
  return { adapters, unsupportedOmissionRefs };
};

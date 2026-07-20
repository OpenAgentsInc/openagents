import { createHash, randomBytes } from "node:crypto";
import { accessSync, constants, statSync, watch, type FSWatcher } from "node:fs";

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

const PYTHON_EXECUTABLE = "/usr/bin/python3";
const SHELL_EXECUTABLE = "/bin/sh";
const PTY_SESSION_TIMEOUT_MS = 7 * 24 * 60 * 60 * 1_000;
const PTY_CAPTURE_BYTES = 64 * 1_024;

export const PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING =
  "omission.pylon.portable.installed-executable-profile-authority.missing";
export const PYLON_PORTABLE_EXACT_PTY_RUNTIME_UNAVAILABLE =
  "omission.pylon.portable.pty.exact-runtime-unavailable";

type WatchFactory = (
  path: string,
  options: Readonly<{ persistent: boolean }>,
) => FSWatcher;

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
  result.exitCode === null &&
  result.signal === null &&
  !result.cancelled &&
  !result.timedOut;

const exactExecutableIsAvailable = (path: string): boolean => {
  try {
    accessSync(path, constants.X_OK);
    return statSync(path).isFile();
  } catch {
    return false;
  }
};

const makePtyAdapter = (options: Readonly<{
  instanceNonce: () => string;
  processService: KhalaProcessService;
}>): PylonPortableDestinationHelperAdapter => ({
  kind: "pty",
  start: async (input) => {
    const binding = bindingValue(input);
    const khalaSessionId = stableRef("session.pylon.portable.pty", binding);
    const poll = (sessionId: string, chars?: string) =>
      Effect.runPromise(options.processService.writeStdin({
        ...(chars === undefined ? {} : { chars }),
        khalaSessionId,
        maxCaptureBytes: PTY_CAPTURE_BYTES,
        sessionId,
        yieldTimeMs: 0,
      }));
    const started = await Effect.runPromise(options.processService.startSession({
      argv: [SHELL_EXECUTABLE],
      command: SHELL_EXECUTABLE,
      cwd: input.workingDirectory,
      khalaSessionId,
      maxCaptureBytes: PTY_CAPTURE_BYTES,
      timeoutMs: PTY_SESSION_TIMEOUT_MS,
      workspaceRoot: input.workingDirectory,
      yieldTimeMs: 10,
    }));
    try {
      if (!ptyIsLive(started) || !ptyIsLive(await poll(started.sessionId))) {
        throw new Error("the PTY session did not remain live after its start probe");
      }
    } catch (error) {
      await poll(started.sessionId, "\u0003").catch(() => undefined);
      throw error;
    }
    let disposed = false;
    return {
      instanceRef: stableRef(
        "instance.pylon.portable.pty",
        `${binding}\n${started.sessionId}\n${options.instanceNonce()}`,
      ),
      versionRef: "version.pylon.portable.pty.khala-tools-python-pty-fork.v1",
      evidenceRefs: [stableRef("evidence.pylon.portable.pty.live", binding)],
      isLive: async () => {
        if (disposed) return false;
        return poll(started.sessionId).then(ptyIsLive, () => false);
      },
      dispose: async () => {
        if (disposed) return;
        disposed = true;
        await poll(started.sessionId, "\u0003").catch(() => undefined);
      },
    };
  },
});

const makeWatcherAdapter = (options: Readonly<{
  instanceNonce: () => string;
  watchDirectory: WatchFactory;
}>): PylonPortableDestinationHelperAdapter => ({
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
    watchDirectory?: WatchFactory;
  }> = {},
): PylonPortableDestinationProductionHelpers => {
  const executableIsAvailable =
    options.exactExecutableIsAvailable ?? exactExecutableIsAvailable;
  const instanceNonce = options.instanceNonce ?? (() => randomBytes(24).toString("hex"));
  const adapters: PylonPortableDestinationHelperAdapter[] = [
    makeWatcherAdapter({
      instanceNonce,
      watchDirectory: options.watchDirectory ?? watch,
    }),
  ];
  const unsupportedOmissionRefs: Partial<Record<IdePortableDestinationHelperKind, string>> = {
    lsp: PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING,
    dap: PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING,
    native: PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING,
  };
  if (
    executableIsAvailable(PYTHON_EXECUTABLE) &&
    executableIsAvailable(SHELL_EXECUTABLE)
  ) {
    adapters.unshift(makePtyAdapter({
      instanceNonce,
      processService: options.processService ?? defaultKhalaProcessService,
    }));
  } else {
    unsupportedOmissionRefs.pty = PYLON_PORTABLE_EXACT_PTY_RUNTIME_UNAVAILABLE;
  }
  return { adapters, unsupportedOmissionRefs };
};

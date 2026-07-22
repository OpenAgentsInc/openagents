import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import type {
  KhalaProcessService,
  KhalaProcessSessionResult,
  KhalaProcessSessionStartInput,
  KhalaProcessSessionTerminationInput,
} from "@openagentsinc/khala-tools";
import { KhalaToolRuntimeError } from "@openagentsinc/khala-tools";
import { Effect } from "effect";
import { describe, expect, test, vi } from "vite-plus/test";

import {
  makePylonPortableDestinationProductionHelpers,
  PYLON_PORTABLE_EXACT_PTY_RUNTIME_UNAVAILABLE,
  PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING,
} from "./portable-destination-production-helper-adapters.js";
import type { PylonPortableDestinationHelperStartInput } from "./portable-destination-helper-supervisor.js";

const observedAt = "2026-07-20T17:00:00.000Z";

const input = (
  workingDirectory: string,
  overrides: Partial<PylonPortableDestinationHelperStartInput> = {},
): PylonPortableDestinationHelperStartInput => ({
  destinationRunnerSessionReservationRef: "runner-session-reservation.ide13.production",
  sessionRef: "session.ide13.production",
  destinationAttachmentRef: "attachment.ide13.production.2",
  destinationGeneration: 2,
  workspaceRef: "workspace.ide13.production",
  workingDirectory,
  authorityEvidenceRef: "evidence.ide13.production.authority",
  authenticationPolicyRef: "policy.ide13.production.authentication",
  capabilityLeaseRefs: ["lease.ide13.production.provider"],
  authentication: {
    state: "reauthenticated",
    policyRef: "policy.ide13.production.authentication",
    evidenceRef: "evidence.ide13.production.authority",
    observedAt,
    expiresAt: null,
  },
  signal: new AbortController().signal,
  ...overrides,
});

const sessionResult = (sessionId: string, live: boolean): KhalaProcessSessionResult => ({
  cancelled: !live,
  durationMs: 0,
  events: [],
  exitCode: live ? null : 130,
  sandbox: { enforced: false, kind: "none", note: "test service" },
  sessionId,
  signal: live ? null : "SIGINT",
  stderr: "",
  stderrTruncated: false,
  stdout: "",
  stdoutTruncated: false,
  timedOut: false,
});

const makeProcessService = () => {
  let live = true;
  const starts: KhalaProcessSessionStartInput[] = [];
  const terminations: KhalaProcessSessionTerminationInput[] = [];
  const processService: KhalaProcessService = {
    execCommand: () => Effect.die("not used"),
    marker: "khala.process_service",
    startSession: (value) => {
      starts.push(value);
      live = true;
      return Effect.succeed(sessionResult(`khala-session-${starts.length}`, true));
    },
    terminateSession: (value) => {
      terminations.push(value);
      live = false;
      return Effect.succeed({
        ...sessionResult(value.sessionId, false),
        exitObserved: true,
        khalaSessionId: value.khalaSessionId,
        termination: "graceful_interrupt",
      });
    },
    writeStdin: (value) => {
      if (value.chars === "\u0003") live = false;
      return Effect.succeed(sessionResult(value.sessionId, live));
    },
  };
  return { processService, starts, terminations };
};

const adapterFor = (
  helpers: ReturnType<typeof makePylonPortableDestinationProductionHelpers>,
  kind: "lsp" | "pty" | "watcher",
) => {
  const adapter = helpers.adapters.find((candidate) => candidate.kind === kind);
  if (adapter === undefined) throw new Error(`missing ${kind} adapter`);
  return adapter;
};

const hostileLspProcess = (script: string, observed: ChildProcessWithoutNullStreams[]) => {
  const child = spawn(process.execPath, ["-e", script], {
    env: { LANG: "C.UTF-8", LC_ALL: "C.UTF-8" },
    stdio: ["pipe", "pipe", "pipe"],
  });
  observed.push(child);
  return child;
};

describe("portable destination production helper adapters", () => {
  test("starts the exact Khala PTY argv and proves liveness before readiness", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-production-pty-"));
    const { processService, starts, terminations } = makeProcessService();
    let nonce = 0;
    try {
      const helpers = makePylonPortableDestinationProductionHelpers({
        exactExecutableIsAvailable: () => true,
        instanceNonce: () => `nonce-${(nonce += 1)}`,
        processService,
      });
      const adapter = adapterFor(helpers, "pty");
      const first = await adapter.start(input(root));

      expect(starts[0]).toMatchObject({
        argv: ["/bin/sh"],
        command: "/bin/sh",
        cwd: root,
        workspaceRoot: root,
      });
      expect(await first.isLive()).toBe(true);
      await first.dispose();
      expect(terminations).toEqual([
        {
          khalaSessionId: expect.stringMatching(/^session\.pylon\.portable\.pty\./u),
          sessionId: "khala-session-1",
        },
      ]);
      expect(await first.isLive()).toBe(false);

      const restarted = await adapter.start(input(root));
      expect(restarted.instanceRef).not.toBe(first.instanceRef);
      await restarted.dispose();
      expect(terminations[1]).toMatchObject({ sessionId: "khala-session-2" });
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("binds watcher evidence to reservation, generation, and workspace and closes it", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-production-watcher-"));
    let nonce = 0;
    try {
      const helpers = makePylonPortableDestinationProductionHelpers({
        exactExecutableIsAvailable: () => false,
        instanceNonce: () => `nonce-${(nonce += 1)}`,
      });
      const adapter = adapterFor(helpers, "watcher");
      const first = await adapter.start(input(root));
      const replacement = await adapter.start(
        input(root, {
          destinationRunnerSessionReservationRef: "runner-session-reservation.ide13.production.3",
          destinationGeneration: 3,
          workspaceRef: "workspace.ide13.production.3",
        }),
      );

      expect(await first.isLive()).toBe(true);
      expect(first.evidenceRefs).not.toEqual(replacement.evidenceRefs);
      expect(first.instanceRef).not.toBe(replacement.instanceRef);
      await first.dispose();
      await replacement.dispose();
      expect(await first.isLive()).toBe(false);
      expect(await replacement.isLive()).toBe(false);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("keeps an unconfirmed PTY teardown visible and retryable", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-production-pty-failed-teardown-"));
    const { processService } = makeProcessService();
    const terminateSession = vi.fn(() =>
      Effect.fail(
        new KhalaToolRuntimeError({
          code: "process_session_termination_unconfirmed",
          reason: "test termination was not observed",
        }),
      ),
    );
    try {
      const helpers = makePylonPortableDestinationProductionHelpers({
        exactExecutableIsAvailable: () => true,
        processService: { ...processService, terminateSession },
      });
      const handle = await adapterFor(helpers, "pty").start(input(root));

      await expect(handle.dispose()).rejects.toMatchObject({
        code: "process_session_termination_unconfirmed",
      });
      await expect(handle.dispose()).rejects.toMatchObject({
        code: "process_session_termination_unconfirmed",
      });
      expect(terminateSession).toHaveBeenCalledTimes(2);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("reports missing executable authority and exact PTY runtime without discovery", () => {
    const exactExecutableIsAvailable = vi.fn(() => false);
    const helpers = makePylonPortableDestinationProductionHelpers({
      exactExecutableIsAvailable,
      resolveLspProfile: () => null,
    });

    expect(helpers.adapters.map((adapter) => adapter.kind)).toEqual(["watcher"]);
    expect(exactExecutableIsAvailable.mock.calls).toEqual([["/usr/bin/python3"]]);
    expect(helpers.unsupportedOmissionRefs).toEqual({
      pty: PYLON_PORTABLE_EXACT_PTY_RUNTIME_UNAVAILABLE,
      lsp: PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING,
      dap: PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING,
      native: PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING,
    });
  });

  test("keeps LSP, DAP, and native unsupported without a durable executable profile", () => {
    const helpers = makePylonPortableDestinationProductionHelpers({
      exactExecutableIsAvailable: () => true,
      resolveLspProfile: () => null,
    });

    expect(helpers.adapters.map((adapter) => adapter.kind)).toEqual(["pty", "watcher"]);
    expect(helpers.unsupportedOmissionRefs).toMatchObject({
      lsp: PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING,
      dap: PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING,
      native: PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING,
    });
  });

  test("starts the verified TypeScript LSP, completes initialize, and observes exit", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-production-lsp-"));
    try {
      const helpers = makePylonPortableDestinationProductionHelpers({
        exactExecutableIsAvailable: () => false,
        instanceNonce: () => "lsp-live-nonce",
      });
      expect(helpers.unsupportedOmissionRefs.lsp).toBeUndefined();
      const handle = await adapterFor(helpers, "lsp").start(input(root));

      expect(handle.versionRef).toBe(
        "version.pylon.portable.lsp.typescript-language-server-5.3.0.typescript-5.9.2.v1",
      );
      expect(handle.evidenceRefs).toHaveLength(2);
      expect(handle.evidenceRefs.every((ref) => !ref.includes(root))).toBe(true);
      expect(handle.instanceRef.includes(root)).toBe(false);
      expect(await handle.isLive()).toBe(true);
      await handle.dispose();
      expect(await handle.isLive()).toBe(false);

      const replacement = await adapterFor(helpers, "lsp").start(
        input(root, {
          destinationRunnerSessionReservationRef: "runner-session-reservation.ide13.production.3",
          destinationGeneration: 3,
          workspaceRef: "workspace.ide13.production.3",
        }),
      );
      expect(replacement.instanceRef).not.toBe(handle.instanceRef);
      expect(replacement.evidenceRefs).not.toEqual(handle.evidenceRefs);
      await replacement.dispose();
      expect(await replacement.isLive()).toBe(false);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test.each([
    [
      "oversize body",
      `process.stdout.write("Content-Length: 1048577\\r\\n\\r\\n"); setInterval(() => {}, 1000);`,
    ],
    ["endless header", `process.stdout.write("x".repeat(8193)); setInterval(() => {}, 1000);`],
    [
      "notification flood",
      `const body = JSON.stringify({ jsonrpc: "2.0", method: "window/logMessage", params: {} }); const frame = "Content-Length: " + Buffer.byteLength(body) + "\\r\\n\\r\\n" + body; for (let index = 0; index < 65; index += 1) process.stdout.write(frame); setInterval(() => {}, 1000);`,
    ],
  ])("fails closed and observes exit for a hostile %s", async (_label, script) => {
    const root = await mkdtemp(join(tmpdir(), "pylon-hostile-lsp-"));
    const observed: ChildProcessWithoutNullStreams[] = [];
    try {
      const helpers = makePylonPortableDestinationProductionHelpers({
        exactExecutableIsAvailable: () => false,
        startLspProcess: (_profile, _workingDirectory) => hostileLspProcess(script, observed),
      });

      await expect(adapterFor(helpers, "lsp").start(input(root))).rejects.toThrow();
      expect(observed).toHaveLength(1);
      expect(observed[0]?.exitCode !== null || observed[0]?.signalCode !== null).toBe(true);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});

import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import type {
  KhalaProcessService,
  KhalaProcessSessionResult,
  KhalaProcessSessionStartInput,
} from "@openagentsinc/khala-tools";
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
  const processService: KhalaProcessService = {
    execCommand: () => Effect.die("not used"),
    marker: "khala.process_service",
    startSession: (value) => {
      starts.push(value);
      live = true;
      return Effect.succeed(sessionResult(`khala-session-${starts.length}`, true));
    },
    writeStdin: (value) => {
      if (value.chars === "\u0003") live = false;
      return Effect.succeed(sessionResult(value.sessionId, live));
    },
  };
  return { processService, starts };
};

const adapterFor = (
  helpers: ReturnType<typeof makePylonPortableDestinationProductionHelpers>,
  kind: "pty" | "watcher",
) => {
  const adapter = helpers.adapters.find((candidate) => candidate.kind === kind);
  if (adapter === undefined) throw new Error(`missing ${kind} adapter`);
  return adapter;
};

describe("portable destination production helper adapters", () => {
  test("starts the exact Khala PTY argv and proves liveness before readiness", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-production-pty-"));
    const { processService, starts } = makeProcessService();
    let nonce = 0;
    try {
      const helpers = makePylonPortableDestinationProductionHelpers({
        exactExecutableIsAvailable: () => true,
        instanceNonce: () => `nonce-${nonce += 1}`,
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
      expect(await first.isLive()).toBe(false);

      const restarted = await adapter.start(input(root));
      expect(restarted.instanceRef).not.toBe(first.instanceRef);
      await restarted.dispose();
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
        instanceNonce: () => `nonce-${nonce += 1}`,
      });
      const adapter = adapterFor(helpers, "watcher");
      const first = await adapter.start(input(root));
      const replacement = await adapter.start(input(root, {
        destinationRunnerSessionReservationRef: "runner-session-reservation.ide13.production.3",
        destinationGeneration: 3,
        workspaceRef: "workspace.ide13.production.3",
      }));

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

  test("reports missing executable authority and exact PTY runtime without discovery", () => {
    const exactExecutableIsAvailable = vi.fn(() => false);
    const helpers = makePylonPortableDestinationProductionHelpers({
      exactExecutableIsAvailable,
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
    });

    expect(helpers.adapters.map((adapter) => adapter.kind)).toEqual(["pty", "watcher"]);
    expect(helpers.unsupportedOmissionRefs).toMatchObject({
      lsp: PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING,
      dap: PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING,
      native: PYLON_PORTABLE_INSTALLED_EXECUTABLE_PROFILE_AUTHORITY_MISSING,
    });
  });
});

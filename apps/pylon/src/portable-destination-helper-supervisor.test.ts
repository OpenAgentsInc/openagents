import { describe, expect, test, vi } from "vite-plus/test";

import type { IdePortableDestinationAuthentication } from "@openagentsinc/portable-session-contract";

import {
  makePylonPortableDestinationHelperSupervisor,
  PylonPortableDestinationHelperSupervisorError,
  type PylonPortableDestinationAuthenticator,
  type PylonPortableDestinationHelperAdapter,
  type PylonPortableDestinationHelperActivationInput,
} from "./portable-destination-helper-supervisor.js";

const observedAt = "2026-07-20T16:00:00.000Z";
const now = () => new Date(observedAt);
const input = (
  overrides: Partial<PylonPortableDestinationHelperActivationInput> = {},
): PylonPortableDestinationHelperActivationInput => ({
  destinationRunnerSessionReservationRef: "runner-session-reservation.ide13.helpers",
  sessionRef: "session.ide13.helpers",
  destinationAttachmentRef: "attachment.ide13.helpers.2",
  destinationGeneration: 2,
  workspaceRef: "workspace.ide13.helpers",
  workingDirectory: "/tmp/pylon-ide13-helper-workspace",
  authorityEvidenceRef: "evidence.ide13.helpers.authority",
  authenticationPolicyRef: "policy.ide13.helpers.authentication",
  capabilityLeaseRefs: ["lease.ide13.helpers.provider"],
  ...overrides,
});

const authentication = (
  state: IdePortableDestinationAuthentication["state"] = "reauthenticated",
  expiresAt: string | null = null,
): IdePortableDestinationAuthentication => ({
  state,
  policyRef: "policy.ide13.helpers.authentication",
  evidenceRef: "evidence.ide13.helpers.authority",
  observedAt,
  expiresAt,
});

const authenticator = (
  result: IdePortableDestinationAuthentication = authentication(),
): PylonPortableDestinationAuthenticator => ({
  authenticate: vi.fn(async () => result),
});

const liveAdapter = (
  kind: PylonPortableDestinationHelperAdapter["kind"],
  disposed: Array<string>,
  suffix: string = kind,
): PylonPortableDestinationHelperAdapter => ({
  kind,
  start: vi.fn(async () => ({
    instanceRef: `instance.ide13.helpers.${suffix}`,
    versionRef: `version.ide13.helpers.${suffix}.1`,
    evidenceRefs: [`evidence.ide13.helpers.${suffix}.live`],
    isLive: () => true,
    dispose: () => {
      disposed.push(kind);
    },
  })),
});

describe("portable destination helper supervisor", () => {
  test("starts only injected live helpers and leaves unavailable kinds unsupported", async () => {
    const disposed: string[] = [];
    const watcher = liveAdapter("watcher", disposed);
    const supervisor = makePylonPortableDestinationHelperSupervisor({
      authenticator: authenticator(),
      adapters: [watcher],
      now,
    });

    const result = await supervisor.activate(input());

    expect(result.authentication.state).toBe("reauthenticated");
    expect(result.helpersObservedAt).toBe(observedAt);
    expect(result.helpers.find((helper) => helper.kind === "watcher")).toEqual({
      kind: "watcher",
      readiness: "ready",
      instanceRef: "instance.ide13.helpers.watcher",
      versionRef: "version.ide13.helpers.watcher.1",
      omissionRef: null,
      evidenceRefs: ["evidence.ide13.helpers.watcher.live"],
    });
    expect(result.helpers.filter((helper) => helper.kind !== "watcher")).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ kind: "pty", readiness: "unsupported", instanceRef: null }),
        expect.objectContaining({ kind: "lsp", readiness: "unsupported", instanceRef: null }),
        expect.objectContaining({ kind: "dap", readiness: "unsupported", instanceRef: null }),
        expect.objectContaining({ kind: "native", readiness: "unsupported", instanceRef: null }),
      ]),
    );
    expect(result.evidenceRefs).toContain("evidence.ide13.helpers.watcher.live");
    expect(result.evidenceRefs).toContainEqual(
      expect.stringMatching(/^receipt\.pylon\.portable\.destination-helpers\./u),
    );
    expect(disposed).toEqual([]);
  });

  test("rejects wrong scope, generation drift, and conflicting reservation replay", async () => {
    const supervisor = makePylonPortableDestinationHelperSupervisor({
      authenticator: authenticator(),
      now,
    });
    await expect(
      supervisor.activate(
        input({
          destinationRunnerSessionReservationRef: "wrong reservation",
        }),
      ),
    ).rejects.toMatchObject({ reason: "invalid_scope" });

    await supervisor.activate(input());
    await expect(supervisor.activate(input({ destinationGeneration: 3 }))).rejects.toMatchObject({
      reason: "conflicting_replay",
    });
    await expect(
      supervisor.activate(input({ workspaceRef: "workspace.ide13.helpers.drift" })),
    ).rejects.toMatchObject({ reason: "conflicting_replay" });
  });

  test("rejects expired or revoked authentication before any helper starts", async () => {
    const starts = vi.fn(async () => {
      throw new Error("must not start");
    });
    for (const auth of [
      authentication("expired", "2026-07-20T15:59:00.000Z"),
      authentication("revoked"),
      authentication("reauthenticated", "2026-07-20T15:59:00.000Z"),
    ]) {
      const supervisor = makePylonPortableDestinationHelperSupervisor({
        authenticator: authenticator(auth),
        adapters: [{ kind: "watcher", start: starts }],
        now,
      });
      await expect(supervisor.activate(input())).rejects.toMatchObject({
        reason: "authentication_failed",
      });
    }
    expect(starts).not.toHaveBeenCalled();
  });

  test("rolls back prior live helpers when a later adapter fails", async () => {
    const disposed: string[] = [];
    const supervisor = makePylonPortableDestinationHelperSupervisor({
      authenticator: authenticator(),
      adapters: [
        liveAdapter("lsp", disposed),
        {
          kind: "dap",
          start: async () => {
            throw new Error("DAP unavailable");
          },
        },
      ],
      now,
    });

    await expect(supervisor.activate(input())).rejects.toMatchObject({
      reason: "helper_start_failed",
    });
    expect(disposed).toEqual(["lsp"]);
  });

  test("returns the same receipt for same-byte live replay and starts once", async () => {
    const disposed: string[] = [];
    const watcher = liveAdapter("watcher", disposed);
    const supervisor = makePylonPortableDestinationHelperSupervisor({
      authenticator: authenticator(),
      adapters: [watcher],
      now,
    });

    const first = await supervisor.activate(input());
    const replay = await supervisor.activate(input());

    expect(replay).toBe(first);
    expect(watcher.start).toHaveBeenCalledTimes(1);
  });

  test("keeps the live generation until its authenticated replacement is ready to start", async () => {
    const disposed: string[] = [];
    const authenticate = vi
      .fn()
      .mockResolvedValueOnce(authentication())
      .mockResolvedValueOnce(authentication("revoked"))
      .mockResolvedValueOnce(authentication());
    const supervisor = makePylonPortableDestinationHelperSupervisor({
      authenticator: { authenticate },
      adapters: [liveAdapter("watcher", disposed)],
      now,
    });
    await supervisor.activate(input());
    const replacement = input({
      destinationRunnerSessionReservationRef: "runner-session-reservation.ide13.helpers.3",
      destinationAttachmentRef: "attachment.ide13.helpers.3",
      destinationGeneration: 3,
    });

    await expect(supervisor.activate(replacement)).rejects.toMatchObject({
      reason: "authentication_failed",
    });
    expect(disposed).toEqual([]);

    await supervisor.activate(replacement);
    expect(disposed).toEqual(["watcher"]);
  });

  test("cancels an in-flight authenticated start during process shutdown", async () => {
    let resolveAuthentication!: (value: IdePortableDestinationAuthentication) => void;
    const authenticationPending = new Promise<IdePortableDestinationAuthentication>((resolve) => {
      resolveAuthentication = resolve;
    });
    const watcher = liveAdapter("watcher", []);
    const supervisor = makePylonPortableDestinationHelperSupervisor({
      authenticator: { authenticate: () => authenticationPending },
      adapters: [watcher],
      now,
    });

    const activation = supervisor.activate(input());
    const shutdown = supervisor.disposeAll();
    resolveAuthentication(authentication());

    await shutdown;
    await expect(activation).rejects.toMatchObject({ reason: "helper_start_failed" });
    expect(watcher.start).not.toHaveBeenCalled();
  });

  test("rolls back a helper that returns after shutdown cancellation", async () => {
    let reportStarted!: () => void;
    let releaseStart!: () => void;
    const started = new Promise<void>((resolve) => {
      reportStarted = resolve;
    });
    const release = new Promise<void>((resolve) => {
      releaseStart = resolve;
    });
    const dispose = vi.fn();
    const supervisor = makePylonPortableDestinationHelperSupervisor({
      authenticator: authenticator(),
      adapters: [{
        kind: "watcher",
        start: async () => {
          reportStarted();
          await release;
          return {
            instanceRef: "instance.ide13.helpers.late-watcher",
            versionRef: "version.ide13.helpers.late-watcher.1",
            evidenceRefs: ["evidence.ide13.helpers.late-watcher.live"],
            isLive: () => true,
            dispose,
          };
        },
      }],
      now,
    });

    const activation = supervisor.activate(input());
    await started;
    const shutdown = supervisor.disposeAll();
    releaseStart();

    await shutdown;
    await expect(activation).rejects.toMatchObject({ reason: "helper_start_failed" });
    expect(dispose).toHaveBeenCalledOnce();
  });

  test("disposes on caller abort, explicit revocation, shutdown, and fresh restart", async () => {
    const disposed: string[] = [];
    const make = () =>
      makePylonPortableDestinationHelperSupervisor({
        authenticator: authenticator(),
        adapters: [liveAdapter("watcher", disposed, `watcher.${disposed.length}`)],
        now,
      });

    const controller = new AbortController();
    const first = make();
    await first.activate(input({ signal: controller.signal }));
    controller.abort();
    await vi.waitFor(() => expect(disposed).toEqual(["watcher"]));

    await first.activate(input());
    await first.disposeSession("session.ide13.helpers");
    expect(disposed).toEqual(["watcher", "watcher"]);

    await first.activate(input());
    await first.disposeAll();
    expect(disposed).toEqual(["watcher", "watcher", "watcher"]);

    const restarted = make();
    const restartedResult = await restarted.activate(input());
    expect(restartedResult.helpers.find((helper) => helper.kind === "watcher")).toMatchObject({
      readiness: "ready",
    });
    await restarted.disposeReservation("runner-session-reservation.ide13.helpers");
    expect(disposed).toEqual(["watcher", "watcher", "watcher", "watcher"]);
  });

  test("never reports a non-live adapter handle as ready", async () => {
    const dispose = vi.fn();
    const supervisor = makePylonPortableDestinationHelperSupervisor({
      authenticator: authenticator(),
      adapters: [
        {
          kind: "native",
          start: async () => ({
            instanceRef: "instance.ide13.helpers.native",
            versionRef: "version.ide13.helpers.native.1",
            evidenceRefs: ["evidence.ide13.helpers.native"],
            isLive: () => false,
            dispose,
          }),
        },
      ],
      now,
    });

    await expect(supervisor.activate(input())).rejects.toBeInstanceOf(
      PylonPortableDestinationHelperSupervisorError,
    );
    expect(dispose).toHaveBeenCalledTimes(1);
  });
});

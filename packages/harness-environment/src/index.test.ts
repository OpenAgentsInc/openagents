import { makeReferenceAdapter, type HarnessSession } from "@openagentsinc/agent-harness-contract";
import { Effect, Exit, Schema as S, Stream } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  DEFAULT_HARNESS_ENVIRONMENT,
  HarnessEnvironment,
  decodeHarnessEnvironment,
  makeManagedSandboxHarnessEnvironment,
  makeOpenAgentsCloudHarnessEnvironment,
  managedSandboxTurnUrl,
  openAgentsCloudCodingSessionLaunchUrl,
  openAgentsCloudCodingSessionLifecycleUrl,
  startHarnessInEnvironment,
  type HarnessEnvironmentRunner,
  type OpenAgentsCloudHarnessEnvironment,
} from "./index.js";

const source = {
  lane: "ai_sdk_core",
  adapterKind: "openagents_native",
} as const;

describe("HarnessEnvironment", () => {
  test("decodes all three environments and rejects excess or credential fields", async () => {
    const values = [
      DEFAULT_HARNESS_ENVIRONMENT,
      makeOpenAgentsCloudHarnessEnvironment("https://openagents.com"),
      makeManagedSandboxHarnessEnvironment({
        controlPlaneBaseUrl: "https://sandbox-control.openagents.com",
        sandboxRef: "sandbox.owner.01",
      }),
    ];

    for (const value of values) {
      expect(await Effect.runPromise(S.decodeUnknownEffect(HarnessEnvironment)(value))).toEqual(
        value,
      );
    }

    expect(await Effect.runPromise(decodeHarnessEnvironment(undefined))).toEqual(
      DEFAULT_HARNESS_ENVIRONMENT,
    );

    const credentialed = await Effect.runPromiseExit(
      decodeHarnessEnvironment({
        _tag: "openagents_cloud",
        controlPlaneBaseUrl: "https://openagents.com",
        launchPath: "/v1/cloud-coding-sessions",
        bearerToken: "must-not-enter-the-environment-contract",
      }),
    );
    expect(Exit.isFailure(credentialed)).toBe(true);
  });

  test("keeps the existing desktop adapter as the exact default path", async () => {
    const harness = makeReferenceAdapter({
      harnessId: "desktop-default",
      scriptWords: ["local"],
    });

    const direct = await Effect.runPromise(harness.start({ sessionId: "session-direct", source }));
    const selected = await Effect.runPromise(
      startHarnessInEnvironment({
        harness,
        options: { sessionId: "session-selected", source },
      }),
    );

    expect(direct.sessionId).toBe("session-direct");
    expect(selected.sessionId).toBe("session-selected");
    expect(selected.isResume).toBe(false);
  });

  test("selects the openagents_cloud runner and preserves the SDK session stream contract", async () => {
    const harness = makeReferenceAdapter({
      harnessId: "cloud-selected",
      scriptWords: ["cloud"],
    });
    const cloud = makeOpenAgentsCloudHarnessEnvironment("https://openagents.com/control/");
    const remoteSession = await Effect.runPromise(
      harness.start({ sessionId: "session-cloud", source }),
    );
    const observed: Array<OpenAgentsCloudHarnessEnvironment> = [];
    const runner: HarnessEnvironmentRunner<OpenAgentsCloudHarnessEnvironment> = {
      environment: "openagents_cloud",
      start: ({ environment }) =>
        Effect.sync(() => {
          observed.push(environment);
          return remoteSession;
        }),
    };

    const selected: HarnessSession = await Effect.runPromise(
      startHarnessInEnvironment({
        environment: cloud,
        harness,
        options: { sessionId: "session-cloud", source },
        runners: { openagentsCloud: runner },
      }),
    );

    expect(selected).toBe(remoteSession);
    expect(observed).toEqual([cloud]);

    const turn = await Effect.runPromise(
      selected.promptTurn({ turnId: "turn-cloud", prompt: "run" }),
    );
    const events = await Effect.runPromise(Stream.runCollect(turn.events));
    const done = await Effect.runPromise(turn.done);
    expect(events[0]?.kind).toBe("turn.started");
    expect(events.at(-1)?.kind).toBe("turn.finished");
    expect(done.finishReason).toBe("stop");
  });

  test("binds openagents_cloud to the current Agent Computer route seam", () => {
    const environment = makeOpenAgentsCloudHarnessEnvironment("https://openagents.com/");

    expect(openAgentsCloudCodingSessionLaunchUrl(environment)).toBe(
      "https://openagents.com/v1/cloud-coding-sessions",
    );
    expect(openAgentsCloudCodingSessionLifecycleUrl(environment, "cloud turn/session 01")).toBe(
      "https://openagents.com/v1/cloud-coding-sessions/cloud%20turn%2Fsession%2001",
    );
  });

  test("binds managed_sandbox to its broker-only turn route", () => {
    const environment = makeManagedSandboxHarnessEnvironment({
      controlPlaneBaseUrl: "https://control.openagents.com/",
      sandboxRef: "sandbox.owner.01",
    });

    expect(managedSandboxTurnUrl(environment)).toBe(
      "https://control.openagents.com/v1/managed-sandbox/runtime/turns",
    );
  });

  test("fails closed when a selected remote runner is not installed", async () => {
    const harness = makeReferenceAdapter({
      harnessId: "missing-runner",
      scriptWords: ["unused"],
    });
    const error = await Effect.runPromise(
      startHarnessInEnvironment({
        environment: makeOpenAgentsCloudHarnessEnvironment("https://openagents.com"),
        harness,
        options: { sessionId: "session-missing-runner", source },
      }).pipe(Effect.flip),
    );

    expect(error).toMatchObject({
      _tag: "AgentHarness.EnvironmentError",
      environment: "openagents_cloud",
      failureClass: "environment_runner_unavailable",
    });
  });
});

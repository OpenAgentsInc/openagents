import { describe, expect, test } from "vite-plus/test";

import {
  createAppleFmSupervisor,
  type AppleFmLaunchOutcome,
  type AppleFmLauncher,
  type AppleFmLauncherSession,
} from "./supervisor.js";
import type { AppleFmProbe } from "./client.js";

const readySession = (over: Partial<AppleFmLauncherSession> = {}): { session: AppleFmLauncherSession; stops: () => number } => {
  let stops = 0;
  const session: AppleFmLauncherSession = {
    mode: "launched",
    probe: async (): Promise<AppleFmProbe> => ({
      status: "ready",
      ready: true,
      model: "apple-foundation-model",
      profileId: "apple-fm-local",
      usageTruth: "estimated",
    }),
    complete: async () => ({ outcome: "completed", text: "hello", usageTruth: "estimated", totalTokens: 5 }),
    stop: () => {
      stops += 1;
    },
    ...over,
  };
  return { session, stops: () => stops };
};

const launcher = (input: {
  supported?: boolean;
  outcome: AppleFmLaunchOutcome;
  onLaunch?: (onCrash: (failureClass: string) => void) => void;
}): AppleFmLauncher => ({
  supported: () => input.supported ?? true,
  launch: async ({ onCrash }) => {
    input.onLaunch?.(onCrash);
    return input.outcome;
  },
});

describe("Apple FM neutral supervisor lifecycle", () => {
  test("unsupported platform stays not_supported and refuses turns without launching", async () => {
    let launched = false;
    const supervisor = createAppleFmSupervisor(
      launcher({ supported: false, outcome: { kind: "helper_missing", blockerRef: "b" }, onLaunch: () => (launched = true) }),
    );
    expect(supervisor.status()).toMatchObject({ supported: false, state: "not_supported", ready: false, readiness: "unsupported" });
    expect(await supervisor.ensureStarted()).toMatchObject({ state: "not_supported" });
    expect(launched).toBe(false);
    expect(await supervisor.runTurn("hi")).toMatchObject({ outcome: "failed", failureClass: "unsupported_platform" });
  });

  test("missing helper reports helper_missing with a bounded blocker", async () => {
    const supervisor = createAppleFmSupervisor(
      launcher({ outcome: { kind: "helper_missing", blockerRef: "blocker.apple_fm.helper_missing" } }),
    );
    const status = await supervisor.ensureStarted();
    expect(status).toMatchObject({ state: "helper_missing", ready: false, unavailableReason: "helper_missing" });
    expect(status.blockerRefs).toEqual(["blocker.apple_fm.helper_missing"]);
  });

  test("digest/signature failure reports failed with a typed failure class", async () => {
    const supervisor = createAppleFmSupervisor(
      launcher({
        outcome: { kind: "failed", blockerRef: "blocker.apple_fm.apple_fm_helper_digest_mismatch", failureClass: "apple_fm_helper_digest_mismatch" },
      }),
    );
    expect(await supervisor.ensureStarted()).toMatchObject({ state: "failed", ready: false, unavailableReason: "apple_fm_helper_digest_mismatch" });
  });

  test("ready launch reaches ready and admits one bounded turn; not-ready refuses", async () => {
    const { session } = readySession();
    const supervisor = createAppleFmSupervisor(launcher({ outcome: { kind: "session", session } }));
    expect(await supervisor.runTurn("hi")).toMatchObject({ outcome: "failed", failureClass: "not_ready" });
    const status = await supervisor.ensureStarted();
    expect(status).toMatchObject({ state: "ready", ready: true, mode: "local_launched", model: "apple-foundation-model", usageTruth: "estimated" });
    expect(await supervisor.runTurn("read the readme")).toMatchObject({ outcome: "completed", text: "hello", totalTokens: 5 });
  });

  test("not-ready health projects unavailable and refuses the turn", async () => {
    const { session } = readySession({
      probe: async () => ({ status: "unsupported", ready: false, unavailableReason: "apple_intelligence_disabled" }),
    });
    const supervisor = createAppleFmSupervisor(launcher({ outcome: { kind: "session", session } }));
    const status = await supervisor.ensureStarted();
    expect(status).toMatchObject({ state: "unavailable", ready: false, readiness: "unsupported", unavailableReason: "apple_intelligence_disabled" });
    expect(await supervisor.runTurn("hi")).toMatchObject({ outcome: "failed", failureClass: "not_ready" });
  });

  test("adopted bridge is never stopped on stop() or dispose()", async () => {
    const { session, stops } = readySession({ mode: "adopted" });
    const supervisor = createAppleFmSupervisor(launcher({ outcome: { kind: "session", session } }));
    expect(await supervisor.ensureStarted()).toMatchObject({ mode: "local_adopted", state: "ready", ready: true });
    supervisor.stop();
    supervisor.dispose();
    expect(stops()).toBe(0);
  });

  test("launched bridge is stopped exactly once and stop() resets the projection", async () => {
    const { session, stops } = readySession();
    const supervisor = createAppleFmSupervisor(launcher({ outcome: { kind: "session", session } }));
    await supervisor.ensureStarted();
    expect(supervisor.stop()).toMatchObject({ state: "stopped", ready: false, mode: "none" });
    expect(stops()).toBe(1);
  });

  test("a crash after ready transitions to failed for that generation only", async () => {
    const holder: { crash: ((failureClass: string) => void) | null } = { crash: null };
    const { session } = readySession();
    const supervisor = createAppleFmSupervisor(
      launcher({ outcome: { kind: "session", session }, onLaunch: (onCrash) => (holder.crash = onCrash) }),
    );
    await supervisor.ensureStarted();
    expect(supervisor.status().state).toBe("ready");
    holder.crash?.("helper_crashed");
    expect(supervisor.status()).toMatchObject({ state: "failed", ready: false, unavailableReason: "helper_crashed" });
    supervisor.stop();
    holder.crash?.("helper_crashed");
    expect(supervisor.status().state).toBe("stopped");
  });

  test("dispose is idempotent and refuses further turns", async () => {
    const { session } = readySession();
    const supervisor = createAppleFmSupervisor(launcher({ outcome: { kind: "session", session } }));
    await supervisor.ensureStarted();
    supervisor.dispose();
    supervisor.dispose();
    expect(await supervisor.runTurn("hi")).toMatchObject({ outcome: "failed", failureClass: "unsupported_platform" });
  });
});

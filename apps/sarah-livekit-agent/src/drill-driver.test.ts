import { SARAH_VOICE_SETTLEMENT_PROTOCOL_VERSION } from "@openagentsinc/audio-contract";
import { describe, expect, test, vi } from "vite-plus/test";
import type { Room } from "@livekit/rtc-node";
import {
  SARAH_LIVEKIT_DRILL_SCHEMA,
  assertPublicSafeSarahLiveKitDrillObservation,
  digestDrillInstance,
  runSarahLiveKitDrill,
  type SarahLiveKitDrillFaultResult,
  type SarahLiveKitDrillInput,
  type SarahLiveKitDrillObservation,
} from "./drill-driver.js";
import { SARAH_LIVEKIT_SFU_LOSS_BOUND_MS } from "./failure-matrix.js";
import type { SarahLiveKitControlTerminal, SarahLiveKitLiveSession } from "./acceptance-livekit.js";
import type { SarahLiveKitAcceptanceScenario } from "./acceptance-harness.js";

const SFU_DIGEST = digestDrillInstance("livekit-server-abc");
const WORKER_DIGEST = digestDrillInstance("sarah-worker-xyz");

const scenario: SarahLiveKitAcceptanceScenario = {
  kind: "private",
  bearer: "drill-bearer",
  subscriberRef: "drill-subscriber",
  ownerRef: "owner-drill",
  deviceRef: "device-drill",
  threadRef: "thread-drill",
  sessionRef: "session-drill",
  generation: 1,
  pcm: new Uint8Array(48_000),
  roomContext: { kind: "private" },
};

/**
 * A clock whose `sleep` advances instantly.
 *
 * The driver's real waits are a deliberate live hold and a settlement poll, and
 * a test that actually waited them would be measuring `setTimeout`. Advancing
 * the clock keeps every ordering assertion exact and the suite fast.
 */
const fakeClock = (startAtMs = 1_000) => {
  let now = startAtMs;
  return {
    now: () => now,
    sleep: (durationMs: number) => {
      now += durationMs;
      return Promise.resolve();
    },
    advance: (durationMs: number) => {
      now += durationMs;
    },
  };
};

class FakeRoom {
  readonly listeners = new Map<string, Set<() => void>>();

  on(event: string, listener: () => void): this {
    (this.listeners.get(event) ?? this.listeners.set(event, new Set()).get(event))?.add(listener);
    return this;
  }

  off(event: string, listener: () => void): this {
    this.listeners.get(event)?.delete(listener);
    return this;
  }

  emit(event: string): void {
    for (const listener of this.listeners.get(event) ?? []) listener();
  }
}

type Harness = Readonly<{
  input: SarahLiveKitDrillInput;
  dependencies: Parameters<typeof runSarahLiveKitDrill>[1];
  clock: ReturnType<typeof fakeClock>;
  room: FakeRoom;
  released: () => number;
  faultCalls: () => number;
  resolveControlTerminal: (terminal: SarahLiveKitControlTerminal) => void;
}>;

const settlementBody = () => ({
  schema: SARAH_VOICE_SETTLEMENT_PROTOCOL_VERSION,
  sessionRef: "session-drill",
  state: "released" as const,
  creditMode: "metered" as const,
  finalChargeMsat: 4_200,
  spendableRemainingCreditMsat: 1_000,
  receiptRef: "receipt.sarah_voice_settlement.drill",
});

/**
 * @param settleAfterMs How long after the fault the settlement authority turns
 *   terminal, or `null` for a settlement that never arrives.
 */
const harness = (
  options: Readonly<{
    settleAfterMs: number | null;
    fault?: SarahLiveKitDrillFaultResult;
    injectFault?: SarahLiveKitDrillInput["injectFault"];
    billableSessions?: number;
    overrides?: Partial<SarahLiveKitDrillInput>;
  }>,
): Harness => {
  const clock = fakeClock();
  const room = new FakeRoom();
  const subscriberRoom = new FakeRoom();
  let releaseCount = 0;
  let faultCount = 0;
  let faultAtMs: number | undefined;
  let resolveTerminal: ((terminal: SarahLiveKitControlTerminal) => void) | undefined;
  const terminal = new Promise<SarahLiveKitControlTerminal>((resolve) => {
    resolveTerminal = resolve;
  });

  const http = vi.fn(async () => {
    if (
      options.settleAfterMs === null ||
      faultAtMs === undefined ||
      clock.now() < faultAtMs + options.settleAfterMs
    ) {
      return new Response(JSON.stringify({ error: "not_terminal" }), { status: 404 });
    }
    return Response.json(settlementBody());
  });

  const session = {
    kind: "private",
    identity: {
      ownerRef: scenario.ownerRef,
      deviceRef: scenario.deviceRef,
      threadRef: scenario.threadRef,
      sessionRef: scenario.sessionRef,
      generation: 1,
    },
    roomRef: "room-drill-private",
    participantRef: "participant-drill",
    sarahParticipantRef: "principal.sarah",
    timings: {
      startedAtMs: 1_000,
      admissionLatencyMs: 10,
      sessionLatencyMs: 20,
      roomConnectLatencyMs: 30,
      microphonePublishLatencyMs: 40,
      activeRoomStartedAtMs: 1_100,
      firstSarahAudioAtMs: 1_500,
      firstSarahAudioLatencyMs: 500,
    },
    room: room as unknown as Room,
    subscriberRoom: subscriberRoom as unknown as Room,
    control: {
      ready: Promise.resolve(1_200),
      terminal,
      interrupt: () => Promise.reject(new Error("unused")),
      close: () => Promise.resolve(),
      dispose: () => {},
    },
    output: undefined as never,
    subscriberOutput: undefined as never,
    fanoutAudio: Promise.resolve([1_500, 1_500] as const),
    clock: { now: clock.now, sleep: clock.sleep },
    http: http as never,
    unpublishMicrophone: () => Promise.resolve(),
    release: () => {
      releaseCount += 1;
      return Promise.resolve();
    },
  } satisfies Partial<SarahLiveKitLiveSession> as unknown as SarahLiveKitLiveSession;

  const injectFault =
    options.injectFault ??
    (() => {
      faultCount += 1;
      faultAtMs = clock.now();
      return Promise.resolve(
        options.fault ?? { targetInstanceDigest: SFU_DIGEST, workerInstanceDigest: WORKER_DIGEST },
      );
    });

  return {
    clock,
    room,
    released: () => releaseCount,
    faultCalls: () => faultCount,
    resolveControlTerminal: (value) => resolveTerminal?.(value),
    input: {
      scenario: "sfu_loss",
      session: scenario,
      boundMs: SARAH_LIVEKIT_SFU_LOSS_BOUND_MS,
      holdMs: 2_000,
      injectFault: (context) => {
        // The default injector records the fault instant; a supplied one is
        // responsible for its own bookkeeping.
        if (options.injectFault === undefined) {
          faultAtMs = clock.now();
        }
        return injectFault(context);
      },
      countBillableSessions: () => options.billableSessions ?? 1,
      ...options.overrides,
    },
    dependencies: { openSession: () => Promise.resolve(session) },
  };
};

/** Emit the media loss the SFU fault causes, so the default path is a pass. */
const withMediaLoss = (built: Harness): Harness => ({
  ...built,
  input: {
    ...built.input,
    injectFault: async (context) => {
      const result = await built.input.injectFault(context);
      built.room.emit("disconnected");
      return result;
    },
  },
});

const run = (built: Harness): Promise<SarahLiveKitDrillObservation> =>
  runSarahLiveKitDrill(built.input, built.dependencies);

describe("Sarah LiveKit single-session drill driver", () => {
  test("holds one session live, faults it, and bounds the settlement", async () => {
    const built = withMediaLoss(harness({ settleAfterMs: 4_000 }));
    const observation = await run(built);

    expect(observation.schema).toBe(SARAH_LIVEKIT_DRILL_SCHEMA);
    expect(observation.outcome).toBe("passed");
    expect(observation.contradictions).toEqual([]);
    expect(observation.faultAction).toBe("delete_exact_sfu_pod");
    expect(observation.concurrentBillableSessionCount).toBe(1);
    expect(observation.withinBound).toBe(true);
    expect(observation.settlement?.state).toBe("released");
    expect(observation.settlement?.receiptDigest).toMatch(/^sha256:[0-9a-f]{64}$/u);
    expect(built.released()).toBe(1);
  });

  test("records the fault instant separately from the session start", async () => {
    const built = withMediaLoss(harness({ settleAfterMs: 4_000 }));
    const observation = await run(built);

    // The bound is measured from the fault, so the two instants must not be
    // conflated: the session was live for the hold before anything was broken.
    expect(observation.sessionStartedAtMs).toBe(1_000);
    expect(observation.faultInjectedAtMs).toBe(1_000 + observation.holdMs);
    expect(observation.faultInjectedAtMs).toBeGreaterThan(observation.sessionStartedAtMs);
    expect(observation.faultToTerminalMs).toBe(
      (observation.settlement as { terminalObservedAtMs: number }).terminalObservedAtMs -
        observation.faultInjectedAtMs,
    );
    expect(observation.mediaLossDetectedWithinMs).toBe(0);
  });

  test("satisfies the sfu_loss sole-billable-session requirement and contradicts a second", async () => {
    const sole = await run(withMediaLoss(harness({ settleAfterMs: 4_000, billableSessions: 1 })));
    expect(sole.concurrentBillableSessionCount).toBe(1);
    expect(sole.contradictions).toEqual([]);

    const shared = await run(withMediaLoss(harness({ settleAfterMs: 4_000, billableSessions: 2 })));
    expect(shared.outcome).toBe("contradicted");
    expect(shared.contradictions).toContain("concurrent_billable_session_count_not_one");
    expect(shared.concurrentBillableSessionCount).toBe(2);
  });

  test("records an exceeded bound as contradicted rather than throwing", async () => {
    const built = withMediaLoss(
      harness({ settleAfterMs: SARAH_LIVEKIT_SFU_LOSS_BOUND_MS + 1_000 }),
    );
    const observation = await run(built);

    expect(observation.outcome).toBe("contradicted");
    expect(observation.contradictions).toEqual(["fault_to_terminal_exceeded_bound"]);
    expect(observation.withinBound).toBe(false);
    // The settlement is still preserved: a late settlement is a finding about
    // production, not a run to discard.
    expect(observation.settlement).not.toBeNull();
    expect(observation.faultToTerminalMs).toBeGreaterThan(SARAH_LIVEKIT_SFU_LOSS_BOUND_MS);
  });

  test("records a settlement that never arrives as contradicted with no settlement", async () => {
    const observation = await run(withMediaLoss(harness({ settleAfterMs: null })));

    expect(observation.outcome).toBe("contradicted");
    expect(observation.contradictions).toContain(
      "settlement_not_terminal_within_observation_window",
    );
    expect(observation.settlement).toBeNull();
    expect(observation.faultToTerminalMs).toBeNull();
    expect(observation.withinBound).toBe(false);
  });

  test("contradicts an sfu_loss fault that destroyed the Sarah worker instance", async () => {
    const observation = await run(
      withMediaLoss(
        harness({
          settleAfterMs: 4_000,
          fault: { targetInstanceDigest: WORKER_DIGEST, workerInstanceDigest: WORKER_DIGEST },
        }),
      ),
    );

    expect(observation.outcome).toBe("contradicted");
    expect(observation.contradictions).toContain("fault_destroyed_the_sarah_worker_instance");
  });

  test("contradicts an sfu_loss fault that produced no media loss", async () => {
    const observation = await run(harness({ settleAfterMs: 4_000 }));

    expect(observation.outcome).toBe("contradicted");
    expect(observation.contradictions).toContain("media_loss_not_observed");
    expect(observation.mediaLossObservedAtMs).toBeNull();
  });

  test("contradicts media loss that preceded the drill's own fault", async () => {
    const built = harness({ settleAfterMs: 4_000 });
    // Media the drill did not destroy died first, so the interval this run
    // measured is not attributable to its fault.
    const observation = await run({
      ...built,
      input: {
        ...built.input,
        countBillableSessions: () => {
          built.room.emit("disconnected");
          built.clock.advance(750);
          return 1;
        },
      },
    });

    expect(observation.outcome).toBe("contradicted");
    expect(observation.contradictions).toContain("media_loss_observed_before_the_fault");
    expect(observation.mediaLossDetectedWithinMs).toBe(-750);
  });

  test("excludes read-only target discovery from the bound", async () => {
    const built = harness({ settleAfterMs: 4_000 });
    const observation = await run({
      ...built,
      input: {
        ...built.input,
        injectFault: async (context) => {
          // Stand in for reading three instances' gauges and three workers' logs.
          built.clock.advance(9_000);
          const injectedAtMs = built.clock.now();
          const result = await built.input.injectFault(context);
          built.room.emit("disconnected");
          return { ...result, injectedAtMs };
        },
      },
    });

    // Nine seconds of preparation charged to a thirty second bound would
    // manufacture a contradiction out of the drill's own setup.
    expect(observation.faultDiscoveryMs).toBe(9_000);
    expect(observation.outcome).toBe("passed");
    expect(observation.faultToTerminalMs).toBeLessThanOrEqual(SARAH_LIVEKIT_SFU_LOSS_BOUND_MS);
  });

  test("refuses a fault instant outside the interval the driver bracketed", async () => {
    const built = harness({ settleAfterMs: 4_000 });
    await expect(
      run({
        ...built,
        input: {
          ...built.input,
          injectFault: async (context) => {
            const result = await built.input.injectFault(context);
            // An injector cannot claim the fault landed before the driver saw
            // the call start; that is how a bound would be talked down.
            return { ...result, injectedAtMs: built.clock.now() - 60_000 };
          },
        },
      }),
    ).rejects.toThrow("outside the interval the driver bracketed");
    expect(built.released()).toBe(1);
  });

  test("preserves the control channel's own terminal reason", async () => {
    const built = withMediaLoss(harness({ settleAfterMs: 4_000 }));
    built.resolveControlTerminal({ atMs: 2_500, kind: "closing", reason: "transport_error" });
    const observation = await run(built);

    expect(observation.controlTerminal).toEqual({
      atMs: 2_500,
      kind: "closing",
      reason: "transport_error",
    });
  });

  test("releases the live session when the fault injector fails", async () => {
    const built = harness({
      settleAfterMs: 4_000,
      injectFault: () => Promise.reject(new Error("kubectl delete refused")),
    });

    await expect(run(built)).rejects.toThrow("kubectl delete refused");
    // A fault that could not be injected is a drill that did not run, so it
    // raises rather than recording a contradiction — but the session it opened
    // is still cleaned up.
    expect(built.released()).toBe(1);
  });

  test("refuses an sfu_loss bound other than the one the failure matrix defines", async () => {
    await expect(
      run(harness({ settleAfterMs: 4_000, overrides: { boundMs: 60_000 } })),
    ).rejects.toThrow(`${SARAH_LIVEKIT_SFU_LOSS_BOUND_MS} ms bound`);
  });

  test("refuses an observation window that cannot outlive the bound", async () => {
    await expect(
      run(
        harness({
          settleAfterMs: 4_000,
          overrides: { observationWindowMs: SARAH_LIVEKIT_SFU_LOSS_BOUND_MS },
        }),
      ),
    ).rejects.toThrow("observation window must exceed the bound");
  });

  test("refuses a scenario with no fault to inject", async () => {
    await expect(
      run(
        harness({
          settleAfterMs: 4_000,
          overrides: {
            scenario: "success" as never,
            boundMs: 30_000,
          },
        }),
      ),
    ).rejects.toThrow("no fault to inject");
  });

  test("never opens a session for an invalid drill request", async () => {
    const built = harness({ settleAfterMs: 4_000, overrides: { boundMs: 0 } });
    await expect(run(built)).rejects.toThrow();
    expect(built.released()).toBe(0);
    expect(built.faultCalls()).toBe(0);
  });

  test("carries no room, owner, or grant material out of the session", async () => {
    const observation = await run(withMediaLoss(harness({ settleAfterMs: 4_000 })));
    expect(() => assertPublicSafeSarahLiveKitDrillObservation(observation)).not.toThrow();
    expect(JSON.stringify(observation)).not.toContain("room-drill-private");
    expect(JSON.stringify(observation)).not.toContain("owner-drill");
    expect(JSON.stringify(observation)).not.toContain("drill-bearer");
  });

  test("refuses to publish an observation carrying private material", () => {
    const withRoomRef = { schema: SARAH_LIVEKIT_DRILL_SCHEMA, roomRef: "room-drill-private" };
    expect(() =>
      assertPublicSafeSarahLiveKitDrillObservation(
        withRoomRef as unknown as SarahLiveKitDrillObservation,
      ),
    ).toThrow("forbidden field roomRef");

    const withOrigin = { schema: SARAH_LIVEKIT_DRILL_SCHEMA, note: "wss://livekit.openagents.com" };
    expect(() =>
      assertPublicSafeSarahLiveKitDrillObservation(
        withOrigin as unknown as SarahLiveKitDrillObservation,
      ),
    ).toThrow("private material");
  });

  test("digests a cluster instance name rather than carrying the address", () => {
    expect(digestDrillInstance("livekit-server-abc")).toMatch(/^sha256:[0-9a-f]{64}$/u);
    expect(digestDrillInstance("livekit-server-abc")).not.toContain("livekit-server");
    expect(() => digestDrillInstance("")).toThrow("invalid");
  });
});

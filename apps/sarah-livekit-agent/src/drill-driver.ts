import { createHash } from "node:crypto";
import { RoomEvent } from "@livekit/rtc-node";
import {
  SARAH_LIVEKIT_SFU_LOSS_BOUND_MS,
  EXPECTED_FAULT_ACTION,
  type SarahLiveKitFailureScenario,
} from "./failure-matrix.js";
import {
  SARAH_LIVEKIT_SETTLEMENT_POLL_INTERVAL_MS,
  openLiveSarahLiveKitSession,
  pollSarahLiveKitSettlement,
  type SarahLiveKitControlTerminal,
  type SarahLiveKitLiveDependencies,
  type SarahLiveKitLiveSession,
} from "./acceptance-livekit.js";
import {
  digestSettlementReceipt,
  type SarahLiveKitAcceptanceScenario,
} from "./acceptance-harness.js";

/**
 * The single-session failure-drill driver.
 *
 * The two-room acceptance harness is the only other session driver, and it
 * cannot run a drill. It requires both a private and a community scenario, runs
 * them concurrently, hard-asserts that their live windows overlapped, and
 * returns only after both have settled. Nothing in that shape can hold one
 * session open while a fault is injected into it, and `sfu_loss` additionally
 * requires `concurrentBillableSessionCount: 1`, which a driver that always runs
 * two sessions can never satisfy.
 *
 * This driver is the other half: exactly one session, brought live through the
 * production path, held open, faulted at a recorded instant, then watched until
 * its accounting is terminal.
 *
 * Three properties are load-bearing.
 *
 *   1. It reuses the production path. The bring-up is
 *      `openLiveSarahLiveKitSession`, factored out of the acceptance runner
 *      rather than reimplemented, so a drill observes the same admission,
 *      session, grant, control channel, and media publication a real generation
 *      uses. A drill driver with its own session implementation would prove
 *      something about the drill driver.
 *   2. The fault instant is separate from the session start. Every bound in the
 *      failure matrix is measured from the fault, and a session's own duration
 *      says nothing about how long the authority took to notice the fault.
 *   3. A failed drill is a result, not an exception. A bound that was exceeded,
 *      a settlement that never arrived, a fault aimed at the wrong instance —
 *      each is returned as `contradicted` with a named reason, because a driver
 *      that throws on a bad outcome teaches its operator to discard the runs
 *      that matter most.
 *
 * What it does NOT do: judge the worker-side terminal reason. The settlement
 * authority does not publish one, and the control channel's `closing.reason` is
 * a different, client-facing vocabulary. Both are recorded; neither is silently
 * mapped onto the matrix's worker-side reasons.
 */
export const SARAH_LIVEKIT_DRILL_SCHEMA = "openagents.sarah.livekit-drill-observation.v1" as const;

/** How long the session is held live and generating before the fault. */
export const SARAH_LIVEKIT_DRILL_DEFAULT_HOLD_MS = 2_000;

/**
 * Faults the driver can carry. `success` is excluded: its fault action is
 * `none`, so there is no instant to measure a bound from and nothing to hold a
 * session open for.
 */
export const SARAH_LIVEKIT_DRILL_SCENARIOS = [
  "cancellation",
  "timeout",
  "planned_worker_crash",
  "sfu_loss",
  "provider_disconnect",
  "hold_exhaustion",
  "reconnect",
] as const satisfies readonly SarahLiveKitFailureScenario[];
export type SarahLiveKitDrillScenario = (typeof SARAH_LIVEKIT_DRILL_SCENARIOS)[number];

/**
 * Every way a drill can contradict the row it was run for.
 *
 * Named rather than free text so a contradiction is comparable across runs and
 * cannot be softened by an operator's phrasing.
 */
export const SARAH_LIVEKIT_DRILL_CONTRADICTIONS = [
  "settlement_not_terminal_within_observation_window",
  "fault_to_terminal_exceeded_bound",
  "concurrent_billable_session_count_not_one",
  "fault_destroyed_the_sarah_worker_instance",
  "media_loss_observed_before_the_fault",
  "settlement_observed_before_media_loss",
  "media_loss_not_observed",
] as const;
export type SarahLiveKitDrillContradiction = (typeof SARAH_LIVEKIT_DRILL_CONTRADICTIONS)[number];

/**
 * What the fault injector is given.
 *
 * `roomRef` and `sessionRef` are private addressing the injector needs to aim at
 * exactly this generation. They are never carried into the observation.
 */
export type SarahLiveKitDrillFaultContext = Readonly<{
  scenario: SarahLiveKitDrillScenario;
  faultAction: (typeof EXPECTED_FAULT_ACTION)[SarahLiveKitDrillScenario];
  roomRef: string;
  /** The owner participant identity for this generation, minted per admission. */
  participantRef: string;
  sessionRef: string;
  ownerRef: string;
  /**
   * When this session started. Bounds any log scan the injector runs: the
   * participant ref is stable per owner, so an unbounded window can match an
   * earlier generation by the same owner on a different instance.
   */
  sessionStartedAtMs: number;
  /** Close the packaged control channel, for client-side faults. */
  requestClientCancel: () => Promise<void>;
}>;

export type SarahLiveKitDrillFaultResult = Readonly<{
  /**
   * Digest of the exact instance the fault destroyed, when it names one. A
   * digest and not a name, because a pod name is a cluster address.
   */
  targetInstanceDigest?: string;
  /**
   * Digest of the Sarah worker instance. Required for `sfu_loss`: the whole
   * point of deleting one `livekit-server` pod rather than a node is that the
   * worker survives, and an observation that cannot show which instance the
   * worker was on cannot show that.
   */
  workerInstanceDigest?: string;
  /** Whether the worker instance was still present after the fault. */
  workerInstanceSurvived?: boolean;
  /**
   * The instant the destructive action was issued, when the injector had to do
   * read-only work first.
   *
   * Target discovery is not part of the fault: reading three instances' gauges
   * and three workers' logs takes seconds, and charging that to a thirty second
   * bound would manufacture contradictions out of the drill's own preparation.
   *
   * This stays ungameable because the driver brackets the whole call and refuses
   * any value outside `[before, after]`. An injector can exclude its own
   * read-only preamble and nothing else — it cannot claim an instant earlier
   * than the driver saw it start or later than the driver saw it return.
   */
  injectedAtMs?: number;
}>;

export type SarahLiveKitDrillSettlement = Readonly<{
  state: "settled" | "released";
  creditMode: "metered" | "staging_owner_entitlement";
  finalChargeMsat: number;
  receiptDigest: `sha256:${string}`;
  terminalObservedAtMs: number;
  lastNonTerminalObservedAtMs: number;
}>;

export type SarahLiveKitDrillObservation = Readonly<{
  schema: typeof SARAH_LIVEKIT_DRILL_SCHEMA;
  scenario: SarahLiveKitDrillScenario;
  faultAction: (typeof EXPECTED_FAULT_ACTION)[SarahLiveKitDrillScenario];
  roomKind: "private" | "community";
  outcome: "passed" | "contradicted";
  /** Every requirement this run failed. Empty if and only if it passed. */
  contradictions: readonly SarahLiveKitDrillContradiction[];
  sessionStartedAtMs: number;
  /** First audible Sarah frame. The session is demonstrably generating here. */
  sessionLiveAtMs: number;
  holdMs: number;
  /**
   * When the destructive action was issued.
   *
   * The driver stamps immediately before and immediately after the injector and
   * accepts the injector's own instant only inside that bracket, defaulting to
   * the leading stamp. So the value can exclude read-only target discovery and
   * nothing else: it can never be earlier than the driver saw the call start,
   * nor later than it saw the call return.
   */
  faultInjectedAtMs: number;
  mediaLossObservedAtMs: number | null;
  controlTerminal: SarahLiveKitControlTerminal | null;
  settlement: SarahLiveKitDrillSettlement | null;
  boundMs: number;
  faultToTerminalMs: number | null;
  mediaLossDetectedWithinMs: number | null;
  withinBound: boolean;
  /**
   * Billable Sarah sessions live anywhere in the cluster, read at the fault
   * instant. `sfu_loss` requires exactly one, because deleting an SFU pod takes
   * every room on that instance.
   */
  concurrentBillableSessionCount: number;
  faultTargetDigest: string | null;
  workerInstanceDigest: string | null;
  workerInstanceSurvived: boolean | null;
  /** How long the injector's read-only target discovery took. */
  faultDiscoveryMs: number;
  observationWindowMs: number;
  settlementPollIntervalMs: number;
  limitations: readonly [
    "terminal_settlement_instant_is_observed_not_authoritative",
    "worker_side_terminal_reason_is_not_published_by_the_settlement_authority",
    "fault_execution_is_delegated_to_the_injector_and_attested_by_digest",
  ];
}>;

export type SarahLiveKitDrillInput = Readonly<{
  scenario: SarahLiveKitDrillScenario;
  /**
   * The one session the drill runs. Private or community, never both: the
   * concurrency the acceptance harness requires is exactly what makes an
   * SFU-loss fault unattributable and customer-affecting.
   */
  session: SarahLiveKitAcceptanceScenario;
  /** Measured from the fault, never from the session start. */
  boundMs: number;
  /** How long the observer keeps reading settlement. Must exceed the bound. */
  observationWindowMs?: number;
  holdMs?: number;
  injectFault: (context: SarahLiveKitDrillFaultContext) => Promise<SarahLiveKitDrillFaultResult>;
  /** Read at the fault instant, which is the only instant the count is about. */
  countBillableSessions: () => Promise<number> | number;
}>;

export type SarahLiveKitDrillDependencies = SarahLiveKitLiveDependencies &
  Readonly<{
    openSession?: (
      scenario: SarahLiveKitAcceptanceScenario,
      dependencies: SarahLiveKitLiveDependencies,
    ) => Promise<SarahLiveKitLiveSession>;
  }>;

const SHA256 = /^sha256:[0-9a-f]{64}$/u;

const assertDigest = (value: string | undefined, label: string): void => {
  if (value !== undefined && !SHA256.test(value)) {
    throw new Error(`${label} must be a sha256 digest`);
  }
};

/** Digest a cluster instance name so the observation never carries an address. */
export const digestDrillInstance = (instanceName: string): `sha256:${string}` => {
  if (instanceName.trim() === "" || instanceName.length > 256) {
    throw new Error("drill instance name is invalid");
  }
  return `sha256:${createHash("sha256").update(instanceName).digest("hex")}`;
};

/**
 * Watch both rooms for the first media disconnection.
 *
 * Registered before the fault, so the handler cannot miss a loss that happens
 * faster than the injector returns.
 */
const observeMediaLoss = (session: SarahLiveKitLiveSession) => {
  let observedAtMs: number | undefined;
  const rooms = [session.room, session.subscriberRoom];
  const listener = () => {
    observedAtMs ??= session.clock.now();
  };
  for (const room of rooms) room.on(RoomEvent.Disconnected, listener);
  return {
    observedAtMs: () => observedAtMs,
    close: () => {
      for (const room of rooms) room.off(RoomEvent.Disconnected, listener);
    },
  };
};

export const runSarahLiveKitDrill = async (
  input: SarahLiveKitDrillInput,
  dependencies: SarahLiveKitDrillDependencies = {},
): Promise<SarahLiveKitDrillObservation> => {
  if (!SARAH_LIVEKIT_DRILL_SCENARIOS.includes(input.scenario)) {
    throw new Error("drill scenario has no fault to inject");
  }
  // The matrix owns the SFU-loss bound. A driver that accepted a different one
  // could produce an observation the receipt validator then rejects, or worse,
  // one it accepts against a bound nobody agreed to.
  if (input.scenario === "sfu_loss" && input.boundMs !== SARAH_LIVEKIT_SFU_LOSS_BOUND_MS) {
    throw new Error(
      `sfu_loss must use the ${SARAH_LIVEKIT_SFU_LOSS_BOUND_MS} ms bound the failure matrix defines`,
    );
  }
  if (!Number.isSafeInteger(input.boundMs) || input.boundMs <= 0) {
    throw new Error("drill bound must be a positive whole number of milliseconds");
  }
  const holdMs = input.holdMs ?? SARAH_LIVEKIT_DRILL_DEFAULT_HOLD_MS;
  if (!Number.isSafeInteger(holdMs) || holdMs <= 0) {
    throw new Error("drill hold must be a positive whole number of milliseconds");
  }
  const observationWindowMs = input.observationWindowMs ?? input.boundMs * 2;
  if (!Number.isSafeInteger(observationWindowMs) || observationWindowMs <= input.boundMs) {
    // An observation window equal to the bound cannot tell a settlement that
    // was one millisecond late from one that never happened, and those are very
    // different findings.
    throw new Error("drill observation window must exceed the bound it is measuring");
  }

  const faultAction = EXPECTED_FAULT_ACTION[input.scenario];
  const openSession = dependencies.openSession ?? openLiveSarahLiveKitSession;
  const session = await openSession(input.session, dependencies);
  const { clock, http } = session;
  const mediaLoss = observeMediaLoss(session);
  // Resolves and never rejects, so a destroyed transport is preserved as
  // evidence instead of surfacing as an unobserved rejection.
  let controlTerminal: SarahLiveKitControlTerminal | null = null;
  void session.control.terminal.then((terminal) => {
    controlTerminal = terminal;
  });

  try {
    await clock.sleep(holdMs);

    const concurrentBillableSessionCount = await input.countBillableSessions();
    if (
      !Number.isSafeInteger(concurrentBillableSessionCount) ||
      concurrentBillableSessionCount < 1
    ) {
      // The drill's own session is live, so a count below one is a broken
      // counter rather than a finding about production.
      throw new Error("drill billable session count is invalid");
    }

    const faultRequestedAtMs = clock.now();
    const fault = await input.injectFault({
      scenario: input.scenario,
      faultAction,
      roomRef: session.roomRef,
      participantRef: session.participantRef,
      sessionStartedAtMs: session.timings.startedAtMs,
      sessionRef: input.session.sessionRef,
      ownerRef: input.session.ownerRef,
      requestClientCancel: () => session.control.close(),
    });
    const faultReturnedAtMs = clock.now();
    assertDigest(fault.targetInstanceDigest, "drill fault target digest");
    assertDigest(fault.workerInstanceDigest, "drill worker instance digest");
    const reported = fault.injectedAtMs;
    if (
      reported !== undefined &&
      (!Number.isSafeInteger(reported) ||
        reported < faultRequestedAtMs ||
        reported > faultReturnedAtMs)
    ) {
      throw new Error("drill fault instant is outside the interval the driver bracketed");
    }
    const faultInjectedAtMs = reported ?? faultRequestedAtMs;

    const reading = await pollSarahLiveKitSettlement(http, clock, input.session, {
      windowMs: observationWindowMs,
      intervalMs: SARAH_LIVEKIT_SETTLEMENT_POLL_INTERVAL_MS,
    });

    const mediaLossObservedAtMs = mediaLoss.observedAtMs() ?? null;
    const settlement: SarahLiveKitDrillSettlement | null =
      reading === null
        ? null
        : {
            state: reading.settlement.state,
            creditMode: reading.settlement.creditMode,
            finalChargeMsat: reading.settlement.finalChargeMsat,
            receiptDigest: digestSettlementReceipt(reading.settlement.receiptRef),
            terminalObservedAtMs: reading.terminalObservedAtMs,
            lastNonTerminalObservedAtMs: reading.lastNonTerminalObservedAtMs,
          };
    const faultToTerminalMs =
      settlement === null ? null : settlement.terminalObservedAtMs - faultInjectedAtMs;
    const mediaLossDetectedWithinMs =
      mediaLossObservedAtMs === null ? null : mediaLossObservedAtMs - faultInjectedAtMs;

    const contradictions: SarahLiveKitDrillContradiction[] = [];
    if (settlement === null || faultToTerminalMs === null) {
      contradictions.push("settlement_not_terminal_within_observation_window");
    } else if (faultToTerminalMs > input.boundMs) {
      contradictions.push("fault_to_terminal_exceeded_bound");
    }
    if (mediaLossDetectedWithinMs !== null && mediaLossDetectedWithinMs < 0) {
      contradictions.push("media_loss_observed_before_the_fault");
    }
    if (
      settlement !== null &&
      mediaLossObservedAtMs !== null &&
      settlement.terminalObservedAtMs < mediaLossObservedAtMs
    ) {
      contradictions.push("settlement_observed_before_media_loss");
    }
    if (input.scenario === "sfu_loss") {
      if (concurrentBillableSessionCount !== 1) {
        contradictions.push("concurrent_billable_session_count_not_one");
      }
      if (mediaLossObservedAtMs === null) {
        // Destroying the instance carrying this room and observing no media
        // loss means the fault did not reach the room under test.
        contradictions.push("media_loss_not_observed");
      }
      if (
        fault.targetInstanceDigest !== undefined &&
        fault.targetInstanceDigest === fault.workerInstanceDigest
      ) {
        contradictions.push("fault_destroyed_the_sarah_worker_instance");
      }
    }

    return {
      schema: SARAH_LIVEKIT_DRILL_SCHEMA,
      scenario: input.scenario,
      faultAction,
      roomKind: session.kind,
      outcome: contradictions.length === 0 ? "passed" : "contradicted",
      contradictions,
      sessionStartedAtMs: session.timings.startedAtMs,
      sessionLiveAtMs: session.timings.firstSarahAudioAtMs,
      holdMs,
      faultInjectedAtMs,
      mediaLossObservedAtMs,
      controlTerminal,
      settlement,
      boundMs: input.boundMs,
      faultToTerminalMs,
      mediaLossDetectedWithinMs,
      withinBound: faultToTerminalMs !== null && faultToTerminalMs <= input.boundMs,
      concurrentBillableSessionCount,
      faultTargetDigest: fault.targetInstanceDigest ?? null,
      workerInstanceDigest: fault.workerInstanceDigest ?? null,
      workerInstanceSurvived: fault.workerInstanceSurvived ?? null,
      faultDiscoveryMs: faultInjectedAtMs - faultRequestedAtMs,
      observationWindowMs,
      settlementPollIntervalMs: SARAH_LIVEKIT_SETTLEMENT_POLL_INTERVAL_MS,
      limitations: [
        "terminal_settlement_instant_is_observed_not_authoritative",
        "worker_side_terminal_reason_is_not_published_by_the_settlement_authority",
        "fault_execution_is_delegated_to_the_injector_and_attested_by_digest",
      ],
    };
  } finally {
    mediaLoss.close();
    await session.release();
  }
};

const PUBLIC_UNSAFE_VALUE =
  /(?:eyJ[A-Za-z0-9_-]{20,}|sk-[A-Za-z0-9_-]{20,}|oa_omega_[A-Za-z0-9_-]{20,}|wss?:\/\/|https?:\/\/)/iu;

/**
 * Refuse to hand out a drill observation that carries private material.
 *
 * The observation is assembled from a session that holds bearers, grants,
 * tickets, room refs, and owner refs. None of them are copied into it, and this
 * check keeps a later edit from quietly widening the projection.
 */
export const assertPublicSafeSarahLiveKitDrillObservation = (
  observation: SarahLiveKitDrillObservation,
): void => {
  const forbiddenKeys = new Set([
    "authorization",
    "bearer",
    "ticket",
    "grant",
    "participantgrant",
    "roomref",
    "ownerref",
    "deviceref",
    "sessionref",
    "threadref",
    "communityref",
    "channelref",
    "pcm",
    "address",
    "port",
    "url",
  ]);
  const walk = (value: unknown): void => {
    if (Array.isArray(value)) {
      value.forEach(walk);
      return;
    }
    if (typeof value === "object" && value !== null) {
      for (const [key, child] of Object.entries(value)) {
        if (forbiddenKeys.has(key.toLowerCase())) {
          throw new Error(`Sarah LiveKit drill observation contains forbidden field ${key}`);
        }
        walk(child);
      }
      return;
    }
    if (typeof value === "string" && PUBLIC_UNSAFE_VALUE.test(value)) {
      throw new Error("Sarah LiveKit drill observation contains private material");
    }
  };
  walk(observation);
};

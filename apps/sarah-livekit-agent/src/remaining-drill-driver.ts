import { createHash, randomUUID } from "node:crypto";
import {
  SARAH_LIVEKIT_SETTLEMENT_POLL_INTERVAL_MS,
  openLiveSarahLiveKitSession,
  pollSarahLiveKitSettlement,
  type SarahLiveKitLiveDependencies,
  type SarahLiveKitLiveSession,
} from "./acceptance-livekit.js";
import type { SarahLiveKitAcceptanceScenario } from "./acceptance-harness.js";

export const SARAH_LIVEKIT_REMAINING_DRILL_PRIVATE_SCHEMA =
  "openagents.sarah.livekit-remaining-drill-private.v1" as const;
export const SARAH_LIVEKIT_REMAINING_DRILL_RECEIPT_SCHEMA =
  "openagents.sarah.livekit-remaining-drill-receipt.v1" as const;

export const SARAH_LIVEKIT_REMAINING_DRILL_SCENARIOS = [
  "provider_disconnect",
  "reconnect",
] as const;
export const SARAH_LIVEKIT_RETIRED_DRILL_SCENARIOS = [
  {
    scenario: "hold_exhaustion",
    classification: "not_applicable_removed",
    authority: "owner_waived_unmetered_v1",
    reason: "Sarah voice admission creates no credit hold or ledger charge",
  },
] as const;
export type SarahLiveKitRemainingDrillScenario =
  (typeof SARAH_LIVEKIT_REMAINING_DRILL_SCENARIOS)[number];

export type SarahLiveKitAuthoritySnapshot = Readonly<{
  sessionRef: string;
  generation: number;
  state: "reserved" | "connected" | "accounting_uncertain" | "settled" | "released";
  closeReason: string | null;
  startedAtMs: number;
  terminalAtMs: number | null;
  reservationRef: string;
  settlementReceiptRef: string | null;
  workerJobRef: string | null;
  providerSessionRefDigest: string | null;
  reservedMsat: number;
  chargedMsat: number;
  inputTokens: number;
  outputTokens: number;
  cachedInputTokens: number;
  audioInputTokens: number;
  audioOutputTokens: number;
  responseCount: number;
  transcriptionCount: number;
  cancelledResponseCount: number;
  terminalEventCount: number;
  workerJobCount: number;
  providerSessionCount: number;
  activityAfterTerminalCount: number;
  providerDisconnectApplied: boolean;
  providerDisconnectRequestRef: string | null;
}>;

type ProviderDisconnectResult = Readonly<{
  requestRef: string;
  sessionRef: string;
  generation: number;
  providerSessionRefDigest: string;
  state: "requested" | "applied";
  replayed: boolean;
  sharedInfrastructureMutated: false;
}>;

export type SarahLiveKitRemainingDrillPrivateObservation = Readonly<{
  schema: typeof SARAH_LIVEKIT_REMAINING_DRILL_PRIVATE_SCHEMA;
  scenario: SarahLiveKitRemainingDrillScenario;
  environment: "production";
  sourceRevision: string;
  workerImageDigest: string;
  observedAt: string;
  outcome: "passed";
  turnsSent: number;
  previous: SarahLiveKitAuthoritySnapshot;
  fresh: SarahLiveKitAuthoritySnapshot | null;
  providerDisconnect: ProviderDisconnectResult | null;
}>;

export type SarahLiveKitRemainingDrillReceipt = Readonly<{
  schema: typeof SARAH_LIVEKIT_REMAINING_DRILL_RECEIPT_SCHEMA;
  scenario: SarahLiveKitRemainingDrillScenario;
  issueRef: "github-issue-ref://OpenAgentsInc/openagents/9285";
  environment: "production";
  sourceRevision: string;
  workerImageDigest: string;
  observedAt: string;
  outcome: "passed";
  turnsSent: number;
  previous: PublicAuthoritySnapshot;
  fresh: PublicAuthoritySnapshot | null;
  providerDisconnectApplied: boolean;
  noSharedInfrastructureMutation: true;
  noWorkerOverlap: true;
  noProviderOverlap: true;
  settledGenerationRevived: false;
  resultDigest: `sha256:${string}`;
}>;

type PublicAuthoritySnapshot = Readonly<{
  generationDigest: `sha256:${string}`;
  jobDigest: `sha256:${string}`;
  providerSessionDigest: `sha256:${string}`;
  holdDigest: `sha256:${string}`;
  settlementDigest: `sha256:${string}`;
  terminalReason: string;
  durationMs: number;
  reservedMsat: number;
  chargedMsat: number;
  releasedMsat: number;
  usage: Readonly<{
    inputTokens: number;
    outputTokens: number;
    cachedInputTokens: number;
    audioInputTokens: number;
    audioOutputTokens: number;
    responseCount: number;
    transcriptionCount: number;
    cancelledResponseCount: number;
  }>;
  terminalEventCount: 1;
  workerJobCount: 1;
  providerSessionCount: 1;
  activityAfterTerminalCount: 0;
}>;

export type SarahLiveKitRemainingDrillInput = Readonly<{
  scenario: SarahLiveKitRemainingDrillScenario;
  session: SarahLiveKitAcceptanceScenario;
  freshSession?: SarahLiveKitAcceptanceScenario;
  sourceRevision: string;
  workerImageDigest: string;
  observationWindowMs: number;
  authorityPollIntervalMs?: number;
}>;

export type SarahLiveKitRemainingDrillDependencies = SarahLiveKitLiveDependencies &
  Readonly<{
    openSession?: (
      scenario: SarahLiveKitAcceptanceScenario,
      dependencies: SarahLiveKitLiveDependencies,
    ) => Promise<SarahLiveKitLiveSession>;
    readAuthority: (sessionRef: string) => Promise<SarahLiveKitAuthoritySnapshot | null>;
    requestProviderDisconnect?: (
      input: Readonly<{
        requestRef: string;
        sessionRef: string;
        generation: number;
        providerSessionRefDigest: string;
      }>,
    ) => Promise<ProviderDisconnectResult>;
  }>;

const SHA256 = /^[a-f0-9]{64}$/u;
const COMMIT = /^[a-f0-9]{40}$/u;
const terminal = (state: SarahLiveKitAuthoritySnapshot["state"]): boolean =>
  state === "accounting_uncertain" || state === "settled" || state === "released";
const digest = (value: string): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`;
const PUBLIC_TERMINAL_REASONS = new Set([
  "completed",
  "participant_left",
  "membership_revoked",
  "hold_exhausted",
  "operator_stop",
  "provider_disconnect",
  "provider_mismatch",
  "session_expired",
  "worker_shutdown",
  "worker_error",
]);

const validateSnapshot = (value: SarahLiveKitAuthoritySnapshot): void => {
  if (!Number.isSafeInteger(value.generation) || value.generation < 1) {
    throw new Error("Sarah authority returned an invalid generation");
  }
  for (const [name, amount] of Object.entries({
    reservedMsat: value.reservedMsat,
    chargedMsat: value.chargedMsat,
    inputTokens: value.inputTokens,
    outputTokens: value.outputTokens,
    cachedInputTokens: value.cachedInputTokens,
    audioInputTokens: value.audioInputTokens,
    audioOutputTokens: value.audioOutputTokens,
    responseCount: value.responseCount,
    transcriptionCount: value.transcriptionCount,
    cancelledResponseCount: value.cancelledResponseCount,
    terminalEventCount: value.terminalEventCount,
    workerJobCount: value.workerJobCount,
    providerSessionCount: value.providerSessionCount,
    activityAfterTerminalCount: value.activityAfterTerminalCount,
  })) {
    if (!Number.isSafeInteger(amount) || amount < 0) {
      throw new Error(`Sarah authority returned an invalid ${name}`);
    }
  }
  if (value.providerSessionRefDigest !== null && !SHA256.test(value.providerSessionRefDigest)) {
    throw new Error("Sarah authority returned an invalid provider digest");
  }
  if (terminal(value.state)) {
    if (
      value.terminalAtMs === null ||
      value.terminalAtMs <= value.startedAtMs ||
      value.closeReason === null ||
      !PUBLIC_TERMINAL_REASONS.has(value.closeReason) ||
      value.settlementReceiptRef === null
    ) {
      throw new Error("Sarah terminal authority is incomplete");
    }
  }
};

const readExactAuthority = async (
  dependencies: SarahLiveKitRemainingDrillDependencies,
  sessionRef: string,
): Promise<SarahLiveKitAuthoritySnapshot> => {
  const value = await dependencies.readAuthority(sessionRef);
  if (value === null || value.sessionRef !== sessionRef) {
    throw new Error("Sarah authority did not return the exact drill session");
  }
  validateSnapshot(value);
  return value;
};

const waitForAuthority = async (
  session: SarahLiveKitLiveSession,
  dependencies: SarahLiveKitRemainingDrillDependencies,
  observationWindowMs: number,
  pollIntervalMs: number,
  accept: (snapshot: SarahLiveKitAuthoritySnapshot) => boolean,
): Promise<SarahLiveKitAuthoritySnapshot> => {
  const deadline = session.clock.now() + observationWindowMs;
  while (true) {
    // Sequential reads preserve the authority state transition being measured.
    // eslint-disable-next-line no-await-in-loop
    const snapshot = await readExactAuthority(dependencies, session.identity.sessionRef);
    if (accept(snapshot)) return snapshot;
    if (session.clock.now() >= deadline) {
      throw new Error("Sarah authority did not reach the required drill state within the window");
    }
    // The next read is authorized only because the current read did not satisfy the predicate.
    // eslint-disable-next-line no-await-in-loop
    await session.clock.sleep(pollIntervalMs);
  }
};

const assertTerminalAuthority = (
  snapshot: SarahLiveKitAuthoritySnapshot,
  expectedReason: string | null,
): void => {
  if (!terminal(snapshot.state)) throw new Error("Sarah drill authority is not terminal");
  if (expectedReason !== null && snapshot.closeReason !== expectedReason) {
    throw new Error(
      `Sarah drill ended as ${snapshot.closeReason ?? "unknown"}, not ${expectedReason}`,
    );
  }
  if (
    snapshot.terminalEventCount !== 1 ||
    snapshot.workerJobCount !== 1 ||
    snapshot.providerSessionCount !== 1 ||
    snapshot.activityAfterTerminalCount !== 0 ||
    snapshot.workerJobRef === null ||
    snapshot.providerSessionRefDigest === null
  ) {
    throw new Error("Sarah drill authority reported overlap, revival, or incomplete identity");
  }
  if (snapshot.chargedMsat > snapshot.reservedMsat) {
    throw new Error("Sarah drill accounting exceeded the admitted hold");
  }
};

const settleAfterTerminal = async (
  session: SarahLiveKitLiveSession,
  scenario: SarahLiveKitAcceptanceScenario,
  observationWindowMs: number,
): Promise<void> => {
  const settlement = await pollSarahLiveKitSettlement(session.http, session.clock, scenario, {
    windowMs: observationWindowMs,
    intervalMs: SARAH_LIVEKIT_SETTLEMENT_POLL_INTERVAL_MS,
  });
  if (settlement === null) throw new Error("Sarah drill settlement was not readable as terminal");
};

const closeIfActive = async (
  session: SarahLiveKitLiveSession,
  dependencies: SarahLiveKitRemainingDrillDependencies,
  observationWindowMs: number,
  pollIntervalMs: number,
): Promise<SarahLiveKitAuthoritySnapshot> => {
  let snapshot = await readExactAuthority(dependencies, session.identity.sessionRef);
  if (!terminal(snapshot.state)) {
    await session.control.close();
    snapshot = await waitForAuthority(
      session,
      dependencies,
      observationWindowMs,
      pollIntervalMs,
      (candidate) => terminal(candidate.state),
    );
  }
  return snapshot;
};

const runSingleSession = async (
  input: SarahLiveKitRemainingDrillInput,
  dependencies: SarahLiveKitRemainingDrillDependencies,
  session: SarahLiveKitLiveSession,
  pollIntervalMs: number,
): Promise<
  Readonly<{
    turnsSent: number;
    authority: SarahLiveKitAuthoritySnapshot;
    providerDisconnect: ProviderDisconnectResult | null;
  }>
> => {
  if (input.scenario === "provider_disconnect") {
    const requestProviderDisconnect = dependencies.requestProviderDisconnect;
    if (requestProviderDisconnect === undefined) {
      throw new Error("provider_disconnect requires the generation-scoped acceptance client");
    }
    const active = await waitForAuthority(
      session,
      dependencies,
      input.observationWindowMs,
      pollIntervalMs,
      (snapshot) => snapshot.state === "connected" && snapshot.providerSessionRefDigest !== null,
    );
    const providerSessionRefDigest = active.providerSessionRefDigest;
    if (providerSessionRefDigest === null) {
      throw new Error("provider_disconnect has no provider authority digest");
    }
    const request = await requestProviderDisconnect({
      requestRef: `acceptance:provider-disconnect:${randomUUID()}`,
      sessionRef: active.sessionRef,
      generation: active.generation,
      providerSessionRefDigest,
    });
    if (
      request.sessionRef !== active.sessionRef ||
      request.generation !== active.generation ||
      request.providerSessionRefDigest !== providerSessionRefDigest ||
      request.sharedInfrastructureMutated !== false
    ) {
      throw new Error("provider-disconnect acceptance response disagreed with exact authority");
    }
    const authority = await waitForAuthority(
      session,
      dependencies,
      input.observationWindowMs,
      pollIntervalMs,
      (snapshot) => terminal(snapshot.state),
    );
    assertTerminalAuthority(authority, "provider_disconnect");
    if (
      !authority.providerDisconnectApplied ||
      authority.providerDisconnectRequestRef !== request.requestRef
    ) {
      throw new Error("provider-disconnect fault was not durably applied to this generation");
    }
    return { turnsSent: 1, authority, providerDisconnect: request };
  }

  throw new Error("single-session remaining drill scenario is unsupported");
};

export const runSarahLiveKitRemainingDrill = async (
  input: SarahLiveKitRemainingDrillInput,
  dependencies: SarahLiveKitRemainingDrillDependencies,
): Promise<SarahLiveKitRemainingDrillPrivateObservation> => {
  if (!SARAH_LIVEKIT_REMAINING_DRILL_SCENARIOS.includes(input.scenario)) {
    throw new Error("remaining Sarah drill scenario is invalid");
  }
  if (
    !COMMIT.test(input.sourceRevision) ||
    !/^sha256:[a-f0-9]{64}$/u.test(input.workerImageDigest)
  ) {
    throw new Error("remaining Sarah drill candidate identity is invalid");
  }
  if (!Number.isSafeInteger(input.observationWindowMs) || input.observationWindowMs <= 0) {
    throw new Error("remaining Sarah drill observation window is invalid");
  }
  const pollIntervalMs = input.authorityPollIntervalMs ?? 1_000;
  if (!Number.isSafeInteger(pollIntervalMs) || pollIntervalMs <= 0) {
    throw new Error("remaining Sarah drill authority poll interval is invalid");
  }
  const openSession = dependencies.openSession ?? openLiveSarahLiveKitSession;
  const previousSession = await openSession(input.session, dependencies);
  let previous: SarahLiveKitAuthoritySnapshot;
  let fresh: SarahLiveKitAuthoritySnapshot | null = null;
  let turnsSent = 1;
  let providerDisconnect: ProviderDisconnectResult | null = null;
  try {
    if (input.scenario === "reconnect") {
      const freshInput = input.freshSession;
      if (
        freshInput === undefined ||
        freshInput.generation <= input.session.generation ||
        freshInput.sessionRef === input.session.sessionRef ||
        freshInput.ownerRef !== input.session.ownerRef ||
        freshInput.deviceRef !== input.session.deviceRef ||
        freshInput.threadRef !== input.session.threadRef ||
        freshInput.kind !== input.session.kind ||
        JSON.stringify(freshInput.roomContext) !== JSON.stringify(input.session.roomContext)
      ) {
        throw new Error(
          "reconnect requires the same authority and context on a distinct, strictly later generation",
        );
      }
      previous = await closeIfActive(
        previousSession,
        dependencies,
        input.observationWindowMs,
        pollIntervalMs,
      );
      assertTerminalAuthority(previous, null);
      await settleAfterTerminal(previousSession, input.session, input.observationWindowMs);
      await previousSession.release();

      const freshSession = await openSession(freshInput, dependencies);
      try {
        const activeFresh = await readExactAuthority(dependencies, freshInput.sessionRef);
        if (
          activeFresh.generation <= previous.generation ||
          activeFresh.startedAtMs <= (previous.terminalAtMs ?? Number.MAX_SAFE_INTEGER) ||
          terminal(activeFresh.state)
        ) {
          throw new Error(
            "fresh reconnect generation overlapped or did not follow the terminal one",
          );
        }
        const oldAfterFreshAdmission = await readExactAuthority(dependencies, previous.sessionRef);
        assertTerminalAuthority(oldAfterFreshAdmission, previous.closeReason);
        if (
          oldAfterFreshAdmission.terminalAtMs !== previous.terminalAtMs ||
          oldAfterFreshAdmission.activityAfterTerminalCount !== 0
        ) {
          throw new Error("fresh reconnect admission revived the settled generation");
        }
        fresh = await closeIfActive(
          freshSession,
          dependencies,
          input.observationWindowMs,
          pollIntervalMs,
        );
        assertTerminalAuthority(fresh, null);
        await settleAfterTerminal(freshSession, freshInput, input.observationWindowMs);
      } finally {
        await freshSession.release();
      }
    } else {
      const result = await runSingleSession(input, dependencies, previousSession, pollIntervalMs);
      previous = result.authority;
      turnsSent = result.turnsSent;
      providerDisconnect = result.providerDisconnect;
    }
  } finally {
    await previousSession.release();
  }

  return {
    schema: SARAH_LIVEKIT_REMAINING_DRILL_PRIVATE_SCHEMA,
    scenario: input.scenario,
    environment: "production",
    sourceRevision: input.sourceRevision,
    workerImageDigest: input.workerImageDigest,
    observedAt: new Date().toISOString(),
    outcome: "passed",
    turnsSent,
    previous,
    fresh,
    providerDisconnect,
  };
};

const publicSnapshot = (snapshot: SarahLiveKitAuthoritySnapshot): PublicAuthoritySnapshot => {
  assertTerminalAuthority(snapshot, null);
  const terminalAtMs = snapshot.terminalAtMs;
  const workerJobRef = snapshot.workerJobRef;
  const providerDigest = snapshot.providerSessionRefDigest;
  const settlementRef = snapshot.settlementReceiptRef;
  if (
    terminalAtMs === null ||
    workerJobRef === null ||
    providerDigest === null ||
    settlementRef === null ||
    snapshot.closeReason === null
  ) {
    throw new Error("remaining Sarah drill terminal projection is incomplete");
  }
  return {
    generationDigest: digest(`${snapshot.sessionRef}:${snapshot.generation}`),
    jobDigest: digest(workerJobRef),
    providerSessionDigest: `sha256:${providerDigest}`,
    holdDigest: digest(snapshot.reservationRef),
    settlementDigest: digest(settlementRef),
    terminalReason: snapshot.closeReason,
    durationMs: terminalAtMs - snapshot.startedAtMs,
    reservedMsat: snapshot.reservedMsat,
    chargedMsat: snapshot.chargedMsat,
    releasedMsat: snapshot.reservedMsat - snapshot.chargedMsat,
    usage: {
      inputTokens: snapshot.inputTokens,
      outputTokens: snapshot.outputTokens,
      cachedInputTokens: snapshot.cachedInputTokens,
      audioInputTokens: snapshot.audioInputTokens,
      audioOutputTokens: snapshot.audioOutputTokens,
      responseCount: snapshot.responseCount,
      transcriptionCount: snapshot.transcriptionCount,
      cancelledResponseCount: snapshot.cancelledResponseCount,
    },
    terminalEventCount: 1,
    workerJobCount: 1,
    providerSessionCount: 1,
    activityAfterTerminalCount: 0,
  };
};

export const buildSarahLiveKitRemainingDrillReceipt = (
  observation: SarahLiveKitRemainingDrillPrivateObservation,
): SarahLiveKitRemainingDrillReceipt => {
  if (observation.schema !== SARAH_LIVEKIT_REMAINING_DRILL_PRIVATE_SCHEMA) {
    throw new Error("remaining Sarah drill private schema is unsupported");
  }
  const previous = publicSnapshot(observation.previous);
  const fresh = observation.fresh === null ? null : publicSnapshot(observation.fresh);
  if (
    observation.scenario === "provider_disconnect" &&
    (!observation.previous.providerDisconnectApplied ||
      observation.previous.closeReason !== "provider_disconnect" ||
      observation.providerDisconnect === null ||
      observation.providerDisconnect.sharedInfrastructureMutated !== false)
  ) {
    throw new Error("provider_disconnect receipt has no exact applied fault");
  }
  if (observation.scenario === "reconnect") {
    if (
      fresh === null ||
      fresh.generationDigest === previous.generationDigest ||
      observation.providerDisconnect !== null ||
      (observation.fresh?.startedAtMs ?? 0) <=
        (observation.previous.terminalAtMs ?? Number.MAX_SAFE_INTEGER)
    ) {
      throw new Error("reconnect receipt has no strictly later fresh generation");
    }
  } else if (fresh !== null) {
    throw new Error(`${observation.scenario} carried reconnect-only evidence`);
  }
  const body = {
    scenario: observation.scenario,
    sourceRevision: observation.sourceRevision,
    workerImageDigest: observation.workerImageDigest,
    observedAt: observation.observedAt,
    turnsSent: observation.turnsSent,
    previous,
    fresh,
    providerDisconnectApplied:
      observation.scenario === "provider_disconnect" &&
      observation.previous.providerDisconnectApplied &&
      observation.providerDisconnect !== null,
  };
  const receipt: SarahLiveKitRemainingDrillReceipt = {
    schema: SARAH_LIVEKIT_REMAINING_DRILL_RECEIPT_SCHEMA,
    scenario: observation.scenario,
    issueRef: "github-issue-ref://OpenAgentsInc/openagents/9285",
    environment: "production",
    sourceRevision: observation.sourceRevision,
    workerImageDigest: observation.workerImageDigest,
    observedAt: observation.observedAt,
    outcome: "passed",
    turnsSent: observation.turnsSent,
    previous,
    fresh,
    providerDisconnectApplied: body.providerDisconnectApplied,
    noSharedInfrastructureMutation: true,
    noWorkerOverlap: true,
    noProviderOverlap: true,
    settledGenerationRevived: false,
    resultDigest: digest(JSON.stringify(body)),
  };
  assertPublicSafeSarahLiveKitRemainingDrillReceipt(receipt);
  return receipt;
};

const PRIVATE_KEY =
  /(?:sessionref|reservationref|settlementreceiptref|workerjobref|requestref|bearer|authorization|ticket|grant|pcm)/iu;
const PRIVATE_VALUE =
  /(?:https?:\/\/|wss?:\/\/|eyJ[A-Za-z0-9_-]{20,}|sk-[A-Za-z0-9_-]{20,}|oa_omega_[A-Za-z0-9_-]{20,})/u;

export const assertPublicSafeSarahLiveKitRemainingDrillReceipt = (value: unknown): void => {
  const walk = (node: unknown): void => {
    if (Array.isArray(node)) {
      node.forEach(walk);
      return;
    }
    if (typeof node === "object" && node !== null) {
      for (const [key, child] of Object.entries(node)) {
        if (PRIVATE_KEY.test(key)) throw new Error(`remaining Sarah drill receipt leaks ${key}`);
        walk(child);
      }
      return;
    }
    if (typeof node === "string" && PRIVATE_VALUE.test(node)) {
      throw new Error("remaining Sarah drill receipt contains private material");
    }
  };
  walk(value);
};

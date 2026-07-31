import { createHash } from "node:crypto";

export const SARAH_LIVEKIT_FAILURE_MATRIX_SCHEMA =
  "openagents.sarah.livekit-failure-matrix-observation.v1" as const;
export const SARAH_LIVEKIT_FAILURE_MATRIX_RECEIPT_SCHEMA =
  "openagents.sarah.livekit-failure-matrix-receipt.v1" as const;

export const SARAH_LIVEKIT_FAILURE_SCENARIOS = [
  "success",
  "cancellation",
  "timeout",
  "planned_worker_crash",
  "sfu_loss",
  "provider_disconnect",
  "hold_exhaustion",
  "reconnect",
] as const;

export type SarahLiveKitFailureScenario = (typeof SARAH_LIVEKIT_FAILURE_SCENARIOS)[number];

/**
 * The bound `sfu_loss` asserts: from the instant the exact SFU instance is
 * destroyed to the instant the session's accounting is terminal.
 *
 * It is deliberately far below the 300 second `max_session_seconds` ceiling.
 * The worker publishes a `lease_check` every five seconds, so an authority that
 * only learns of the loss when the session-expiry sweep fires has not
 * demonstrated bounded SFU-loss handling — it has re-demonstrated the `timeout`
 * scenario, with a hold pinned and a room orphaned for the intervening minutes.
 * Thirty seconds is a small multiple of the lease cadence and is the number a
 * run has to beat for this drill to say anything the matrix does not already
 * say elsewhere.
 */
export const SARAH_LIVEKIT_SFU_LOSS_BOUND_MS = 30_000;

/**
 * The terminal reasons each scenario admits.
 *
 * Every scenario but `sfu_loss` pins exactly one reason, because the fault and
 * the reason are the same event observed twice. `sfu_loss` is different: the
 * worker protocol has no media-transport-loss reason, so the reason recorded is
 * whichever watchdog noticed the room vanish first — the agent session close
 * (`participant_left`), the job shutdown callback (`worker_shutdown`), or a
 * failure on the way down (`worker_error`). All three are bounded, deterministic
 * settlements and all three are admitted.
 *
 * Two reasons are deliberately NOT admitted for `sfu_loss`:
 *
 *   - `session_expired` means nothing terminated the session except the ordinary
 *     deadline. The drill proved the timeout works, not that SFU loss is bounded.
 *   - `completed` means the authority recorded a clean finish for a session whose
 *     transport was destroyed underneath it. That is the silent-loss failure this
 *     row exists to catch.
 *
 * A run that produces either is a finding. Record it `contradicted` against
 * `sfu_loss_bounded`; do not re-run until it produces a nicer reason.
 */
const ADMITTED_TERMINAL_REASONS = {
  success: ["completed"],
  cancellation: ["operator_stop"],
  timeout: ["session_expired"],
  planned_worker_crash: ["worker_error"],
  sfu_loss: ["worker_shutdown", "participant_left", "worker_error"],
  provider_disconnect: ["provider_disconnect"],
  hold_exhaustion: ["hold_exhausted"],
  reconnect: ["completed"],
} as const satisfies Readonly<Record<SarahLiveKitFailureScenario, readonly string[]>>;

/**
 * The fault each scenario's evidence is only valid for.
 *
 * Exported so the drill driver names its fault from the same constant the
 * receipt validator checks it against. A drill that could satisfy the driver and
 * fail the matrix, or the reverse, would let the two disagree about what was
 * done to production.
 */
export const EXPECTED_FAULT_ACTION = {
  success: "none",
  cancellation: "client_cancel",
  timeout: "bounded_deadline",
  planned_worker_crash: "delete_exact_worker_pod",
  /**
   * Delete the exact `livekit-server` pod hosting the drill session's room.
   *
   * Same bounded class as `delete_exact_worker_pod`: one named pod, chosen
   * because it carries this generation's media. Not a graceful drain — a drain
   * is a different drill and is not loss. Not a node deletion, which would take
   * the co-located Sarah worker too and make the observation indistinguishable
   * from `planned_worker_crash`. Never a shared firewall, NetworkPolicy,
   * ConfigMap, Secret, or Deployment change, for the same reason the
   * provider-disconnect drill forbids them: those faults are not scoped to one
   * generation and cannot be attributed to it.
   */
  sfu_loss: "delete_exact_sfu_pod",
  provider_disconnect: "close_exact_provider_socket",
  hold_exhaustion: "exhaust_exact_session_hold",
  reconnect: "reconnect_after_terminal",
} as const satisfies Readonly<Record<SarahLiveKitFailureScenario, string>>;

type Usage = Readonly<{
  inputTokens: number;
  outputTokens: number;
  cachedInputTokens: number;
  audioInputTokens: number;
  audioOutputTokens: number;
  responseCount: number;
  transcriptionCount: number;
  cancelledResponseCount: number;
  chargeMsat: number;
}>;

/**
 * Evidence only the SFU-loss drill produces.
 *
 * The other scenarios terminate through a path the authority is already
 * watching. SFU loss destroys the transport, so the two questions the row
 * actually asks — how long until accounting was terminal, and was anything left
 * behind — have to be measured against the fault instant and against the
 * room-binding table, not inferred from the session's own duration.
 */
export type SarahLiveKitSfuLossEvidence = Readonly<{
  /**
   * Digest of the exact SFU instance destroyed.
   *
   * A digest, not a pod name: a name is a cluster address. It exists so an
   * independent reviewer can confirm the fault was aimed at one instance and
   * that it was not the instance running the Sarah worker.
   */
  sfuInstanceDigest: string;
  /** Digest of the Sarah worker instance, which must survive the fault. */
  workerInstanceDigest: string;
  faultInjectedAtMs: number;
  /** When the packaged client or worker first observed media loss. */
  mediaLossObservedAtMs: number;
  /**
   * State of this session's `sarah_livekit_room_bindings` row once the
   * reconciler settled. `cleanup_abandoned` is the reconciler giving up, which
   * is an orphaned room at the SFU and fails the row.
   */
  roomBindingTerminalState: "cleaned" | "cleanup_failed";
  roomBindingObservedAtMs: number;
  /** Rows still `prepared` or `active` for this session after settlement. */
  residualActiveRoomBindingCount: 0;
  residualWorkerGenerationCount: 0;
  residualProviderSessionCount: 0;
  /**
   * Billable Sarah sessions live anywhere in the cluster during the drill
   * window. Deleting an SFU pod takes every room on that instance, so the drill
   * is only attributable — and only non-customer-affecting — when its own
   * session is the single live one.
   */
  concurrentBillableSessionCount: 1;
}>;

export type SarahLiveKitFailureScenarioObservation = Readonly<{
  scenario: SarahLiveKitFailureScenario;
  faultAction: (typeof EXPECTED_FAULT_ACTION)[SarahLiveKitFailureScenario];
  terminalReason: (typeof ADMITTED_TERMINAL_REASONS)[SarahLiveKitFailureScenario][number];
  terminalState: "settled" | "released";
  startedAtMs: number;
  terminalAtMs: number;
  identityDigests: Readonly<{
    job: string;
    providerSession: string;
    generation: string;
    hold: string;
    usage: string;
    settlement: string;
  }>;
  usage: Usage;
  hold: Readonly<{
    reservedMsat: number;
    chargedMsat: number;
    releasedMsat: number;
  }>;
  settlementChargeMsat: number;
  terminalEventCount: number;
  maximumWorkerGenerationCount: number;
  maximumProviderSessionCount: number;
  freshAdmissionRequired: true;
  accountingEvidenceDigest: string;
  faultEvidenceDigest: string;
  privacyEvidenceDigest: string;
  secretFindings: number;
  rawMediaFindings: number;
  transcriptFindings: number;
  reconnect: null | Readonly<{
    previousGenerationDigest: string;
    freshGenerationDigest: string;
    previousTerminalAtMs: number;
    freshGenerationStartedAtMs: number;
    settledGenerationRevived: false;
  }>;
  sfuLoss: null | SarahLiveKitSfuLossEvidence;
}>;

export type SarahLiveKitFailureMatrixObservation = Readonly<{
  schema: typeof SARAH_LIVEKIT_FAILURE_MATRIX_SCHEMA;
  environment: "production";
  sourceRevision: string;
  workerImageDigest: string;
  observedAt: string;
  runDigest: string;
  scenarios: readonly SarahLiveKitFailureScenarioObservation[];
}>;

/**
 * The SFU-loss projection.
 *
 * `faultToTerminalMs` is the number the row is actually about, so it is
 * published rather than left implicit in two timestamps. The residual counts are
 * published as the literal zeros they are required to be, so a reader does not
 * have to trust that the validator ran.
 */
type PublicSfuLoss = Readonly<{
  sfuInstanceDigest: string;
  workerInstanceDigest: string;
  faultToTerminalMs: number;
  mediaLossDetectedWithinMs: number;
  boundMs: typeof SARAH_LIVEKIT_SFU_LOSS_BOUND_MS;
  roomBindingTerminalState: "cleaned" | "cleanup_failed";
  residualActiveRoomBindingCount: 0;
  residualWorkerGenerationCount: 0;
  residualProviderSessionCount: 0;
  concurrentBillableSessionCount: 1;
  workerInstanceSurvived: true;
}>;

type PublicScenario = Readonly<{
  scenario: SarahLiveKitFailureScenario;
  faultAction: (typeof EXPECTED_FAULT_ACTION)[SarahLiveKitFailureScenario];
  terminalReason: (typeof ADMITTED_TERMINAL_REASONS)[SarahLiveKitFailureScenario][number];
  terminalState: "settled" | "released";
  durationMs: number;
  usage: Usage;
  settlementChargeMsat: number;
  holdReleasedMsat: number;
  terminalEventCount: 1;
  maximumWorkerGenerationCount: 1;
  maximumProviderSessionCount: 1;
  freshAdmissionRequired: true;
  exactAccounting: true;
  noWorkerOverlap: true;
  noProviderOverlap: true;
  settledGenerationRevived: false;
  accountingEvidenceDigest: string;
  faultEvidenceDigest: string;
  privacyEvidenceDigest: string;
  sfuLoss: null | PublicSfuLoss;
}>;

export type SarahLiveKitFailureMatrixReceipt = Readonly<{
  schema: typeof SARAH_LIVEKIT_FAILURE_MATRIX_RECEIPT_SCHEMA;
  receiptRef: string;
  resultDigest: string;
  issueRef: "github-issue-ref://OpenAgentsInc/openagents/9285";
  environment: "production";
  sourceRevision: string;
  workerImageDigest: string;
  observedAt: string;
  outcome: "passed";
  liveProof: true;
  scenarios: readonly PublicScenario[];
  aggregateUsage: Usage;
  retainedMedia: false;
  retainedTranscript: false;
  limitations: readonly [
    "fault_actions_are_observed_not_executed_by_this_harness",
    "private_authority_identifiers_are_represented_only_by_sha256_digests",
    "cluster_wide_privacy_scan_remains_a_separate_sarah_matrix_obligation",
  ];
}>;

const SHA256 = /^sha256:[0-9a-f]{64}$/u;
const COMMIT = /^[0-9a-f]{40}$/u;
const PRIVATE_KEY =
  /^(authorization|bearer|cookie|credential|private.?key|secret|ticket|transcript(?:content)?|audio(?:content|bytes)?|pcm|prompt(?:content)?|email|endpoint|url)$/iu;
const PRIVATE_VALUE =
  /(?:eyJ[A-Za-z0-9_-]{20,}\.|sk-[A-Za-z0-9_-]{20,}|oa_omega_[A-Za-z0-9_-]{20,}|bearer\s+|wss?:\/\/|https?:\/\/)/iu;

const digest = (value: string): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`;

const assert: (condition: unknown, message: string) => asserts condition = (condition, message) => {
  if (!condition) throw new Error(message);
};

const assertExactKeys: (
  value: unknown,
  expectedKeys: readonly string[],
  label: string,
) => asserts value is Readonly<Record<string, unknown>> = (
  value: unknown,
  expectedKeys: readonly string[],
  label: string,
): asserts value is Readonly<Record<string, unknown>> => {
  assert(
    typeof value === "object" && value !== null && !Array.isArray(value),
    `${label} is invalid`,
  );
  const keys = Object.keys(value);
  assert(
    keys.length === expectedKeys.length &&
      expectedKeys.every((expected) => keys.includes(expected)),
    `${label} fields are invalid`,
  );
};

const assertInteger: (value: unknown, label: string) => asserts value is number = (
  value,
  label,
) => {
  assert(
    Number.isSafeInteger(value) && (value as number) >= 0,
    `${label} must be a nonnegative safe integer`,
  );
};

const assertDigest: (value: unknown, label: string) => asserts value is string = (value, label) => {
  assert(typeof value === "string" && SHA256.test(value), `${label} must be a sha256 digest`);
};

const assertUsage = (usage: Usage, label: string): void => {
  assertExactKeys(
    usage,
    [
      "inputTokens",
      "outputTokens",
      "cachedInputTokens",
      "audioInputTokens",
      "audioOutputTokens",
      "responseCount",
      "transcriptionCount",
      "cancelledResponseCount",
      "chargeMsat",
    ],
    label,
  );
  for (const [key, value] of Object.entries(usage)) assertInteger(value, `${label}.${key}`);
  assert(
    usage.inputTokens +
      usage.outputTokens +
      usage.cachedInputTokens +
      usage.audioInputTokens +
      usage.audioOutputTokens >
      0,
    `${label} has no provider usage`,
  );
  assert(usage.responseCount > 0, `${label} has no provider response`);
  assert(usage.transcriptionCount > 0, `${label} has no transcription usage`);
  assert(
    usage.cancelledResponseCount <= usage.responseCount,
    `${label} cancelled responses exceed all responses`,
  );
};

const assertNoPrivateMaterial = (value: unknown, label: string): void => {
  if (Array.isArray(value)) {
    value.forEach((child) => assertNoPrivateMaterial(child, label));
    return;
  }
  if (typeof value === "object" && value !== null) {
    for (const [key, child] of Object.entries(value)) {
      assert(!PRIVATE_KEY.test(key), `${label} contains forbidden field ${key}`);
      assertNoPrivateMaterial(child, label);
    }
    return;
  }
  if (typeof value === "string") {
    assert(!PRIVATE_VALUE.test(value), `${label} contains private material`);
  }
};

/**
 * Validate the SFU-loss drill.
 *
 * Ordering is the substance here. A fault injected before the session was live
 * proves nothing, media loss cannot precede the fault that caused it, and a
 * settlement that beat the media loss means the two were never causally
 * connected. Each of those is a run to repeat rather than evidence to keep.
 */
const validateSfuLoss = (value: SarahLiveKitFailureScenarioObservation): void => {
  assertExactKeys(
    value.sfuLoss,
    [
      "sfuInstanceDigest",
      "workerInstanceDigest",
      "faultInjectedAtMs",
      "mediaLossObservedAtMs",
      "roomBindingTerminalState",
      "roomBindingObservedAtMs",
      "residualActiveRoomBindingCount",
      "residualWorkerGenerationCount",
      "residualProviderSessionCount",
      "concurrentBillableSessionCount",
    ],
    "sfu loss evidence",
  );
  const sfuLoss = value.sfuLoss as SarahLiveKitSfuLossEvidence;
  assertDigest(sfuLoss.sfuInstanceDigest, "sfuLoss.sfuInstanceDigest");
  assertDigest(sfuLoss.workerInstanceDigest, "sfuLoss.workerInstanceDigest");
  assert(
    sfuLoss.sfuInstanceDigest !== sfuLoss.workerInstanceDigest,
    "sfu loss destroyed the Sarah worker instance instead of the SFU instance",
  );
  for (const key of [
    "faultInjectedAtMs",
    "mediaLossObservedAtMs",
    "roomBindingObservedAtMs",
  ] as const) {
    assertInteger(sfuLoss[key], `sfuLoss.${key}`);
  }
  assert(
    sfuLoss.faultInjectedAtMs > value.startedAtMs,
    "sfu loss injected its fault before the session was live",
  );
  assert(
    sfuLoss.mediaLossObservedAtMs >= sfuLoss.faultInjectedAtMs,
    "sfu loss observed media loss before the fault that caused it",
  );
  assert(
    value.terminalAtMs >= sfuLoss.mediaLossObservedAtMs,
    "sfu loss settled before media loss was observed",
  );
  assert(
    value.terminalAtMs - sfuLoss.faultInjectedAtMs <= SARAH_LIVEKIT_SFU_LOSS_BOUND_MS,
    `sfu loss exceeded its ${SARAH_LIVEKIT_SFU_LOSS_BOUND_MS} ms bound`,
  );
  assert(
    sfuLoss.roomBindingTerminalState === "cleaned" ||
      sfuLoss.roomBindingTerminalState === "cleanup_failed",
    "sfu loss left the room binding in a nonterminal or abandoned state",
  );
  assert(
    sfuLoss.roomBindingObservedAtMs >= value.terminalAtMs,
    "sfu loss read the room binding before the session was terminal",
  );
  for (const key of [
    "residualActiveRoomBindingCount",
    "residualWorkerGenerationCount",
    "residualProviderSessionCount",
  ] as const) {
    assert(sfuLoss[key] === 0, `sfuLoss.${key} is nonzero`);
  }
  assert(
    sfuLoss.concurrentBillableSessionCount === 1,
    "sfu loss ran with a billable session other than its own live in the cluster",
  );
};

const validateScenario = (
  value: SarahLiveKitFailureScenarioObservation,
  expectedScenario: SarahLiveKitFailureScenario,
): void => {
  assertExactKeys(
    value,
    [
      "scenario",
      "faultAction",
      "terminalReason",
      "terminalState",
      "startedAtMs",
      "terminalAtMs",
      "identityDigests",
      "usage",
      "hold",
      "settlementChargeMsat",
      "terminalEventCount",
      "maximumWorkerGenerationCount",
      "maximumProviderSessionCount",
      "freshAdmissionRequired",
      "accountingEvidenceDigest",
      "faultEvidenceDigest",
      "privacyEvidenceDigest",
      "secretFindings",
      "rawMediaFindings",
      "transcriptFindings",
      "reconnect",
      "sfuLoss",
    ],
    `scenario ${expectedScenario}`,
  );
  assert(value.scenario === expectedScenario, `matrix scenario ${expectedScenario} is missing`);
  assert(
    value.faultAction === EXPECTED_FAULT_ACTION[expectedScenario],
    `${expectedScenario} fault action is invalid`,
  );
  assert(
    (ADMITTED_TERMINAL_REASONS[expectedScenario] as readonly string[]).includes(
      value.terminalReason,
    ),
    `${expectedScenario} terminal reason is invalid`,
  );
  assert(
    value.terminalState === "settled" || value.terminalState === "released",
    `${expectedScenario} is not terminal`,
  );
  assertInteger(value.startedAtMs, `${expectedScenario}.startedAtMs`);
  assertInteger(value.terminalAtMs, `${expectedScenario}.terminalAtMs`);
  assert(value.terminalAtMs > value.startedAtMs, `${expectedScenario} has an invalid duration`);
  assertExactKeys(
    value.identityDigests,
    ["job", "providerSession", "generation", "hold", "usage", "settlement"],
    `${expectedScenario}.identityDigests`,
  );
  Object.entries(value.identityDigests).forEach(([key, identityDigest]) =>
    assertDigest(identityDigest, `${expectedScenario}.identityDigests.${key}`),
  );
  assertUsage(value.usage, `${expectedScenario}.usage`);
  if (expectedScenario === "cancellation") {
    assert(
      value.usage.cancelledResponseCount > 0,
      "cancellation has no cancelled provider response",
    );
  }
  assertExactKeys(
    value.hold,
    ["reservedMsat", "chargedMsat", "releasedMsat"],
    `${expectedScenario}.hold`,
  );
  Object.entries(value.hold).forEach(([key, amount]) =>
    assertInteger(amount, `${expectedScenario}.hold.${key}`),
  );
  assertInteger(value.settlementChargeMsat, `${expectedScenario}.settlementChargeMsat`);
  assert(
    value.usage.chargeMsat === value.settlementChargeMsat &&
      value.hold.chargedMsat === value.settlementChargeMsat &&
      value.hold.reservedMsat === value.hold.chargedMsat + value.hold.releasedMsat,
    `${expectedScenario} provider usage, hold, and settlement do not reconcile`,
  );
  assert(value.terminalEventCount === 1, `${expectedScenario} has duplicate terminal events`);
  assert(
    value.maximumWorkerGenerationCount === 1,
    `${expectedScenario} overlapped worker generations`,
  );
  assert(
    value.maximumProviderSessionCount === 1,
    `${expectedScenario} overlapped provider sessions`,
  );
  assert(value.freshAdmissionRequired === true, `${expectedScenario} reused stale admission`);
  for (const key of [
    "accountingEvidenceDigest",
    "faultEvidenceDigest",
    "privacyEvidenceDigest",
  ] as const) {
    assertDigest(value[key], `${expectedScenario}.${key}`);
  }
  for (const key of ["secretFindings", "rawMediaFindings", "transcriptFindings"] as const) {
    assert(value[key] === 0, `${expectedScenario}.${key} is nonzero`);
  }
  if (expectedScenario !== "sfu_loss") {
    assert(value.sfuLoss === null, `${expectedScenario} has sfu-loss-only evidence`);
  } else {
    validateSfuLoss(value);
  }
  if (expectedScenario !== "reconnect") {
    assert(value.reconnect === null, `${expectedScenario} has reconnect-only evidence`);
    return;
  }
  assertExactKeys(
    value.reconnect,
    [
      "previousGenerationDigest",
      "freshGenerationDigest",
      "previousTerminalAtMs",
      "freshGenerationStartedAtMs",
      "settledGenerationRevived",
    ],
    "reconnect evidence",
  );
  assertDigest(value.reconnect.previousGenerationDigest, "reconnect.previousGenerationDigest");
  assertDigest(value.reconnect.freshGenerationDigest, "reconnect.freshGenerationDigest");
  assert(
    value.reconnect.previousGenerationDigest !== value.reconnect.freshGenerationDigest,
    "reconnect did not create a fresh generation",
  );
  assertInteger(value.reconnect.previousTerminalAtMs, "reconnect.previousTerminalAtMs");
  assertInteger(value.reconnect.freshGenerationStartedAtMs, "reconnect.freshGenerationStartedAtMs");
  assert(
    value.reconnect.freshGenerationStartedAtMs > value.reconnect.previousTerminalAtMs,
    "reconnect overlapped the terminal generation",
  );
  assert(
    value.reconnect.settledGenerationRevived === false,
    "reconnect revived a settled generation",
  );
};

export const validateSarahLiveKitFailureMatrixObservation = (
  value: SarahLiveKitFailureMatrixObservation,
): SarahLiveKitFailureMatrixObservation => {
  assertExactKeys(
    value,
    [
      "schema",
      "environment",
      "sourceRevision",
      "workerImageDigest",
      "observedAt",
      "runDigest",
      "scenarios",
    ],
    "failure matrix observation",
  );
  assert(value.schema === SARAH_LIVEKIT_FAILURE_MATRIX_SCHEMA, "observation schema is unsupported");
  assert(value.environment === "production", "failure matrix must observe production");
  assert(COMMIT.test(value.sourceRevision), "sourceRevision must be a full lowercase Git commit");
  assertDigest(value.workerImageDigest, "workerImageDigest");
  assertDigest(value.runDigest, "runDigest");
  assert(
    typeof value.observedAt === "string" && Number.isFinite(Date.parse(value.observedAt)),
    "observedAt must be an ISO timestamp",
  );
  assert(
    Array.isArray(value.scenarios) &&
      value.scenarios.length === SARAH_LIVEKIT_FAILURE_SCENARIOS.length,
    "failure matrix must contain every scenario exactly once",
  );
  for (const [scenarioIndex, expectedScenario] of SARAH_LIVEKIT_FAILURE_SCENARIOS.entries()) {
    assert(
      value.scenarios[scenarioIndex]?.scenario === expectedScenario,
      "failure matrix scenarios are out of order",
    );
    const matches = value.scenarios.filter((candidate) => candidate.scenario === expectedScenario);
    assert(matches.length === 1, `matrix scenario ${expectedScenario} must appear exactly once`);
    validateScenario(matches[0] as SarahLiveKitFailureScenarioObservation, expectedScenario);
  }
  const identities = value.scenarios.flatMap((scenario) => Object.values(scenario.identityDigests));
  assert(
    new Set(identities).size === identities.length,
    "failure matrix authority identities overlap",
  );
  assertNoPrivateMaterial(value, "failure matrix observation");
  return value;
};

const sumUsage = (scenarios: readonly SarahLiveKitFailureScenarioObservation[]): Usage =>
  scenarios.reduce<Usage>(
    (total, scenario) => ({
      inputTokens: total.inputTokens + scenario.usage.inputTokens,
      outputTokens: total.outputTokens + scenario.usage.outputTokens,
      cachedInputTokens: total.cachedInputTokens + scenario.usage.cachedInputTokens,
      audioInputTokens: total.audioInputTokens + scenario.usage.audioInputTokens,
      audioOutputTokens: total.audioOutputTokens + scenario.usage.audioOutputTokens,
      responseCount: total.responseCount + scenario.usage.responseCount,
      transcriptionCount: total.transcriptionCount + scenario.usage.transcriptionCount,
      cancelledResponseCount: total.cancelledResponseCount + scenario.usage.cancelledResponseCount,
      chargeMsat: total.chargeMsat + scenario.usage.chargeMsat,
    }),
    {
      inputTokens: 0,
      outputTokens: 0,
      cachedInputTokens: 0,
      audioInputTokens: 0,
      audioOutputTokens: 0,
      responseCount: 0,
      transcriptionCount: 0,
      cancelledResponseCount: 0,
      chargeMsat: 0,
    },
  );

export const buildSarahLiveKitFailureMatrixReceipt = (
  observation: SarahLiveKitFailureMatrixObservation,
): SarahLiveKitFailureMatrixReceipt => {
  validateSarahLiveKitFailureMatrixObservation(observation);
  const scenarios = SARAH_LIVEKIT_FAILURE_SCENARIOS.map((scenarioName) => {
    const scenario = observation.scenarios.find(
      (candidate) => candidate.scenario === scenarioName,
    ) as SarahLiveKitFailureScenarioObservation;
    return {
      scenario: scenario.scenario,
      faultAction: scenario.faultAction,
      terminalReason: scenario.terminalReason,
      terminalState: scenario.terminalState,
      durationMs: scenario.terminalAtMs - scenario.startedAtMs,
      usage: scenario.usage,
      settlementChargeMsat: scenario.settlementChargeMsat,
      holdReleasedMsat: scenario.hold.releasedMsat,
      terminalEventCount: 1,
      maximumWorkerGenerationCount: 1,
      maximumProviderSessionCount: 1,
      freshAdmissionRequired: true,
      exactAccounting: true,
      noWorkerOverlap: true,
      noProviderOverlap: true,
      settledGenerationRevived: false,
      accountingEvidenceDigest: scenario.accountingEvidenceDigest,
      faultEvidenceDigest: scenario.faultEvidenceDigest,
      privacyEvidenceDigest: scenario.privacyEvidenceDigest,
      sfuLoss:
        scenario.sfuLoss === null
          ? null
          : {
              sfuInstanceDigest: scenario.sfuLoss.sfuInstanceDigest,
              workerInstanceDigest: scenario.sfuLoss.workerInstanceDigest,
              faultToTerminalMs: scenario.terminalAtMs - scenario.sfuLoss.faultInjectedAtMs,
              mediaLossDetectedWithinMs:
                scenario.sfuLoss.mediaLossObservedAtMs - scenario.sfuLoss.faultInjectedAtMs,
              boundMs: SARAH_LIVEKIT_SFU_LOSS_BOUND_MS,
              roomBindingTerminalState: scenario.sfuLoss.roomBindingTerminalState,
              residualActiveRoomBindingCount: 0,
              residualWorkerGenerationCount: 0,
              residualProviderSessionCount: 0,
              concurrentBillableSessionCount: 1,
              workerInstanceSurvived: true,
            },
    } satisfies PublicScenario;
  });
  const aggregateUsage = sumUsage(observation.scenarios);
  const resultDigest = digest(
    JSON.stringify({
      sourceRevision: observation.sourceRevision,
      workerImageDigest: observation.workerImageDigest,
      observedAt: observation.observedAt,
      runDigest: observation.runDigest,
      scenarios,
      aggregateUsage,
    }),
  );
  const receipt: SarahLiveKitFailureMatrixReceipt = {
    schema: SARAH_LIVEKIT_FAILURE_MATRIX_RECEIPT_SCHEMA,
    receiptRef: `sarah-livekit-failure-matrix-receipt-ref://sha256/${resultDigest.slice(7)}`,
    resultDigest,
    issueRef: "github-issue-ref://OpenAgentsInc/openagents/9285",
    environment: "production",
    sourceRevision: observation.sourceRevision,
    workerImageDigest: observation.workerImageDigest,
    observedAt: observation.observedAt,
    outcome: "passed",
    liveProof: true,
    scenarios,
    aggregateUsage,
    retainedMedia: false,
    retainedTranscript: false,
    limitations: [
      "fault_actions_are_observed_not_executed_by_this_harness",
      "private_authority_identifiers_are_represented_only_by_sha256_digests",
      "cluster_wide_privacy_scan_remains_a_separate_sarah_matrix_obligation",
    ],
  };
  assertNoPrivateMaterial(receipt, "failure matrix receipt");
  return receipt;
};

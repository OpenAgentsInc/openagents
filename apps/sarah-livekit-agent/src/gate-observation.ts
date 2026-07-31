import { createHash } from "node:crypto";
import { SARAH_LIVEKIT_SFU_LOSS_BOUND_MS } from "./failure-matrix.js";
import type {
  ForcedTransportProfile,
  SelectedIcePathClassification,
} from "./ice-classification.js";

/**
 * Recorder for the Sarah LiveKit release-gate observations that no automated
 * harness run can produce.
 *
 * The two-scenario acceptance receipt records one headless run of the private
 * and community journeys. Several release-gate rows require things that only a
 * human operator, driving packaged clients and imposing real faults, can cause:
 * three authenticated desktops passing a server-issued floor, a moderator stop,
 * four named membership refusals, forced UDP/TCP/TURN transport, live capability
 * refusals inside a community room, and eight bounded failure drills.
 *
 * Before this module those results had nowhere to go. An operator could perform
 * every journey perfectly and the evidence would evaporate, because the receipt
 * schema had no field for floor, moderator, membership, refusal, transport
 * classification, admission terms, or drill outcome.
 *
 * This module is a RECORDER, not a judge. It never promotes anything: emitting a
 * row with every observation satisfied means "this operator preserved these
 * observations", not "this release-gate row is green". Row admission stays with
 * the Omega evidence manifest, which weighs these receipts against the candidate
 * binding.
 *
 * Three properties are load-bearing:
 *
 *   1. Fail closed. Every observation a row requires must be explicitly
 *      accounted for. An observation that was not made must be recorded as
 *      `not_observed` with a reason; silence is rejected. Nothing defaults to
 *      true, and nothing is a hard-coded literal.
 *   2. Truthful negatives. A refusal that did not happen, a bound that was
 *      exceeded, or an isolation that did not hold is recordable as
 *      `contradicted`. The recorder must be able to preserve a failed journey,
 *      or operators learn to discard inconvenient runs.
 *   3. Public safety. Participant identities, room refs, addresses, and grants
 *      never enter a receipt. Identities appear only as digests, and only where
 *      a comparison genuinely needs them.
 */

export const SARAH_LIVEKIT_GATE_OBSERVATION_SCHEMA =
  "openagents.sarah.livekit-gate-observation.v1" as const;

/** Release-gate rows this recorder can carry observations for. */
export const GATE_ROW_IDS = [
  "sarah-livekit-private",
  "sarah-livekit-room",
  "sarah-livekit-connectivity",
  "sarah-livekit-isolation",
  "sarah-livekit-failure",
] as const;
export type GateRowId = (typeof GATE_ROW_IDS)[number];

/**
 * The observation keys each row requires.
 *
 * This vocabulary is taken verbatim from the `not_observed` lists the rc29
 * evidence manifest recorded, so a receipt produced here answers exactly the
 * gaps that manifest named. Changing a key is a gate-contract change: the Omega
 * manifest resolves rows against these names.
 */
export const REQUIRED_OBSERVATIONS: Readonly<Record<GateRowId, readonly string[]>> = {
  "sarah-livekit-private": [
    "packaged_desktop_client_participation",
    "admission_terms_seen",
    "bounded_command_confirmed",
    "agent_thread_started",
    "reconnect_same_generation",
  ],
  "sarah-livekit-room": [
    "authenticated_desktop_count",
    "floor_acquired",
    "floor_transfer_completed",
    "shared_answer_heard_by_all",
    "moderator_stop_completed",
    "non_floor_refused",
    "removed_member_refused",
    "stale_floor_grant_refused",
    "replayed_floor_grant_refused",
  ],
  "sarah-livekit-connectivity": [
    "direct_udp_completed",
    "tcp_fallback_completed",
    "turn_tls_completed",
  ],
  "sarah-livekit-isolation": [
    "distinct_provider_generations",
    "context_isolated",
    "capability_isolated",
    "private_fact_refused",
    "privileged_tool_refused",
  ],
  "sarah-livekit-failure": [
    "worker_crash_bounded",
    "sfu_loss_bounded",
    "openai_disconnect_bounded",
    "replayed_grant_refused",
    "duplicate_participant_refused",
    "membership_revocation_bounded",
    "hold_exhaustion_bounded",
    "reconnect_no_overlap",
    "privacy_scope_count",
    "media_key_rekey_proof",
  ],
} as const;

/**
 * What the operator found.
 *
 * `contradicted` is not a synonym for `not_observed`. `contradicted` means the
 * journey ran and produced the opposite of what the row requires, which is a
 * finding. `not_observed` means the journey did not reach that step.
 */
export type GateFinding = "satisfied" | "contradicted" | "not_observed";

/** How a participant joined, for rows that require packaged-client participation. */
export const CLIENT_KINDS = [
  "packaged_omega_desktop",
  "packaged_openagents_mobile",
  "headless_harness",
  "browser",
  "other",
] as const;
export type ClientKind = (typeof CLIENT_KINDS)[number];

export type GateParticipant = Readonly<{
  /** Digest of the participant ref. The ref itself is never recorded. */
  participantRefDigest: string;
  clientKind: ClientKind;
  /** The candidate build this participant was running, when it is a packaged client. */
  packageSha256?: string;
  authenticated: boolean;
  audibleSarahOutputObserved: boolean;
}>;

/**
 * Evidence attached to an observation.
 *
 * Each variant carries the exact values the corresponding gate row asks for, and
 * nothing else. Every identity is a digest.
 */
export type GateEvidence =
  /** Who was in the room, on what client, and who actually heard Sarah. */
  | Readonly<{
      kind: "participants";
      participants: readonly GateParticipant[];
    }>
  /**
   * A server-issued floor lease movement.
   *
   * `fromHolderDigest` and `toHolderDigest` are the floor lease's own
   * `holderUserRefDigest`, so a transfer between two distinct members is
   * verifiable from the receipt without any identity being disclosed.
   */
  | Readonly<{
      kind: "floor";
      state: "available" | "held" | "stopped";
      issuance: number;
      fromHolderDigest?: string;
      toHolderDigest?: string;
      stopReason?:
        | "moderator_stop"
        | "timeout"
        | "member_removed"
        | "sarah_removed"
        | "membership_changed"
        | "presence_expired";
    }>
  /**
   * An action the server refused.
   *
   * `refusalCode` is the server's own typed refusal, so the receipt records what
   * the authority actually said rather than the operator's paraphrase.
   */
  | Readonly<{
      kind: "refusal";
      attempted: string;
      refusalCode: string;
      httpStatus: number;
    }>
  /** A classified transport capture for one forced-transport matrix cell. */
  | Readonly<{
      kind: "ice";
      forcedTransportProfile: ForcedTransportProfile;
      observedTransportKind: string;
      publisher: SelectedIcePathClassification;
      subscriber: SelectedIcePathClassification;
      /** Digest of the acceptance receipt this capture came from. */
      acceptanceResultDigest: string;
    }>
  /**
   * Admission terms displayed before capture.
   *
   * The ordering is the point: `displayedAtMs` must precede `firstCaptureAtMs`,
   * or the terms were not shown before the microphone opened.
   */
  | Readonly<{
      kind: "admission_terms";
      priceMsat: number;
      holdMsat: number;
      rateMsatPerMillionTokens: number;
      limitSeconds: number;
      displayedAtMs: number;
      firstCaptureAtMs: number;
    }>
  /**
   * A comparison between two provider generations.
   *
   * The digests are salted per receipt, so they are equal iff the underlying
   * generations are equal and are otherwise meaningless outside this receipt.
   */
  | Readonly<{
      kind: "generation_comparison";
      leftComparableDigest: string;
      rightComparableDigest: string;
      distinct: boolean;
    }>
  /** One executed failure drill. */
  | Readonly<{
      kind: "drill";
      faultInjected: string;
      boundedWithinMs: number;
      settlementState: "settled" | "released";
      settlementReceiptDigest: string;
    }>
  /** A retention scan across named scopes. */
  | Readonly<{
      kind: "scan";
      scopes: readonly string[];
      residueFound: boolean;
      sameWindowComplete: boolean;
    }>
  /**
   * A named fact with its source.
   *
   * Used for observations with no richer structure, such as a started agent
   * thread. `sourceKind` keeps a source-code or configuration claim from being
   * mistaken for a live observation, which is the exact promotion the manifest
   * refused for the capability profile.
   */
  | Readonly<{
      kind: "fact";
      fact: string;
      sourceKind: "live_client_observation" | "server_authority_response" | "operator_attestation";
      refDigest?: string;
    }>;

export type GateObservation = Readonly<{
  key: string;
  finding: GateFinding;
  /** Required when the finding is `not_observed`; forbidden otherwise. */
  reason?: string;
  /** Required when the finding is `satisfied` or `contradicted`. */
  evidence?: GateEvidence;
  observedAtMs: number;
}>;

/**
 * Binding to the exact candidate and infrastructure the journey ran against.
 *
 * An observation with no binding is not evidence for any candidate, so every
 * field here is required.
 */
export type GateBinding = Readonly<{
  omegaReleaseTag: string;
  omegaPackageSha256: string;
  openagentsSourceRevision: string;
  livekitConfigRevision: string;
  sarahWorkerImageDigest: string;
}>;

/**
 * Whether the row's observations were all preserved, and what they said.
 *
 * `observed_pass` means every required observation was made and satisfied. It is
 * deliberately not called "passed": promoting a row is the evidence manifest's
 * decision, made against the binding, not this receipt's.
 */
export type GateRowOutcome = "observed_pass" | "observed_fail" | "incomplete";

export type SarahLiveKitGateObservationReceipt = Readonly<{
  schema: typeof SARAH_LIVEKIT_GATE_OBSERVATION_SCHEMA;
  receiptRef: string;
  resultDigest: `sha256:${string}`;
  observedAt: string;
  environment: "production";
  row: GateRowId;
  outcome: GateRowOutcome;
  binding: GateBinding;
  /** Digest of the operator identity. Distinguishes a producer from a reviewer. */
  operatorRefDigest: string;
  observations: readonly GateObservation[];
  note: string;
}>;

const sha256 = (value: string): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`;

const DIGEST = /^[0-9a-f]{64}$/u;
const PREFIXED_DIGEST = /^sha256:[0-9a-f]{64}$/u;
const REVISION = /^[0-9a-f]{40}$/u;
const SAFE_TEXT = /^[\w .,:;()/><=+-]{1,512}$/u;

const assertDigest = (value: string | undefined, label: string): void => {
  if (value === undefined || !DIGEST.test(value)) {
    throw new Error(`${label} must be a bare lowercase sha256 digest`);
  }
};

const assertSafeText = (value: string, label: string): void => {
  if (!SAFE_TEXT.test(value)) {
    throw new Error(`${label} must be short public-safe text`);
  }
};

const assertBinding = (binding: GateBinding): void => {
  if (!/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/u.test(binding.omegaReleaseTag)) {
    throw new Error("binding omegaReleaseTag must be a semantic release tag");
  }
  assertDigest(binding.omegaPackageSha256, "binding omegaPackageSha256");
  if (!REVISION.test(binding.openagentsSourceRevision)) {
    throw new Error("binding openagentsSourceRevision must be a full lowercase Git commit");
  }
  for (const [label, value] of [
    ["livekitConfigRevision", binding.livekitConfigRevision],
    ["sarahWorkerImageDigest", binding.sarahWorkerImageDigest],
  ] as const) {
    if (!PREFIXED_DIGEST.test(value)) {
      throw new Error(`binding ${label} must be a sha256-prefixed digest`);
    }
  }
};

/**
 * Validate one observation's evidence against its own variant.
 *
 * This is where a structurally well-formed but semantically empty observation is
 * caught, for example a "floor transfer" whose two holders are the same member,
 * or admission terms displayed after capture began.
 */
const assertEvidence = (key: string, evidence: GateEvidence): void => {
  switch (evidence.kind) {
    case "participants": {
      if (evidence.participants.length === 0) {
        throw new Error(`${key} participant evidence is empty`);
      }
      const digests = evidence.participants.map((participant) => participant.participantRefDigest);
      if (new Set(digests).size !== digests.length) {
        throw new Error(`${key} participant evidence repeats a participant`);
      }
      for (const participant of evidence.participants) {
        assertDigest(participant.participantRefDigest, `${key} participantRefDigest`);
        if (!CLIENT_KINDS.includes(participant.clientKind)) {
          throw new Error(`${key} participant clientKind is unrecognized`);
        }
        if (participant.packageSha256 !== undefined) {
          assertDigest(participant.packageSha256, `${key} participant packageSha256`);
        }
      }
      return;
    }
    case "floor": {
      if (!Number.isSafeInteger(evidence.issuance) || evidence.issuance < 0) {
        throw new Error(`${key} floor issuance must be a non-negative integer`);
      }
      for (const [label, value] of [
        ["fromHolderDigest", evidence.fromHolderDigest],
        ["toHolderDigest", evidence.toHolderDigest],
      ] as const) {
        if (value !== undefined) assertDigest(value, `${key} floor ${label}`);
      }
      if (
        evidence.fromHolderDigest !== undefined &&
        evidence.fromHolderDigest === evidence.toHolderDigest
      ) {
        throw new Error(`${key} floor movement does not cross two distinct members`);
      }
      if (evidence.state === "stopped" && evidence.stopReason === undefined) {
        throw new Error(`${key} stopped floor must name its stop reason`);
      }
      return;
    }
    case "refusal": {
      assertSafeText(evidence.attempted, `${key} refusal attempted`);
      if (!/^[a-z0-9_]{1,64}$/u.test(evidence.refusalCode)) {
        throw new Error(`${key} refusalCode must be a server refusal identifier`);
      }
      if (!Number.isSafeInteger(evidence.httpStatus) || evidence.httpStatus < 400) {
        throw new Error(`${key} refusal must carry a client or server error status`);
      }
      return;
    }
    case "ice": {
      assertSafeText(evidence.observedTransportKind, `${key} observedTransportKind`);
      if (!PREFIXED_DIGEST.test(evidence.acceptanceResultDigest)) {
        throw new Error(`${key} ice evidence must reference an acceptance result digest`);
      }
      for (const [label, path] of [
        ["publisher", evidence.publisher],
        ["subscriber", evidence.subscriber],
      ] as const) {
        if (path.packetsObserved !== true) {
          throw new Error(`${key} ${label} ICE path carried no packets`);
        }
      }
      return;
    }
    case "admission_terms": {
      const amounts = [
        evidence.priceMsat,
        evidence.holdMsat,
        evidence.rateMsatPerMillionTokens,
        evidence.limitSeconds,
      ];
      if (amounts.some((amount) => !Number.isSafeInteger(amount) || amount <= 0)) {
        throw new Error(`${key} admission terms must all be positive`);
      }
      if (
        !Number.isSafeInteger(evidence.displayedAtMs) ||
        !Number.isSafeInteger(evidence.firstCaptureAtMs) ||
        evidence.displayedAtMs >= evidence.firstCaptureAtMs
      ) {
        throw new Error(`${key} admission terms were not displayed before capture began`);
      }
      return;
    }
    case "generation_comparison": {
      assertDigest(evidence.leftComparableDigest, `${key} leftComparableDigest`);
      assertDigest(evidence.rightComparableDigest, `${key} rightComparableDigest`);
      if (
        evidence.distinct !==
        (evidence.leftComparableDigest !== evidence.rightComparableDigest)
      ) {
        throw new Error(`${key} generation comparison disagrees with its own digests`);
      }
      return;
    }
    case "drill": {
      assertSafeText(evidence.faultInjected, `${key} faultInjected`);
      if (!Number.isSafeInteger(evidence.boundedWithinMs) || evidence.boundedWithinMs <= 0) {
        throw new Error(`${key} drill must record a positive bound`);
      }
      if (!PREFIXED_DIGEST.test(evidence.settlementReceiptDigest)) {
        throw new Error(`${key} drill must reference a settlement receipt digest`);
      }
      return;
    }
    case "scan": {
      if (evidence.scopes.length === 0 || new Set(evidence.scopes).size !== evidence.scopes.length) {
        throw new Error(`${key} scan scopes must be a non-empty set of distinct scopes`);
      }
      for (const scope of evidence.scopes) assertSafeText(scope, `${key} scan scope`);
      return;
    }
    case "fact": {
      assertSafeText(evidence.fact, `${key} fact`);
      if (evidence.refDigest !== undefined) assertDigest(evidence.refDigest, `${key} refDigest`);
      return;
    }
  }
};

/**
 * Semantic checks that a satisfied observation genuinely satisfies its key.
 *
 * Without this an operator could attach any well-formed evidence to any key. The
 * checks here are the ones where the key's meaning is stronger than its evidence
 * variant: counts, orderings, and whether the right thing was refused.
 */
const assertSatisfiesKey = (observation: GateObservation, evidence: GateEvidence): void => {
  const { key } = observation;
  if (key === "authenticated_desktop_count") {
    if (evidence.kind !== "participants") throw new Error(`${key} requires participant evidence`);
    const desktops = evidence.participants.filter(
      (participant) => participant.authenticated && participant.clientKind === "packaged_omega_desktop",
    );
    if (desktops.length < 3) {
      throw new Error(`${key} requires at least three authenticated packaged desktops`);
    }
    return;
  }
  if (key === "shared_answer_heard_by_all") {
    if (evidence.kind !== "participants") throw new Error(`${key} requires participant evidence`);
    if (!evidence.participants.every((participant) => participant.audibleSarahOutputObserved)) {
      throw new Error(`${key} requires every participant to have heard the shared answer`);
    }
    return;
  }
  if (key === "floor_transfer_completed") {
    if (evidence.kind !== "floor") throw new Error(`${key} requires floor evidence`);
    if (evidence.fromHolderDigest === undefined || evidence.toHolderDigest === undefined) {
      throw new Error(`${key} requires both the losing and gaining floor holders`);
    }
    return;
  }
  if (key === "moderator_stop_completed") {
    if (evidence.kind !== "floor") throw new Error(`${key} requires floor evidence`);
    if (evidence.state !== "stopped" || evidence.stopReason !== "moderator_stop") {
      throw new Error(`${key} requires a floor stopped by a moderator`);
    }
    return;
  }
  if (key.endsWith("_refused")) {
    if (evidence.kind !== "refusal") throw new Error(`${key} requires refusal evidence`);
    return;
  }
  if (key === "sfu_loss_bounded") {
    // The recorder and the failure matrix must agree, or a drill could satisfy
    // one and fail the other. Both read the same constant and the same fault
    // name, so the scenario has exactly one definition.
    if (evidence.kind !== "drill") throw new Error(`${key} requires drill evidence`);
    if (evidence.faultInjected !== "delete_exact_sfu_pod") {
      throw new Error(
        `${key} requires the delete_exact_sfu_pod fault, not ${evidence.faultInjected}`,
      );
    }
    if (evidence.boundedWithinMs > SARAH_LIVEKIT_SFU_LOSS_BOUND_MS) {
      throw new Error(
        `${key} exceeded its ${SARAH_LIVEKIT_SFU_LOSS_BOUND_MS} ms bound, measured from fault injection`,
      );
    }
    return;
  }
  if (key === "privacy_scope_count") {
    if (evidence.kind !== "scan") throw new Error(`${key} requires scan evidence`);
    if (evidence.scopes.length < 8 || evidence.residueFound || !evidence.sameWindowComplete) {
      throw new Error(`${key} requires a complete same-window scan of at least eight clean scopes`);
    }
    return;
  }
  if (key === "distinct_provider_generations") {
    if (evidence.kind !== "generation_comparison") {
      throw new Error(`${key} requires generation comparison evidence`);
    }
    if (!evidence.distinct) {
      throw new Error(`${key} requires the two generations to differ`);
    }
    return;
  }
  if (key === "reconnect_same_generation") {
    if (evidence.kind !== "generation_comparison") {
      throw new Error(`${key} requires generation comparison evidence`);
    }
    if (evidence.distinct) {
      throw new Error(`${key} requires the generation to be unchanged across the reconnect`);
    }
    return;
  }
  if (
    key === "direct_udp_completed" ||
    key === "tcp_fallback_completed" ||
    key === "turn_tls_completed"
  ) {
    if (evidence.kind !== "ice") throw new Error(`${key} requires classified ICE evidence`);
    const expected =
      key === "direct_udp_completed"
        ? "direct_udp"
        : key === "tcp_fallback_completed"
          ? "tcp_fallback"
          : "turn_tls";
    if (evidence.observedTransportKind !== expected) {
      throw new Error(`${key} was given a capture classified as ${evidence.observedTransportKind}`);
    }
    return;
  }
};

const assertObservations = (row: GateRowId, observations: readonly GateObservation[]): void => {
  const seen = new Set<string>();
  for (const observation of observations) {
    if (seen.has(observation.key)) {
      throw new Error(`observation ${observation.key} is recorded more than once`);
    }
    seen.add(observation.key);
    if (!Number.isSafeInteger(observation.observedAtMs) || observation.observedAtMs <= 0) {
      throw new Error(`observation ${observation.key} has no valid observation time`);
    }
    if (observation.finding === "not_observed") {
      if (observation.evidence !== undefined) {
        throw new Error(`observation ${observation.key} cannot be unobserved and carry evidence`);
      }
      if (observation.reason === undefined) {
        throw new Error(`observation ${observation.key} must say why it was not observed`);
      }
      assertSafeText(observation.reason, `observation ${observation.key} reason`);
      continue;
    }
    if (observation.reason !== undefined) {
      throw new Error(`observation ${observation.key} carries a reason but was observed`);
    }
    if (observation.evidence === undefined) {
      throw new Error(`observation ${observation.key} claims a finding with no evidence`);
    }
    assertEvidence(observation.key, observation.evidence);
    if (observation.finding === "satisfied") {
      assertSatisfiesKey(observation, observation.evidence);
    }
  }

  // Fail closed: an observation the row requires may not simply be absent.
  const required = REQUIRED_OBSERVATIONS[row];
  const missing = required.filter((key) => !seen.has(key));
  if (missing.length > 0) {
    throw new Error(
      `${row} is missing required observations: ${missing.join(", ")}. ` +
        "Record each one explicitly, using finding \"not_observed\" with a reason if the journey did not reach it.",
    );
  }
  const unknown = [...seen].filter((key) => !required.includes(key));
  if (unknown.length > 0) {
    throw new Error(`${row} does not define the observations: ${unknown.join(", ")}`);
  }
};

const rowOutcome = (observations: readonly GateObservation[]): GateRowOutcome => {
  if (observations.some((observation) => observation.finding === "contradicted")) {
    return "observed_fail";
  }
  if (observations.some((observation) => observation.finding === "not_observed")) {
    return "incomplete";
  }
  return "observed_pass";
};

/**
 * Reject a receipt that carries private material.
 *
 * The forbidden keys are the identity, grant, and network-locating fields a
 * careless extension would reach for. The patterns catch bearer tokens and API
 * keys pasted into a free-text reason.
 */
export const assertPublicSafeGateObservationReceipt = (
  receipt: SarahLiveKitGateObservationReceipt,
): void => {
  const serialized = JSON.stringify(receipt);
  const forbiddenKeys = new Set([
    "authorization",
    "bearer",
    "participantgrant",
    "grant",
    "ticket",
    "gatewayurl",
    "transcriptcontent",
    "pcm",
    "audiocontent",
    "roomref",
    "ownerref",
    "deviceref",
    "communityref",
    "channelref",
    "participantref",
    "operatorref",
    "address",
    "port",
    "url",
    "relatedaddress",
    "usernamefragment",
    "salt",
    "receiptsalt",
  ]);
  const hasForbiddenKey = (value: unknown): boolean => {
    if (Array.isArray(value)) return value.some(hasForbiddenKey);
    if (typeof value !== "object" || value === null) return false;
    return Object.entries(value).some(
      ([key, child]) => forbiddenKeys.has(key.toLowerCase()) || hasForbiddenKey(child),
    );
  };
  if (
    hasForbiddenKey(receipt) ||
    /(?:eyJ[A-Za-z0-9_-]{20,}\.|sk-[A-Za-z0-9_-]{20,}|oa_omega_[A-Za-z0-9_-]{20,})/u.test(
      serialized,
    ) ||
    /\b(?:\d{1,3}\.){3}\d{1,3}\b/u.test(serialized)
  ) {
    throw new Error("Sarah LiveKit gate observation receipt contains private material");
  }
};

/**
 * Record one release-gate row's observations as a content-addressed receipt.
 *
 * Throws rather than emitting a partial or unbound receipt. The returned receipt
 * preserves what was observed; it does not admit the row.
 */
export const recordSarahLiveKitGateObservation = (
  input: Readonly<{
    row: GateRowId;
    binding: GateBinding;
    operatorRefDigest: string;
    observations: readonly GateObservation[];
  }>,
  dependencies: Readonly<{ now: () => number }>,
): SarahLiveKitGateObservationReceipt => {
  if (!GATE_ROW_IDS.includes(input.row)) {
    throw new Error(`${input.row} is not a Sarah LiveKit release-gate row`);
  }
  assertBinding(input.binding);
  assertDigest(input.operatorRefDigest, "operatorRefDigest");
  assertObservations(input.row, input.observations);

  const observations = [...input.observations].sort((left, right) =>
    left.key.localeCompare(right.key),
  );
  const result = {
    schema: SARAH_LIVEKIT_GATE_OBSERVATION_SCHEMA,
    observedAt: new Date(dependencies.now()).toISOString(),
    environment: "production" as const,
    row: input.row,
    outcome: rowOutcome(observations),
    binding: input.binding,
    operatorRefDigest: input.operatorRefDigest,
    observations,
    note:
      "This receipt preserves what one operator observed for one Sarah LiveKit release-gate row " +
      "against the named binding. It is not a release-gate admission: an outcome of observed_pass " +
      "means the observations were preserved, not that the row is green.",
  };
  const resultDigest = sha256(JSON.stringify(result));
  const receipt: SarahLiveKitGateObservationReceipt = {
    ...result,
    receiptRef: `receipt.sarah_livekit_gate_observation.${resultDigest.replace(":", "_")}`,
    resultDigest,
  };
  assertPublicSafeGateObservationReceipt(receipt);
  return receipt;
};

/**
 * Digest a value that must be comparable inside a receipt but never disclosed.
 *
 * Used for participant, operator, and floor-holder identities. Callers pass the
 * raw ref; only the digest is ever recorded.
 */
export const digestGateRef = (ref: string): string => {
  if (ref.trim() === "" || ref.length > 256) {
    throw new Error("gate ref is invalid");
  }
  return createHash("sha256").update(ref).digest("hex");
};

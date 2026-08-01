import { createHash, createHmac, randomBytes } from "node:crypto";
import {
  FORCED_TRANSPORT_PROFILES,
  observedTransportKind,
  satisfiesForcedTransportProfile,
  type ForcedTransportProfile,
  type SelectedIcePathClassification,
} from "./ice-classification.js";

export const SARAH_LIVEKIT_ACCEPTANCE_SCHEMA = "openagents.sarah.livekit-acceptance.v4" as const;
export const MAX_ACCEPTANCE_INTERRUPT_AUDIO_TAIL_MS = 750;

/** The authority identities a Sarah voice generation binds, in receipt order. */
export const IDENTITY_DIGEST_KEYS = [
  "job",
  "providerSession",
  "providerConfiguration",
  "context",
  "capability",
  "hold",
  "usage",
  "settlement",
] as const;
export type IdentityDigestKey = (typeof IDENTITY_DIGEST_KEYS)[number];

/**
 * The identities two concurrent rooms must never share.
 *
 * Each of these is minted per admitted generation, so equality across the
 * private and community rooms is a real isolation failure. The remaining four
 * (`capability`, `hold`, `usage`, `settlement`) are accounting identities a
 * future design could legitimately share, so they are reported but not enforced.
 */
export const GENERATION_BEARING_IDENTITY_DIGEST_KEYS = [
  "job",
  "providerSession",
  "providerConfiguration",
  "context",
] as const satisfies readonly IdentityDigestKey[];

/** The forced-transport cell one classified path is direct evidence for. */
export type ObservedTransportKind = ReturnType<typeof observedTransportKind>;

const MIN_RECEIPT_SALT_BYTES = 32;

export type SarahLiveKitAcceptanceScenario = Readonly<{
  kind: "private" | "community";
  bearer: string;
  subscriberGrant?: string;
  subscriberRef: string;
  ownerRef: string;
  deviceRef: string;
  threadRef: string;
  sessionRef: string;
  generation: number;
  pcm: Uint8Array;
  roomContext:
    | Readonly<{ kind: "private" }>
    | Readonly<{
        kind: "community";
        communityRef: string;
        channelRef: string;
      }>;
}>;

export type SarahLiveKitScenarioObservation = Readonly<{
  kind: "private" | "community";
  startedAtMs: number;
  endedAtMs: number;
  activeRoomStartedAtMs: number;
  activeRoomEndedAtMs: number;
  admissionLatencyMs: number;
  sessionLatencyMs: number;
  roomConnectLatencyMs: number;
  microphonePublishLatencyMs: number;
  firstSarahAudioLatencyMs: number;
  firstSarahTranscriptionLatencyMs: number;
  interruptAckLatencyMs: number;
  postInterruptAudioTailMs: number;
  selectedIcePathObserved: boolean;
  publisherIceStatsObserved: boolean;
  subscriberIceStatsObserved: boolean;
  /**
   * The classified selected ICE candidate pair for each peer connection.
   *
   * `selectedIcePathObserved` above says only that some pair was nominated and
   * carried packets, which cannot be assigned to a connectivity matrix row. The
   * classification names the candidate types and the transport protocol, so the
   * one naturally selected path is attributable. It is public-safe by
   * construction: the classifier never reads address, port, URL, related
   * address, or username fragment out of the LiveKit stats.
   */
  selectedIcePath: Readonly<{
    publisher: SelectedIcePathClassification;
    subscriber: SelectedIcePathClassification;
  }>;
  microphonePublished: true;
  sarahAudioObserved: true;
  sarahTranscriptionObserved: true;
  principalSarahObserved: true;
  controlChannelReady: true;
  interruptAckObserved: true;
  interruptedLifecycleObserved: true;
  identityDigests: Readonly<Record<IdentityDigestKey, string>>;
  providerUsage: Readonly<{
    inputTokens: number;
    outputTokens: number;
    cachedInputTokens: number;
    audioInputTokens: number;
    audioOutputTokens: number;
    chargeMsat: number;
    responseCount: number;
    transcriptionCount: number;
    cancelledResponseCount: number;
  }>;
  /**
   * Measured, never asserted: the eight authority identity digests observed for
   * this scenario are pairwise distinct. The cross-room half of isolation is
   * decided by the harness, which is the only place that holds both scenarios.
   */
  identityIsolationObserved: boolean;
  exactProviderUsageObserved: true;
  subscriberFanoutCount: number;
  audibleFanoutObserved: true;
  settlementState: "settled" | "released";
  settlementCreditMode: "metered" | "staging_owner_entitlement" | "owner_waived_unmetered";
  finalChargeMsat: number;
  settlementReceiptDigest: `sha256:${string}`;
}>;

export type SarahLiveKitPublicScenarioObservation = Readonly<
  Omit<
    SarahLiveKitScenarioObservation,
    | "startedAtMs"
    | "endedAtMs"
    | "activeRoomStartedAtMs"
    | "activeRoomEndedAtMs"
    | "identityDigests"
    | "providerUsage"
  > & {
    durationMs: number;
    activeRoomDurationMs: number;
    cancelledResponseCount: number;
    /** Which connectivity matrix cell each classified path is direct evidence for. */
    observedTransportKind: Readonly<{
      publisher: ObservedTransportKind;
      subscriber: ObservedTransportKind;
    }>;
    /**
     * Salted projection of the raw authority identity digests.
     *
     * Every scenario in one receipt is salted with the SAME fresh 32-byte value,
     * so within a receipt two projected values are equal if and only if the raw
     * digests are equal. That is exactly what an independent reviewer needs to
     * re-verify the cross-room comparison without the harness. Across receipts,
     * and against any dictionary of known digests, the projection leaks nothing:
     * the salt is fresh per receipt and is published nowhere.
     */
    comparableIdentityDigests: Readonly<Record<IdentityDigestKey, string>>;
  }
>;

export type SarahLiveKitAcceptanceReceipt = Readonly<{
  schema: typeof SARAH_LIVEKIT_ACCEPTANCE_SCHEMA;
  receiptRef: string;
  resultDigest: `sha256:${string}`;
  observedAt: string;
  environment: "production";
  apiOrigin: "https://openagents.com";
  livekitOrigin: "wss://livekit.openagents.com";
  sourceRevision: string;
  outcome: "passed";
  concurrentOverlapObserved: true;
  retainedMedia: false;
  retainedTranscript: false;
  /**
   * The transport constraint the operator imposed on this capture.
   *
   * Operator-declared, because no harness can discover a network constraint it
   * did not impose. It is checked against the classified paths, so a declaration
   * the observation contradicts fails closed rather than being recorded as a
   * passing connectivity row.
   */
  forcedTransportProfile: ForcedTransportProfile;
  scenarios: readonly SarahLiveKitPublicScenarioObservation[];
  /** Per identity, whether the private and community rooms observed different digests. */
  crossScenarioIdentityDistinctness: Readonly<Record<IdentityDigestKey, boolean>>;
  limitations: readonly [
    "source_revision_resolved_from_deployed_image_digest",
    "one_selected_ice_path_per_scenario_classified",
    "forced_transport_profile_operator_declared_not_independently_verified",
    "no_media_or_transcript_content_retained",
    "harness_retention_only_separate_cluster_privacy_scan_required",
  ];
}>;

export type SarahLiveKitAcceptanceDependencies = Readonly<{
  now: () => number;
  runScenario: (
    scenario: SarahLiveKitAcceptanceScenario,
  ) => Promise<SarahLiveKitScenarioObservation>;
  /**
   * The single per-receipt salt for the comparable identity digest projection.
   * Defaults to 32 fresh bytes of `node:crypto` randomness; injected only so a
   * test can pin it. The salt is never published.
   */
  randomSalt?: () => Uint8Array;
}>;

const sha256 = (value: string): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`;

const comparableIdentityDigests = (
  digests: Readonly<Record<IdentityDigestKey, string>>,
  receiptSalt: Uint8Array,
): Readonly<Record<IdentityDigestKey, string>> =>
  Object.fromEntries(
    IDENTITY_DIGEST_KEYS.map((key) => [
      key,
      createHmac("sha256", receiptSalt).update(digests[key]).digest("hex"),
    ]),
  ) as Readonly<Record<IdentityDigestKey, string>>;

const assertScenarioInput = (scenario: SarahLiveKitAcceptanceScenario): void => {
  if (scenario.kind !== scenario.roomContext.kind) {
    throw new Error("acceptance scenario kind disagrees with its room context");
  }
  if (scenario.bearer.trim() === "" || scenario.bearer.length > 4_096) {
    throw new Error("acceptance bearer is missing or invalid");
  }
  if (
    (scenario.subscriberGrant !== undefined &&
      (scenario.subscriberGrant.trim() === "" || scenario.subscriberGrant.length > 4_096)) ||
    scenario.subscriberRef.trim() === "" ||
    scenario.subscriberRef.length > 256
  ) {
    throw new Error("acceptance secondary subscriber identity or supplied grant is invalid");
  }
  if (
    scenario.pcm.byteLength < 24_000 ||
    scenario.pcm.byteLength > 24_000 * 2 * 30 ||
    scenario.pcm.byteLength % 2 !== 0
  ) {
    throw new Error("acceptance PCM must be 0.5 through 30 seconds of 24 kHz mono s16le audio");
  }
};

const assertObservation = (
  expectedKind: SarahLiveKitAcceptanceScenario["kind"],
  observation: SarahLiveKitScenarioObservation,
): void => {
  const timings = [
    observation.admissionLatencyMs,
    observation.sessionLatencyMs,
    observation.roomConnectLatencyMs,
    observation.microphonePublishLatencyMs,
    observation.firstSarahAudioLatencyMs,
    observation.firstSarahTranscriptionLatencyMs,
    observation.interruptAckLatencyMs,
    observation.postInterruptAudioTailMs,
  ];
  if (
    observation.kind !== expectedKind ||
    observation.startedAtMs < 0 ||
    observation.endedAtMs <= observation.startedAtMs ||
    observation.activeRoomStartedAtMs < observation.startedAtMs ||
    observation.activeRoomEndedAtMs <= observation.activeRoomStartedAtMs ||
    observation.activeRoomEndedAtMs > observation.endedAtMs ||
    timings.some((timing) => !Number.isSafeInteger(timing) || timing < 0) ||
    !Number.isSafeInteger(observation.finalChargeMsat) ||
    observation.finalChargeMsat <= 0 ||
    !observation.selectedIcePathObserved ||
    !observation.publisherIceStatsObserved ||
    !observation.subscriberIceStatsObserved ||
    !observation.selectedIcePath.publisher.packetsObserved ||
    !observation.selectedIcePath.subscriber.packetsObserved ||
    !observation.microphonePublished ||
    !observation.sarahAudioObserved ||
    !observation.sarahTranscriptionObserved ||
    !observation.principalSarahObserved ||
    !observation.controlChannelReady ||
    !observation.interruptAckObserved ||
    !observation.interruptedLifecycleObserved ||
    !observation.identityIsolationObserved ||
    !observation.exactProviderUsageObserved ||
    observation.postInterruptAudioTailMs > MAX_ACCEPTANCE_INTERRUPT_AUDIO_TAIL_MS ||
    observation.subscriberFanoutCount < 2 ||
    !observation.audibleFanoutObserved ||
    !/^sha256:[0-9a-f]{64}$/u.test(observation.settlementReceiptDigest)
  ) {
    throw new Error(`${expectedKind} LiveKit acceptance observation is incomplete`);
  }
  const identityDigests = Object.values(observation.identityDigests);
  const providerUsageTotal =
    observation.providerUsage.inputTokens +
    observation.providerUsage.outputTokens +
    observation.providerUsage.cachedInputTokens +
    observation.providerUsage.audioInputTokens +
    observation.providerUsage.audioOutputTokens;
  if (
    identityDigests.length !== IDENTITY_DIGEST_KEYS.length ||
    new Set(identityDigests).size !== identityDigests.length ||
    identityDigests.some((digest) => !/^[0-9a-f]{64}$/u.test(digest)) ||
    providerUsageTotal <= 0 ||
    observation.providerUsage.chargeMsat !== observation.finalChargeMsat ||
    observation.providerUsage.responseCount < 1 ||
    observation.providerUsage.transcriptionCount < 1 ||
    observation.providerUsage.cancelledResponseCount < 1 ||
    observation.providerUsage.cancelledResponseCount > observation.providerUsage.responseCount
  ) {
    throw new Error(`${expectedKind} LiveKit acceptance identity or usage evidence is incomplete`);
  }
};

const publicObservation = (
  observation: SarahLiveKitScenarioObservation,
  receiptSalt: Uint8Array,
  crossScenarioIdentityDistinctness: Readonly<Record<IdentityDigestKey, boolean>>,
): SarahLiveKitPublicScenarioObservation => ({
  kind: observation.kind,
  admissionLatencyMs: observation.admissionLatencyMs,
  sessionLatencyMs: observation.sessionLatencyMs,
  roomConnectLatencyMs: observation.roomConnectLatencyMs,
  microphonePublishLatencyMs: observation.microphonePublishLatencyMs,
  firstSarahAudioLatencyMs: observation.firstSarahAudioLatencyMs,
  firstSarahTranscriptionLatencyMs: observation.firstSarahTranscriptionLatencyMs,
  interruptAckLatencyMs: observation.interruptAckLatencyMs,
  postInterruptAudioTailMs: observation.postInterruptAudioTailMs,
  selectedIcePathObserved: observation.selectedIcePathObserved,
  publisherIceStatsObserved: observation.publisherIceStatsObserved,
  subscriberIceStatsObserved: observation.subscriberIceStatsObserved,
  selectedIcePath: observation.selectedIcePath,
  observedTransportKind: {
    publisher: observedTransportKind(observation.selectedIcePath.publisher),
    subscriber: observedTransportKind(observation.selectedIcePath.subscriber),
  },
  microphonePublished: observation.microphonePublished,
  sarahAudioObserved: observation.sarahAudioObserved,
  sarahTranscriptionObserved: observation.sarahTranscriptionObserved,
  principalSarahObserved: observation.principalSarahObserved,
  controlChannelReady: observation.controlChannelReady,
  interruptAckObserved: observation.interruptAckObserved,
  interruptedLifecycleObserved: observation.interruptedLifecycleObserved,
  identityIsolationObserved:
    observation.identityIsolationObserved &&
    GENERATION_BEARING_IDENTITY_DIGEST_KEYS.every((key) => crossScenarioIdentityDistinctness[key]),
  comparableIdentityDigests: comparableIdentityDigests(observation.identityDigests, receiptSalt),
  exactProviderUsageObserved: observation.exactProviderUsageObserved,
  subscriberFanoutCount: observation.subscriberFanoutCount,
  audibleFanoutObserved: observation.audibleFanoutObserved,
  settlementState: observation.settlementState,
  settlementCreditMode: observation.settlementCreditMode,
  finalChargeMsat: observation.finalChargeMsat,
  settlementReceiptDigest: observation.settlementReceiptDigest,
  durationMs: observation.endedAtMs - observation.startedAtMs,
  activeRoomDurationMs: observation.activeRoomEndedAtMs - observation.activeRoomStartedAtMs,
  cancelledResponseCount: observation.providerUsage.cancelledResponseCount,
});

export const assertPublicSafeSarahLiveKitReceipt = (
  receipt: SarahLiveKitAcceptanceReceipt,
): void => {
  const serialized = JSON.stringify(receipt);
  const forbiddenKeys = new Set([
    "authorization",
    "bearer",
    "participantgrant",
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
    // Network-locating ICE material. The classifier never reads these, and this
    // set stops a later edit from widening the projection back onto them.
    "address",
    "relatedaddress",
    "port",
    "url",
    "usernamefragment",
    // The comparable-digest salt is what keeps the projection non-invertible.
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
    )
  ) {
    throw new Error("Sarah LiveKit acceptance receipt contains private material");
  }
};

export const runSarahLiveKitAcceptance = async (
  input: Readonly<{
    sourceRevision: string;
    forcedTransportProfile: ForcedTransportProfile;
    privateScenario: SarahLiveKitAcceptanceScenario;
    communityScenario: SarahLiveKitAcceptanceScenario;
  }>,
  dependencies: SarahLiveKitAcceptanceDependencies,
): Promise<SarahLiveKitAcceptanceReceipt> => {
  if (!/^[0-9a-f]{40}$/u.test(input.sourceRevision)) {
    throw new Error("source revision must be a full lowercase Git commit");
  }
  if (!FORCED_TRANSPORT_PROFILES.includes(input.forcedTransportProfile)) {
    throw new Error("declared forced transport profile is not a known profile");
  }
  if (input.privateScenario.kind !== "private" || input.communityScenario.kind !== "community") {
    throw new Error("acceptance requires one private and one community scenario");
  }
  if (input.privateScenario.ownerRef === input.communityScenario.ownerRef) {
    throw new Error("concurrent acceptance requires two authenticated owners");
  }
  assertScenarioInput(input.privateScenario);
  assertScenarioInput(input.communityScenario);

  const results = await Promise.allSettled([
    dependencies.runScenario(input.privateScenario),
    dependencies.runScenario(input.communityScenario),
  ]);
  const rejected = results.find((result) => result.status === "rejected");
  if (rejected?.status === "rejected") {
    throw rejected.reason instanceof Error
      ? rejected.reason
      : new Error("Sarah LiveKit acceptance scenario failed");
  }
  const [privateResult, communityResult] = results;
  if (privateResult?.status !== "fulfilled" || communityResult?.status !== "fulfilled") {
    throw new Error("Sarah LiveKit acceptance matrix did not complete");
  }
  const privateObservation = privateResult.value;
  const communityObservation = communityResult.value;
  assertObservation("private", privateObservation);
  assertObservation("community", communityObservation);

  // A declared block that the observation contradicts means the block did not
  // take effect, so the capture is not evidence for the row it claims to fill.
  for (const observation of [privateObservation, communityObservation]) {
    for (const peer of ["publisher", "subscriber"] as const) {
      if (
        !satisfiesForcedTransportProfile(
          input.forcedTransportProfile,
          observation.selectedIcePath[peer],
        )
      ) {
        throw new Error(
          `${observation.kind} ${peer} selected ICE path contradicts the declared ` +
            `${input.forcedTransportProfile} transport profile`,
        );
      }
    }
  }

  const crossScenarioIdentityDistinctness = Object.fromEntries(
    IDENTITY_DIGEST_KEYS.map((key) => [
      key,
      privateObservation.identityDigests[key] !== communityObservation.identityDigests[key],
    ]),
  ) as Readonly<Record<IdentityDigestKey, boolean>>;
  for (const key of GENERATION_BEARING_IDENTITY_DIGEST_KEYS) {
    if (!crossScenarioIdentityDistinctness[key]) {
      throw new Error(
        `private and community LiveKit scenarios shared the ${key} authority identity`,
      );
    }
  }

  const concurrentOverlapObserved =
    Math.max(privateObservation.activeRoomStartedAtMs, communityObservation.activeRoomStartedAtMs) <
    Math.min(privateObservation.activeRoomEndedAtMs, communityObservation.activeRoomEndedAtMs);
  if (!concurrentOverlapObserved) {
    throw new Error("private and community acceptance scenarios did not overlap");
  }

  const receiptSalt = (dependencies.randomSalt ?? (() => randomBytes(MIN_RECEIPT_SALT_BYTES)))();
  if (receiptSalt.byteLength < MIN_RECEIPT_SALT_BYTES) {
    throw new Error("acceptance receipt salt must be at least 32 bytes");
  }
  const observedAt = new Date(dependencies.now()).toISOString();
  const scenarios = [
    publicObservation(privateObservation, receiptSalt, crossScenarioIdentityDistinctness),
    publicObservation(communityObservation, receiptSalt, crossScenarioIdentityDistinctness),
  ] as const;
  const result = {
    schema: SARAH_LIVEKIT_ACCEPTANCE_SCHEMA,
    observedAt,
    environment: "production" as const,
    apiOrigin: "https://openagents.com" as const,
    livekitOrigin: "wss://livekit.openagents.com" as const,
    sourceRevision: input.sourceRevision,
    outcome: "passed" as const,
    concurrentOverlapObserved: true as const,
    retainedMedia: false as const,
    retainedTranscript: false as const,
    forcedTransportProfile: input.forcedTransportProfile,
    scenarios,
    crossScenarioIdentityDistinctness,
    limitations: [
      "source_revision_resolved_from_deployed_image_digest",
      "one_selected_ice_path_per_scenario_classified",
      "forced_transport_profile_operator_declared_not_independently_verified",
      "no_media_or_transcript_content_retained",
      "harness_retention_only_separate_cluster_privacy_scan_required",
    ] as const,
  };
  const resultDigest = sha256(JSON.stringify(result));
  const receipt: SarahLiveKitAcceptanceReceipt = {
    ...result,
    receiptRef: `receipt.sarah_livekit_acceptance.${resultDigest.replace(":", "_")}`,
    resultDigest,
  };
  assertPublicSafeSarahLiveKitReceipt(receipt);
  return receipt;
};

export const digestSettlementReceipt = (receiptRef: string): `sha256:${string}` => {
  if (receiptRef.trim() === "" || receiptRef.length > 256) {
    throw new Error("settlement receipt ref is invalid");
  }
  return sha256(receiptRef);
};

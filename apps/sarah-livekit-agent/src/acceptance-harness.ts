import { createHash } from "node:crypto";

export const SARAH_LIVEKIT_ACCEPTANCE_SCHEMA = "openagents.sarah.livekit-acceptance.v2" as const;

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
  selectedIcePathObserved: boolean;
  publisherIceStatsObserved: boolean;
  subscriberIceStatsObserved: boolean;
  microphonePublished: true;
  sarahAudioObserved: true;
  sarahTranscriptionObserved: true;
  principalSarahObserved: true;
  identityDigests: Readonly<{
    job: string;
    providerSession: string;
    providerConfiguration: string;
    context: string;
    capability: string;
    hold: string;
    usage: string;
    settlement: string;
  }>;
  providerUsage: Readonly<{
    inputTokens: number;
    outputTokens: number;
    cachedInputTokens: number;
    audioInputTokens: number;
    audioOutputTokens: number;
    chargeMsat: number;
    responseCount: number;
    transcriptionCount: number;
  }>;
  identityIsolationObserved: true;
  exactProviderUsageObserved: true;
  subscriberFanoutCount: number;
  audibleFanoutObserved: true;
  settlementState: "settled" | "released";
  settlementCreditMode: "metered" | "staging_owner_entitlement";
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
  scenarios: readonly SarahLiveKitPublicScenarioObservation[];
  limitations: readonly [
    "source_revision_resolved_from_deployed_image_digest",
    "one_selected_ice_path_per_scenario",
    "no_media_or_transcript_content_retained",
    "harness_retention_only_separate_cluster_privacy_scan_required",
  ];
}>;

export type SarahLiveKitAcceptanceDependencies = Readonly<{
  now: () => number;
  runScenario: (
    scenario: SarahLiveKitAcceptanceScenario,
  ) => Promise<SarahLiveKitScenarioObservation>;
}>;

const sha256 = (value: string): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`;

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
    !observation.microphonePublished ||
    !observation.sarahAudioObserved ||
    !observation.sarahTranscriptionObserved ||
    !observation.principalSarahObserved ||
    !observation.identityIsolationObserved ||
    !observation.exactProviderUsageObserved ||
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
    identityDigests.length !== 8 ||
    new Set(identityDigests).size !== identityDigests.length ||
    identityDigests.some((digest) => !/^[0-9a-f]{64}$/u.test(digest)) ||
    providerUsageTotal <= 0 ||
    observation.providerUsage.chargeMsat !== observation.finalChargeMsat ||
    observation.providerUsage.responseCount < 1 ||
    observation.providerUsage.transcriptionCount < 1
  ) {
    throw new Error(`${expectedKind} LiveKit acceptance identity or usage evidence is incomplete`);
  }
};

const publicObservation = (
  observation: SarahLiveKitScenarioObservation,
): SarahLiveKitPublicScenarioObservation => ({
  kind: observation.kind,
  admissionLatencyMs: observation.admissionLatencyMs,
  sessionLatencyMs: observation.sessionLatencyMs,
  roomConnectLatencyMs: observation.roomConnectLatencyMs,
  microphonePublishLatencyMs: observation.microphonePublishLatencyMs,
  firstSarahAudioLatencyMs: observation.firstSarahAudioLatencyMs,
  firstSarahTranscriptionLatencyMs: observation.firstSarahTranscriptionLatencyMs,
  selectedIcePathObserved: observation.selectedIcePathObserved,
  publisherIceStatsObserved: observation.publisherIceStatsObserved,
  subscriberIceStatsObserved: observation.subscriberIceStatsObserved,
  microphonePublished: observation.microphonePublished,
  sarahAudioObserved: observation.sarahAudioObserved,
  sarahTranscriptionObserved: observation.sarahTranscriptionObserved,
  principalSarahObserved: observation.principalSarahObserved,
  identityIsolationObserved: observation.identityIsolationObserved,
  exactProviderUsageObserved: observation.exactProviderUsageObserved,
  subscriberFanoutCount: observation.subscriberFanoutCount,
  audibleFanoutObserved: observation.audibleFanoutObserved,
  settlementState: observation.settlementState,
  settlementCreditMode: observation.settlementCreditMode,
  finalChargeMsat: observation.finalChargeMsat,
  settlementReceiptDigest: observation.settlementReceiptDigest,
  durationMs: observation.endedAtMs - observation.startedAtMs,
  activeRoomDurationMs: observation.activeRoomEndedAtMs - observation.activeRoomStartedAtMs,
});

export const assertPublicSafeSarahLiveKitReceipt = (
  receipt: SarahLiveKitAcceptanceReceipt,
): void => {
  const serialized = JSON.stringify(receipt);
  const forbiddenKeys = new Set([
    "authorization",
    "bearer",
    "participantgrant",
    "transcriptcontent",
    "pcm",
    "audiocontent",
    "roomref",
    "ownerref",
    "deviceref",
    "communityref",
    "channelref",
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
    privateScenario: SarahLiveKitAcceptanceScenario;
    communityScenario: SarahLiveKitAcceptanceScenario;
  }>,
  dependencies: SarahLiveKitAcceptanceDependencies,
): Promise<SarahLiveKitAcceptanceReceipt> => {
  if (!/^[0-9a-f]{40}$/u.test(input.sourceRevision)) {
    throw new Error("source revision must be a full lowercase Git commit");
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

  const concurrentOverlapObserved =
    Math.max(privateObservation.activeRoomStartedAtMs, communityObservation.activeRoomStartedAtMs) <
    Math.min(privateObservation.activeRoomEndedAtMs, communityObservation.activeRoomEndedAtMs);
  if (!concurrentOverlapObserved) {
    throw new Error("private and community acceptance scenarios did not overlap");
  }

  const observedAt = new Date(dependencies.now()).toISOString();
  const scenarios = [
    publicObservation(privateObservation),
    publicObservation(communityObservation),
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
    scenarios,
    limitations: [
      "source_revision_resolved_from_deployed_image_digest",
      "one_selected_ice_path_per_scenario",
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

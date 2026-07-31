import { describe, expect, test, vi } from "vite-plus/test";
import {
  IDENTITY_DIGEST_KEYS,
  assertPublicSafeSarahLiveKitReceipt,
  digestSettlementReceipt,
  runSarahLiveKitAcceptance,
  type SarahLiveKitAcceptanceScenario,
  type SarahLiveKitScenarioObservation,
} from "./acceptance-harness.js";
import type {
  ForcedTransportProfile,
  SelectedIcePathClassification,
} from "./ice-classification.js";

const pcm = new Uint8Array(24_000);

/** Pinned so the comparable-digest projection is reproducible inside one test. */
const receiptSalt = () => new Uint8Array(32).fill(7);

const directUdpPath: SelectedIcePathClassification = {
  localCandidateType: "srflx",
  remoteCandidateType: "host",
  protocol: "udp",
  relayed: false,
  packetsObserved: true,
};

const turnTlsPath: SelectedIcePathClassification = {
  localCandidateType: "relay",
  remoteCandidateType: "host",
  protocol: "tls",
  relayed: true,
  packetsObserved: true,
};

const scenario = (
  kind: "private" | "community",
  ownerRef: string,
): SarahLiveKitAcceptanceScenario => ({
  kind,
  bearer: `oa_omega_${kind}_${"x".repeat(40)}`,
  subscriberGrant: `subscriber-grant-${kind}`,
  subscriberRef: `subscriber-${kind}`,
  ownerRef,
  deviceRef: `device-${kind}`,
  threadRef: `thread-${kind}`,
  sessionRef: `session-${kind}`,
  generation: 1,
  pcm,
  roomContext:
    kind === "private"
      ? { kind: "private" }
      : { kind: "community", communityRef: "community-secret", channelRef: "channel-secret" },
});

const observation = (
  kind: "private" | "community",
  startedAtMs: number,
  endedAtMs: number,
): SarahLiveKitScenarioObservation => ({
  kind,
  startedAtMs,
  endedAtMs,
  activeRoomStartedAtMs: startedAtMs + 100,
  activeRoomEndedAtMs: endedAtMs - 100,
  admissionLatencyMs: 10,
  sessionLatencyMs: 20,
  roomConnectLatencyMs: 30,
  microphonePublishLatencyMs: 40,
  firstSarahAudioLatencyMs: 50,
  firstSarahTranscriptionLatencyMs: 60,
  interruptAckLatencyMs: 70,
  postInterruptAudioTailMs: 80,
  selectedIcePathObserved: true,
  publisherIceStatsObserved: true,
  subscriberIceStatsObserved: true,
  selectedIcePath: { publisher: directUdpPath, subscriber: directUdpPath },
  microphonePublished: true,
  sarahAudioObserved: true,
  sarahTranscriptionObserved: true,
  principalSarahObserved: true,
  controlChannelReady: true,
  interruptAckObserved: true,
  interruptedLifecycleObserved: true,
  // Distinct per identity within a room and distinct across the two rooms, which
  // is what real per-generation minting produces.
  identityDigests: Object.fromEntries(
    IDENTITY_DIGEST_KEYS.map((key, index) => [
      key,
      `${(index + 1).toString(16).repeat(63)}${kind === "private" ? "0" : "f"}`,
    ]),
  ) as SarahLiveKitScenarioObservation["identityDigests"],
  providerUsage: {
    inputTokens: 10,
    outputTokens: 11,
    cachedInputTokens: 0,
    audioInputTokens: 12,
    audioOutputTokens: 13,
    chargeMsat: 17,
    responseCount: 1,
    transcriptionCount: 1,
    cancelledResponseCount: 1,
  },
  identityIsolationObserved: true,
  exactProviderUsageObserved: true,
  subscriberFanoutCount: 2,
  audibleFanoutObserved: true,
  settlementState: "settled",
  settlementCreditMode: "metered",
  finalChargeMsat: 17,
  settlementReceiptDigest: digestSettlementReceipt(`settlement-${kind}`),
});

describe("Sarah LiveKit production acceptance harness", () => {
  test("runs private and community scenarios concurrently and emits only public-safe evidence", async () => {
    const entered: Array<"private" | "community"> = [];
    let release: (() => void) | undefined;
    const bothEntered = new Promise<void>((resolve) => {
      release = resolve;
    });
    const runScenario = vi.fn(async (input: SarahLiveKitAcceptanceScenario) => {
      entered.push(input.kind);
      if (entered.length === 2) release?.();
      await bothEntered;
      return input.kind === "private"
        ? observation("private", 1_000, 2_000)
        : observation("community", 1_100, 2_100);
    });

    const receipt = await runSarahLiveKitAcceptance(
      {
        sourceRevision: "a".repeat(40),
        forcedTransportProfile: "unrestricted",
        privateScenario: scenario("private", "owner-private-secret"),
        communityScenario: scenario("community", "owner-community-secret"),
      },
      { now: () => Date.UTC(2026, 6, 31, 12), runScenario, randomSalt: receiptSalt },
    );

    expect(entered).toEqual(["private", "community"]);
    expect(receipt).toMatchObject({
      environment: "production",
      outcome: "passed",
      concurrentOverlapObserved: true,
      retainedMedia: false,
      retainedTranscript: false,
    });
    const serialized = JSON.stringify(receipt);
    expect(serialized).not.toContain("owner-private-secret");
    expect(serialized).not.toContain("owner-community-secret");
    expect(serialized).not.toContain("community-secret");
    expect(serialized).not.toContain("channel-secret");
    expect(serialized).not.toContain("oa_omega_");
    expect(receipt.schema).toBe("openagents.sarah.livekit-acceptance.v4");
    expect(receipt.scenarios[0]).toMatchObject({
      controlChannelReady: true,
      interruptAckObserved: true,
      interruptedLifecycleObserved: true,
      cancelledResponseCount: 1,
    });
    expect(() => assertPublicSafeSarahLiveKitReceipt(receipt)).not.toThrow();
    expect(() =>
      assertPublicSafeSarahLiveKitReceipt({
        ...receipt,
        receiptSalt: "0".repeat(64),
      } as typeof receipt),
    ).toThrow("contains private material");
    expect(() =>
      assertPublicSafeSarahLiveKitReceipt({
        ...receipt,
        address: "203.0.113.7",
      } as typeof receipt),
    ).toThrow("contains private material");
    expect(() =>
      assertPublicSafeSarahLiveKitReceipt({
        ...receipt,
        ticket: "private-ticket-material",
      } as typeof receipt),
    ).toThrow("contains private material");
  });

  test("refuses a same-owner matrix before starting either live scenario", async () => {
    const runScenario = vi.fn();
    await expect(
      runSarahLiveKitAcceptance(
        {
          sourceRevision: "b".repeat(40),
          forcedTransportProfile: "unrestricted",
          privateScenario: scenario("private", "same-owner"),
          communityScenario: scenario("community", "same-owner"),
        },
        { now: () => 0, runScenario },
      ),
    ).rejects.toThrow("requires two authenticated owners");
    expect(runScenario).not.toHaveBeenCalled();
  });

  test("refuses non-overlapping or incomplete live observations", async () => {
    const privateScenario = scenario("private", "owner-private");
    const communityScenario = scenario("community", "owner-community");
    await expect(
      runSarahLiveKitAcceptance(
        {
          sourceRevision: "c".repeat(40),
          forcedTransportProfile: "unrestricted",
          privateScenario,
          communityScenario,
        },
        {
          now: () => 0,
          runScenario: async (input) =>
            input.kind === "private"
              ? observation("private", 1_000, 2_000)
              : observation("community", 2_001, 3_000),
        },
      ),
    ).rejects.toThrow("did not overlap");

    await expect(
      runSarahLiveKitAcceptance(
        {
          sourceRevision: "d".repeat(40),
          forcedTransportProfile: "unrestricted",
          privateScenario,
          communityScenario,
        },
        {
          now: () => 0,
          runScenario: async (input) => ({
            ...observation(input.kind, 1_000, 2_000),
            selectedIcePathObserved: input.kind !== "community",
          }),
        },
      ),
    ).rejects.toThrow("community LiveKit acceptance observation is incomplete");

    await expect(
      runSarahLiveKitAcceptance(
        {
          sourceRevision: "e".repeat(40),
          forcedTransportProfile: "unrestricted",
          privateScenario,
          communityScenario,
        },
        {
          now: () => 0,
          runScenario: async (input) => ({
            ...observation(input.kind, 1_000, 2_000),
            providerUsage: {
              ...observation(input.kind, 1_000, 2_000).providerUsage,
              cancelledResponseCount: input.kind === "private" ? 0 : 2,
            },
          }),
        },
      ),
    ).rejects.toThrow("private LiveKit acceptance identity or usage evidence is incomplete");
  });

  test("refuses missing control evidence and an unbounded audio tail", async () => {
    const privateScenario = scenario("private", "owner-private");
    const communityScenario = scenario("community", "owner-community");
    await expect(
      runSarahLiveKitAcceptance(
        {
          sourceRevision: "f".repeat(40),
          forcedTransportProfile: "unrestricted",
          privateScenario,
          communityScenario,
        },
        {
          now: () => 0,
          runScenario: async (input) =>
            ({
              ...observation(input.kind, 1_000, 2_000),
              interruptAckObserved: input.kind !== "private",
            }) as SarahLiveKitScenarioObservation,
        },
      ),
    ).rejects.toThrow("private LiveKit acceptance observation is incomplete");

    await expect(
      runSarahLiveKitAcceptance(
        {
          sourceRevision: "1".repeat(40),
          forcedTransportProfile: "unrestricted",
          privateScenario,
          communityScenario,
        },
        {
          now: () => 0,
          runScenario: async (input) => ({
            ...observation(input.kind, 1_000, 2_000),
            postInterruptAudioTailMs: input.kind === "community" ? 751 : 80,
          }),
        },
      ),
    ).rejects.toThrow("community LiveKit acceptance observation is incomplete");
  });

  test("carries the classified selected ICE path and its matrix cell into the receipt", async () => {
    const receipt = await runSarahLiveKitAcceptance(
      {
        sourceRevision: "2".repeat(40),
        forcedTransportProfile: "udp_and_plaintext_tcp_blocked",
        privateScenario: scenario("private", "owner-private"),
        communityScenario: scenario("community", "owner-community"),
      },
      {
        now: () => 0,
        randomSalt: receiptSalt,
        runScenario: async (input) => ({
          ...observation(input.kind, 1_000, 2_000),
          selectedIcePath: { publisher: turnTlsPath, subscriber: turnTlsPath },
        }),
      },
    );

    expect(receipt.forcedTransportProfile).toBe("udp_and_plaintext_tcp_blocked");
    expect(receipt.scenarios[0]?.selectedIcePath).toEqual({
      publisher: turnTlsPath,
      subscriber: turnTlsPath,
    });
    expect(receipt.scenarios[0]?.observedTransportKind).toEqual({
      publisher: "turn_tls",
      subscriber: "turn_tls",
    });
    expect(receipt.limitations).toContain("one_selected_ice_path_per_scenario_classified");
    expect(receipt.limitations).toContain(
      "forced_transport_profile_operator_declared_not_independently_verified",
    );
    expect(() => assertPublicSafeSarahLiveKitReceipt(receipt)).not.toThrow();
  });

  test("refuses a declared transport block the observed path contradicts", async () => {
    await expect(
      runSarahLiveKitAcceptance(
        {
          sourceRevision: "3".repeat(40),
          forcedTransportProfile: "udp_blocked",
          privateScenario: scenario("private", "owner-private"),
          communityScenario: scenario("community", "owner-community"),
        },
        {
          now: () => 0,
          randomSalt: receiptSalt,
          // The declared block did not take effect: the capture rode direct UDP.
          runScenario: async (input) => observation(input.kind, 1_000, 2_000),
        },
      ),
    ).rejects.toThrow(
      "private publisher selected ICE path contradicts the declared udp_blocked transport profile",
    );

    await expect(
      runSarahLiveKitAcceptance(
        {
          sourceRevision: "4".repeat(40),
          forcedTransportProfile: "udp_blocked",
          privateScenario: scenario("private", "owner-private"),
          communityScenario: scenario("community", "owner-community"),
        },
        {
          now: () => 0,
          randomSalt: receiptSalt,
          runScenario: async (input) => ({
            ...observation(input.kind, 1_000, 2_000),
            selectedIcePath: {
              publisher: turnTlsPath,
              subscriber: input.kind === "community" ? directUdpPath : turnTlsPath,
            },
          }),
        },
      ),
    ).rejects.toThrow("community subscriber selected ICE path contradicts");
  });

  test("refuses a shared generation-bearing identity across the two rooms", async () => {
    await expect(
      runSarahLiveKitAcceptance(
        {
          sourceRevision: "5".repeat(40),
          forcedTransportProfile: "unrestricted",
          privateScenario: scenario("private", "owner-private"),
          communityScenario: scenario("community", "owner-community"),
        },
        {
          now: () => 0,
          randomSalt: receiptSalt,
          runScenario: async (input) => {
            const base = observation(input.kind, 1_000, 2_000);
            return {
              ...base,
              identityDigests: { ...base.identityDigests, providerSession: "c".repeat(64) },
            };
          },
        },
      ),
    ).rejects.toThrow(
      "private and community LiveKit scenarios shared the providerSession authority identity",
    );
  });

  test("projects identity digests as salted values that compare but do not invert", async () => {
    const sharedAccountingDigest = "d".repeat(64);
    const receipt = await runSarahLiveKitAcceptance(
      {
        sourceRevision: "6".repeat(40),
        forcedTransportProfile: "unrestricted",
        privateScenario: scenario("private", "owner-private"),
        communityScenario: scenario("community", "owner-community"),
      },
      {
        now: () => 0,
        randomSalt: receiptSalt,
        runScenario: async (input) => {
          const base = observation(input.kind, 1_000, 2_000);
          return {
            ...base,
            // `settlement` is not generation-bearing, so an equal value across the
            // two rooms is legal and must stay visibly equal after salting.
            identityDigests: { ...base.identityDigests, settlement: sharedAccountingDigest },
          };
        },
      },
    );

    const [privateScenarioObservation, communityScenarioObservation] = receipt.scenarios;
    expect(privateScenarioObservation).toBeDefined();
    expect(communityScenarioObservation).toBeDefined();
    for (const key of IDENTITY_DIGEST_KEYS) {
      expect(privateScenarioObservation?.comparableIdentityDigests[key]).toMatch(
        /^[0-9a-f]{64}$/u,
      );
      // Equal after salting exactly when the raw digests were equal.
      expect(
        privateScenarioObservation?.comparableIdentityDigests[key] ===
          communityScenarioObservation?.comparableIdentityDigests[key],
      ).toBe(!receipt.crossScenarioIdentityDistinctness[key]);
    }
    expect(receipt.crossScenarioIdentityDistinctness).toMatchObject({
      job: true,
      providerSession: true,
      providerConfiguration: true,
      context: true,
      settlement: false,
    });
    expect(privateScenarioObservation?.identityIsolationObserved).toBe(true);

    // The salt, and every raw digest it protects, stay out of the receipt.
    const serialized = JSON.stringify(receipt);
    expect(serialized).not.toContain(Buffer.from(receiptSalt()).toString("hex"));
    expect(serialized).not.toContain(sharedAccountingDigest);
    for (const key of IDENTITY_DIGEST_KEYS) {
      expect(serialized).not.toContain(
        observation("private", 1_000, 2_000).identityDigests[key],
      );
    }
  });

  test("refuses an unknown declared transport profile before running a scenario", async () => {
    const runScenario = vi.fn();
    await expect(
      runSarahLiveKitAcceptance(
        {
          sourceRevision: "7".repeat(40),
          forcedTransportProfile: "udp_mostly_blocked" as unknown as ForcedTransportProfile,
          privateScenario: scenario("private", "owner-private"),
          communityScenario: scenario("community", "owner-community"),
        },
        { now: () => 0, runScenario },
      ),
    ).rejects.toThrow("not a known profile");
    expect(runScenario).not.toHaveBeenCalled();
  });

  test("settlement refs are projected only as stable digests", () => {
    expect(digestSettlementReceipt("settlement-ref")).toMatch(/^sha256:[0-9a-f]{64}$/u);
    expect(digestSettlementReceipt("settlement-ref")).toBe(
      digestSettlementReceipt("settlement-ref"),
    );
    expect(() => digestSettlementReceipt("")).toThrow("invalid");
  });
});

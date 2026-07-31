import { describe, expect, test, vi } from "vite-plus/test";
import {
  assertPublicSafeSarahLiveKitReceipt,
  digestSettlementReceipt,
  runSarahLiveKitAcceptance,
  type SarahLiveKitAcceptanceScenario,
  type SarahLiveKitScenarioObservation,
} from "./acceptance-harness.js";

const pcm = new Uint8Array(24_000);

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
  microphonePublished: true,
  sarahAudioObserved: true,
  sarahTranscriptionObserved: true,
  principalSarahObserved: true,
  controlChannelReady: true,
  interruptAckObserved: true,
  interruptedLifecycleObserved: true,
  identityDigests: Object.fromEntries(
    [
      "job",
      "providerSession",
      "providerConfiguration",
      "context",
      "capability",
      "hold",
      "usage",
      "settlement",
    ].map((key, index) => [key, (index + 1).toString(16).repeat(64)]),
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
        privateScenario: scenario("private", "owner-private-secret"),
        communityScenario: scenario("community", "owner-community-secret"),
      },
      { now: () => Date.UTC(2026, 6, 31, 12), runScenario },
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
    expect(receipt.schema).toBe("openagents.sarah.livekit-acceptance.v3");
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

  test("settlement refs are projected only as stable digests", () => {
    expect(digestSettlementReceipt("settlement-ref")).toMatch(/^sha256:[0-9a-f]{64}$/u);
    expect(digestSettlementReceipt("settlement-ref")).toBe(
      digestSettlementReceipt("settlement-ref"),
    );
    expect(() => digestSettlementReceipt("")).toThrow("invalid");
  });
});

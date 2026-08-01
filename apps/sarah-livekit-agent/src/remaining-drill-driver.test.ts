import { SARAH_VOICE_SETTLEMENT_PROTOCOL_VERSION } from "@openagentsinc/audio-contract";
import { describe, expect, test, vi } from "vite-plus/test";
import type { SarahLiveKitLiveSession } from "./acceptance-livekit.js";
import type { SarahLiveKitAcceptanceScenario } from "./acceptance-harness.js";
import {
  SARAH_LIVEKIT_REMAINING_DRILL_RECEIPT_SCHEMA,
  SARAH_LIVEKIT_RETIRED_DRILL_SCENARIOS,
  assertPublicSafeSarahLiveKitRemainingDrillReceipt,
  buildSarahLiveKitRemainingDrillReceipt,
  runSarahLiveKitRemainingDrill,
  type SarahLiveKitAuthoritySnapshot,
} from "./remaining-drill-driver.js";

const revision = "a".repeat(40);
const image = `sha256:${"b".repeat(64)}`;
const privateScenario = (
  sessionRef: string,
  generation: number,
): SarahLiveKitAcceptanceScenario => ({
  kind: "private",
  bearer: "private-bearer",
  subscriberRef: `subscriber-${sessionRef}`,
  ownerRef: "owner-one",
  deviceRef: `device-${sessionRef}`,
  threadRef: `thread-${sessionRef}`,
  sessionRef,
  generation,
  pcm: new Uint8Array(4_800),
  roomContext: { kind: "private" },
});

const snapshot = (
  sessionRef: string,
  generation: number,
  overrides: Partial<SarahLiveKitAuthoritySnapshot> = {},
): SarahLiveKitAuthoritySnapshot => ({
  sessionRef,
  generation,
  state: "connected",
  closeReason: null,
  startedAtMs: generation * 1_000,
  terminalAtMs: null,
  reservationRef: `reservation-${sessionRef}`,
  settlementReceiptRef: null,
  workerJobRef: `job-${sessionRef}`,
  providerSessionRefDigest: "c".repeat(64),
  reservedMsat: 10_000,
  chargedMsat: 5_000,
  inputTokens: 10,
  outputTokens: 20,
  cachedInputTokens: 1,
  audioInputTokens: 2,
  audioOutputTokens: 3,
  responseCount: 1,
  transcriptionCount: 1,
  cancelledResponseCount: 0,
  terminalEventCount: 0,
  workerJobCount: 1,
  providerSessionCount: 1,
  activityAfterTerminalCount: 0,
  providerDisconnectApplied: false,
  providerDisconnectRequestRef: null,
  ...overrides,
});

const terminalSnapshot = (
  sessionRef: string,
  generation: number,
  reason: string,
  overrides: Partial<SarahLiveKitAuthoritySnapshot> = {},
): SarahLiveKitAuthoritySnapshot =>
  snapshot(sessionRef, generation, {
    state: "settled",
    closeReason: reason,
    terminalAtMs: generation * 1_000 + 500,
    settlementReceiptRef: `settlement-${sessionRef}`,
    terminalEventCount: 1,
    ...overrides,
  });

const fakeSession = (
  scenario: SarahLiveKitAcceptanceScenario,
  hooks: Readonly<{
    close?: () => void;
    send?: () => void;
  }> = {},
): SarahLiveKitLiveSession => {
  let now = scenario.generation * 1_000;
  return {
    kind: scenario.kind,
    identity: {
      ownerRef: scenario.ownerRef,
      deviceRef: scenario.deviceRef,
      threadRef: scenario.threadRef,
      sessionRef: scenario.sessionRef,
      generation: scenario.generation,
    },
    roomRef: `room-${scenario.sessionRef}`,
    participantRef: `participant-${scenario.sessionRef}`,
    sarahParticipantRef: "principal.sarah",
    timings: {
      startedAtMs: now,
      admissionLatencyMs: 1,
      sessionLatencyMs: 1,
      roomConnectLatencyMs: 1,
      microphonePublishLatencyMs: 1,
      activeRoomStartedAtMs: now,
      firstSarahAudioAtMs: now + 1,
      firstSarahAudioLatencyMs: 1,
    },
    room: undefined as never,
    subscriberRoom: undefined as never,
    control: {
      ready: Promise.resolve(now),
      terminal: new Promise(() => {}),
      interrupt: () => Promise.reject(new Error("unused")),
      close: () => {
        hooks.close?.();
        return Promise.resolve();
      },
      dispose: () => {},
    },
    output: undefined as never,
    subscriberOutput: undefined as never,
    fanoutAudio: Promise.resolve([now + 1, now + 1]),
    clock: {
      now: () => now,
      sleep: (durationMs) => {
        now += durationMs;
        return Promise.resolve();
      },
    },
    http: vi.fn(async () =>
      Response.json({
        schema: SARAH_VOICE_SETTLEMENT_PROTOCOL_VERSION,
        sessionRef: scenario.sessionRef,
        state: "settled",
        creditMode: "metered",
        finalChargeMsat: 5_000,
        spendableRemainingCreditMsat: 5_000,
        receiptRef: `settlement-${scenario.sessionRef}`,
      }),
    ) as never,
    sendMicrophoneTurn: () => {
      hooks.send?.();
      return Promise.resolve();
    },
    unpublishMicrophone: () => Promise.resolve(),
    release: () => Promise.resolve(),
  };
};

describe("remaining Sarah LiveKit drill drivers", () => {
  test("targets provider disconnect by the active generation digest", async () => {
    const scenario = privateScenario("provider-session", 1);
    let terminal = false;
    let requestRef: string | null = null;
    const observation = await runSarahLiveKitRemainingDrill(
      {
        scenario: "provider_disconnect",
        session: scenario,
        sourceRevision: revision,
        workerImageDigest: image,
        observationWindowMs: 10_000,
      },
      {
        openSession: () => Promise.resolve(fakeSession(scenario)),
        readAuthority: () =>
          Promise.resolve(
            terminal
              ? terminalSnapshot(scenario.sessionRef, 1, "provider_disconnect", {
                  providerDisconnectApplied: true,
                  providerDisconnectRequestRef: requestRef,
                })
              : snapshot(scenario.sessionRef, 1),
          ),
        requestProviderDisconnect: (input) => {
          requestRef = input.requestRef;
          terminal = true;
          return Promise.resolve({
            ...input,
            state: "requested",
            replayed: false,
            sharedInfrastructureMutated: false,
          });
        },
      },
    );

    expect(observation.previous.closeReason).toBe("provider_disconnect");
    expect(observation.providerDisconnect?.requestRef).toBe(requestRef);
    const receipt = buildSarahLiveKitRemainingDrillReceipt(observation);
    expect(receipt.schema).toBe(SARAH_LIVEKIT_REMAINING_DRILL_RECEIPT_SCHEMA);
    expect(receipt.providerDisconnectApplied).toBe(true);
    expect(JSON.stringify(receipt)).not.toContain(scenario.sessionRef);
  });

  test("retires hold exhaustion instead of looping against a zero hold", () => {
    expect(SARAH_LIVEKIT_RETIRED_DRILL_SCENARIOS).toEqual([
      expect.objectContaining({
        scenario: "hold_exhaustion",
        classification: "not_applicable_removed",
        authority: "owner_waived_unmetered_v1",
      }),
    ]);
  });

  test("admits a strictly later generation only after the old generation is terminal", async () => {
    const previousScenario = privateScenario("previous-session", 1);
    const freshScenario = {
      ...privateScenario("fresh-session", 2),
      deviceRef: previousScenario.deviceRef,
      threadRef: previousScenario.threadRef,
    };
    let previousTerminal = false;
    let freshTerminal = false;
    const observation = await runSarahLiveKitRemainingDrill(
      {
        scenario: "reconnect",
        session: previousScenario,
        freshSession: freshScenario,
        sourceRevision: revision,
        workerImageDigest: image,
        observationWindowMs: 10_000,
      },
      {
        openSession: (input) =>
          Promise.resolve(
            fakeSession(input, {
              close: () => {
                if (input.sessionRef === previousScenario.sessionRef) previousTerminal = true;
                else freshTerminal = true;
              },
            }),
          ),
        readAuthority: (sessionRef) => {
          if (sessionRef === previousScenario.sessionRef) {
            return Promise.resolve(
              previousTerminal
                ? terminalSnapshot(sessionRef, 1, "operator_stop")
                : snapshot(sessionRef, 1),
            );
          }
          return Promise.resolve(
            freshTerminal
              ? terminalSnapshot(sessionRef, 2, "operator_stop", {
                  startedAtMs: 2_000,
                  terminalAtMs: 2_500,
                  providerSessionRefDigest: "d".repeat(64),
                })
              : snapshot(sessionRef, 2, {
                  startedAtMs: 2_000,
                  providerSessionRefDigest: "d".repeat(64),
                }),
          );
        },
      },
    );

    expect(observation.previous.terminalAtMs).toBeLessThan(observation.fresh?.startedAtMs ?? 0);
    const receipt = buildSarahLiveKitRemainingDrillReceipt(observation);
    expect(receipt.previous.generationDigest).not.toBe(receipt.fresh?.generationDigest);
    expect(receipt.settledGenerationRevived).toBe(false);
  });

  test("public projection refuses private identifiers and credentials", () => {
    expect(() =>
      assertPublicSafeSarahLiveKitRemainingDrillReceipt({ sessionRef: "private" }),
    ).toThrow(/leaks sessionRef/u);
    expect(() =>
      assertPublicSafeSarahLiveKitRemainingDrillReceipt({ value: "https://private.invalid" }),
    ).toThrow(/private material/u);
  });
});

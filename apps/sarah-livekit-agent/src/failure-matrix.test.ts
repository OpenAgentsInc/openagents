import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { describe, expect, test } from "vite-plus/test";
import {
  SARAH_LIVEKIT_FAILURE_MATRIX_SCHEMA,
  SARAH_LIVEKIT_FAILURE_SCENARIOS,
  SARAH_LIVEKIT_SFU_LOSS_BOUND_MS,
  buildSarahLiveKitFailureMatrixReceipt,
  validateSarahLiveKitFailureMatrixObservation,
  type SarahLiveKitFailureMatrixObservation,
  type SarahLiveKitFailureScenario,
} from "./failure-matrix.js";

const digest = (value: string) => `sha256:${createHash("sha256").update(value).digest("hex")}`;

const terminalReason = {
  success: "completed",
  cancellation: "operator_stop",
  timeout: "session_expired",
  planned_worker_crash: "worker_error",
  sfu_loss: "worker_shutdown",
  provider_disconnect: "provider_disconnect",
  hold_exhaustion: "hold_exhausted",
  reconnect: "completed",
} as const;

const faultAction = {
  success: "none",
  cancellation: "client_cancel",
  timeout: "bounded_deadline",
  planned_worker_crash: "delete_exact_worker_pod",
  sfu_loss: "delete_exact_sfu_pod",
  provider_disconnect: "close_exact_provider_socket",
  hold_exhaustion: "exhaust_exact_session_hold",
  reconnect: "reconnect_after_terminal",
} as const;

const observation = (): SarahLiveKitFailureMatrixObservation => ({
  schema: SARAH_LIVEKIT_FAILURE_MATRIX_SCHEMA,
  environment: "production",
  sourceRevision: "a".repeat(40),
  workerImageDigest: digest("worker"),
  observedAt: "2026-07-31T12:00:00.000Z",
  runDigest: digest("run"),
  scenarios: SARAH_LIVEKIT_FAILURE_SCENARIOS.map((scenario, scenarioIndex) => {
    const base = scenarioIndex * 10;
    return {
      scenario,
      faultAction: faultAction[scenario],
      terminalReason: terminalReason[scenario],
      terminalState: scenario === "hold_exhaustion" ? "released" : "settled",
      startedAtMs: 1_000 + scenarioIndex * 10_000,
      terminalAtMs: 2_000 + scenarioIndex * 10_000,
      identityDigests: {
        job: digest(`identity-${base}`),
        providerSession: digest(`identity-${base + 1}`),
        generation: digest(`identity-${base + 2}`),
        hold: digest(`identity-${base + 3}`),
        usage: digest(`identity-${base + 4}`),
        settlement: digest(`identity-${base + 5}`),
      },
      usage: {
        inputTokens: 10,
        outputTokens: 11,
        cachedInputTokens: 1,
        audioInputTokens: 12,
        audioOutputTokens: 13,
        responseCount: 2,
        transcriptionCount: 1,
        cancelledResponseCount: scenario === "success" ? 0 : 1,
        chargeMsat: 17,
      },
      hold: {
        reservedMsat: 100,
        chargedMsat: 17,
        releasedMsat: 83,
      },
      settlementChargeMsat: 17,
      terminalEventCount: 1,
      maximumWorkerGenerationCount: 1,
      maximumProviderSessionCount: 1,
      freshAdmissionRequired: true,
      accountingEvidenceDigest: digest(`accounting-${scenario}`),
      faultEvidenceDigest: digest(`fault-${scenario}`),
      privacyEvidenceDigest: digest(`privacy-${scenario}`),
      secretFindings: 0,
      rawMediaFindings: 0,
      transcriptFindings: 0,
      reconnect:
        scenario === "reconnect"
          ? {
              previousGenerationDigest: digest("reconnect-previous"),
              freshGenerationDigest: digest("reconnect-fresh"),
              previousTerminalAtMs: 60_000,
              freshGenerationStartedAtMs: 60_001,
              settledGenerationRevived: false,
            }
          : null,
      sfuLoss:
        scenario === "sfu_loss"
          ? {
              sfuInstanceDigest: digest("sfu-instance"),
              workerInstanceDigest: digest("worker-instance"),
              faultInjectedAtMs: 1_000 + scenarioIndex * 10_000 + 400,
              mediaLossObservedAtMs: 1_000 + scenarioIndex * 10_000 + 700,
              roomBindingTerminalState: "cleaned",
              roomBindingObservedAtMs: 2_000 + scenarioIndex * 10_000 + 5_000,
              residualActiveRoomBindingCount: 0,
              residualWorkerGenerationCount: 0,
              residualProviderSessionCount: 0,
              concurrentBillableSessionCount: 1,
            }
          : null,
    };
  }),
});

const mutateScenario = (
  input: SarahLiveKitFailureMatrixObservation,
  scenario: SarahLiveKitFailureScenario,
  mutate: (value: Record<string, unknown>) => void,
): SarahLiveKitFailureMatrixObservation => {
  const copy = structuredClone(input) as unknown as {
    scenarios: Array<Record<string, unknown>>;
  };
  const target = copy.scenarios.find((candidate) => candidate["scenario"] === scenario);
  if (target === undefined) throw new Error(`test scenario ${scenario} is absent`);
  mutate(target);
  return copy as unknown as SarahLiveKitFailureMatrixObservation;
};

describe("Sarah LiveKit terminal failure matrix", () => {
  test("keeps the owner-gated CLI non-mutating and private input outside Git", () => {
    const source = readFileSync(new URL("./failure-matrix-cli.ts", import.meta.url), "utf8");
    expect(source).toContain("I_ACCEPT_EP263_SARAH_FAILURE_MATRIX");
    expect(source).toContain("private failure-matrix observation must remain outside");
    expect(source).toContain("receipt path must be under docs/ops/receipts/livekit");
    expect(source).toContain('flag: "wx"');
    expect(source).toContain("must be from the last 24 hours");
    expect(source).not.toMatch(
      /from "node:child_process"|execFile|spawnSync|fetch\(|kubectl|gcloud/u,
    );
  });

  test("requires every scenario and projects exact accounting without private authority refs", () => {
    const input = observation();
    expect(validateSarahLiveKitFailureMatrixObservation(input)).toBe(input);
    const receipt = buildSarahLiveKitFailureMatrixReceipt(input);

    expect(receipt).toMatchObject({
      issueRef: "github-issue-ref://OpenAgentsInc/openagents/9285",
      outcome: "passed",
      liveProof: true,
      retainedMedia: false,
      retainedTranscript: false,
      aggregateUsage: {
        inputTokens: 80,
        outputTokens: 88,
        chargeMsat: 136,
      },
    });
    expect(receipt.scenarios.map((scenario) => scenario.scenario)).toEqual(
      SARAH_LIVEKIT_FAILURE_SCENARIOS,
    );
    expect(receipt.scenarios.every((scenario) => scenario.exactAccounting)).toBe(true);
    const serialized = JSON.stringify(receipt);
    expect(serialized).not.toContain("identity-");
    expect(serialized).not.toContain("reconnect-previous");
    expect(receipt.receiptRef).toMatch(
      /^sarah-livekit-failure-matrix-receipt-ref:\/\/sha256\/[0-9a-f]{64}$/u,
    );
  });

  test("rejects settlement disagreement, duplicate terminals, overlap, and privacy findings", () => {
    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        mutateScenario(observation(), "cancellation", (scenario) => {
          scenario["settlementChargeMsat"] = 18;
        }),
      ),
    ).toThrow("do not reconcile");

    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        mutateScenario(observation(), "timeout", (scenario) => {
          scenario["terminalEventCount"] = 2;
        }),
      ),
    ).toThrow("duplicate terminal");

    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        mutateScenario(observation(), "planned_worker_crash", (scenario) => {
          scenario["maximumWorkerGenerationCount"] = 2;
        }),
      ),
    ).toThrow("overlapped worker");

    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        mutateScenario(observation(), "provider_disconnect", (scenario) => {
          scenario["maximumProviderSessionCount"] = 2;
        }),
      ),
    ).toThrow("overlapped provider");

    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        mutateScenario(observation(), "hold_exhaustion", (scenario) => {
          scenario["rawMediaFindings"] = 1;
        }),
      ),
    ).toThrow("rawMediaFindings is nonzero");
  });

  test("requires reconnect to start after terminal with a new generation", () => {
    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        mutateScenario(observation(), "reconnect", (scenario) => {
          const reconnect = scenario["reconnect"] as Record<string, unknown>;
          reconnect["freshGenerationDigest"] = reconnect["previousGenerationDigest"];
        }),
      ),
    ).toThrow("fresh generation");

    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        mutateScenario(observation(), "reconnect", (scenario) => {
          const reconnect = scenario["reconnect"] as Record<string, unknown>;
          reconnect["freshGenerationStartedAtMs"] = reconnect["previousTerminalAtMs"];
        }),
      ),
    ).toThrow("overlapped the terminal generation");

    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        mutateScenario(observation(), "reconnect", (scenario) => {
          const reconnect = scenario["reconnect"] as Record<string, unknown>;
          reconnect["settledGenerationRevived"] = true;
        }),
      ),
    ).toThrow("revived a settled generation");
  });

  test("bounds SFU loss against the fault instant and publishes the elapsed time", () => {
    const receipt = buildSarahLiveKitFailureMatrixReceipt(observation());
    const sfuLoss = receipt.scenarios.find((scenario) => scenario.scenario === "sfu_loss");
    expect(sfuLoss?.faultAction).toBe("delete_exact_sfu_pod");
    expect(sfuLoss?.sfuLoss).toMatchObject({
      faultToTerminalMs: 600,
      mediaLossDetectedWithinMs: 300,
      boundMs: SARAH_LIVEKIT_SFU_LOSS_BOUND_MS,
      roomBindingTerminalState: "cleaned",
      workerInstanceSurvived: true,
      concurrentBillableSessionCount: 1,
    });
    expect(receipt.scenarios.filter((scenario) => scenario.sfuLoss !== null)).toHaveLength(1);

    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        mutateScenario(observation(), "sfu_loss", (scenario) => {
          scenario["terminalAtMs"] =
            (scenario["sfuLoss"] as Record<string, number>)["faultInjectedAtMs"]! +
            SARAH_LIVEKIT_SFU_LOSS_BOUND_MS +
            1;
        }),
      ),
    ).toThrow("exceeded its 30000 ms bound");
  });

  test("refuses an SFU-loss drill that proves nothing about SFU loss", () => {
    // The session merely ran out. That is the timeout scenario wearing a
    // different fault label, with a hold pinned for the intervening minutes.
    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        mutateScenario(observation(), "sfu_loss", (scenario) => {
          scenario["terminalReason"] = "session_expired";
        }),
      ),
    ).toThrow("sfu_loss terminal reason is invalid");

    // A clean finish for a session whose transport was destroyed underneath it
    // is the silent-loss failure this row exists to catch.
    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        mutateScenario(observation(), "sfu_loss", (scenario) => {
          scenario["terminalReason"] = "completed";
        }),
      ),
    ).toThrow("sfu_loss terminal reason is invalid");

    // The fault landed on the Sarah worker, so this is planned_worker_crash.
    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        mutateScenario(observation(), "sfu_loss", (scenario) => {
          const sfuLoss = scenario["sfuLoss"] as Record<string, unknown>;
          sfuLoss["workerInstanceDigest"] = sfuLoss["sfuInstanceDigest"];
        }),
      ),
    ).toThrow("destroyed the Sarah worker instance");

    // Media loss cannot precede the fault that caused it.
    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        mutateScenario(observation(), "sfu_loss", (scenario) => {
          const sfuLoss = scenario["sfuLoss"] as Record<string, number>;
          sfuLoss["mediaLossObservedAtMs"] = sfuLoss["faultInjectedAtMs"]! - 1;
        }),
      ),
    ).toThrow("before the fault that caused it");

    // The reconciler gave up: the room is orphaned at the SFU.
    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        mutateScenario(observation(), "sfu_loss", (scenario) => {
          (scenario["sfuLoss"] as Record<string, unknown>)["roomBindingTerminalState"] =
            "cleanup_abandoned";
        }),
      ),
    ).toThrow("nonterminal or abandoned state");

    // Another owner's session shared the destroyed instance.
    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        mutateScenario(observation(), "sfu_loss", (scenario) => {
          (scenario["sfuLoss"] as Record<string, unknown>)["concurrentBillableSessionCount"] = 2;
        }),
      ),
    ).toThrow("billable session other than its own");

    // Only sfu_loss carries this evidence.
    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        mutateScenario(observation(), "timeout", (scenario) => {
          scenario["sfuLoss"] = {
            sfuInstanceDigest: digest("stray-sfu"),
            workerInstanceDigest: digest("stray-worker"),
            faultInjectedAtMs: 20_500,
            mediaLossObservedAtMs: 20_600,
            roomBindingTerminalState: "cleaned",
            roomBindingObservedAtMs: 30_000,
            residualActiveRoomBindingCount: 0,
            residualWorkerGenerationCount: 0,
            residualProviderSessionCount: 0,
            concurrentBillableSessionCount: 1,
          };
        }),
      ),
    ).toThrow("has sfu-loss-only evidence");
  });

  test("rejects duplicate identities and private material before receipt projection", () => {
    const duplicate = structuredClone(observation()) as unknown as {
      scenarios: Array<{ identityDigests: { job: string } }>;
    };
    duplicate.scenarios[1]!.identityDigests.job = duplicate.scenarios[0]!.identityDigests.job;
    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        duplicate as unknown as SarahLiveKitFailureMatrixObservation,
      ),
    ).toThrow("authority identities overlap");

    const privateMaterial = structuredClone(observation()) as unknown as Record<string, unknown>;
    privateMaterial["authorization"] = "Bearer private";
    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        privateMaterial as unknown as SarahLiveKitFailureMatrixObservation,
      ),
    ).toThrow("fields are invalid");
  });
});

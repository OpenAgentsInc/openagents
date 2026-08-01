import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { describe, expect, test } from "vite-plus/test";
import {
  SARAH_LIVEKIT_FAILURE_MATRIX_SCHEMA,
  SARAH_LIVEKIT_FAILURE_SCENARIOS,
  SARAH_LIVEKIT_SFU_LOSS_BOUND_MS,
  buildSarahLiveKitFailureMatrixReceipt,
  validateSarahLiveKitFailureMatrixAuthorityRows,
  validateSarahLiveKitFailureMatrixObservation,
  type SarahLiveKitFailureMatrixObservation,
  type SarahLiveKitFailureScenario,
} from "./failure-matrix.js";

const digest = (value: string) => `sha256:${createHash("sha256").update(value).digest("hex")}`;
const rawDigest = (value: string) => createHash("sha256").update(value).digest("hex");
const authorityCapture = (value: string, generation: number) => {
  const ledgerDigest = rawDigest(`ledger-${value}`);
  const captureDigest = rawDigest(`capture-${value}`);
  return {
    schema: "openagents.sarah.unmetered-authority-capture.v1" as const,
    authority: "owner_waived_unmetered_v1" as const,
    generation,
    sessionRefDigest: rawDigest(`session-${value}`),
    startLedgerStateDigest: ledgerDigest,
    endLedgerStateDigest: ledgerDigest,
    startBalanceStateDigest: rawDigest(`balance-${value}`),
    endBalanceStateDigest: rawDigest(`balance-${value}`),
    ledgerMutationCount: 0 as const,
    captureReceiptRef: `sarah_voice_unmetered_authority:${captureDigest}`,
    captureDigest,
  };
};

const terminalReason = {
  success: "completed",
  cancellation: "operator_stop",
  timeout: "session_expired",
  planned_worker_crash: "worker_error",
  sfu_loss: "worker_shutdown",
  provider_disconnect: "provider_disconnect",
  reconnect: "completed",
} as const;

const faultAction = {
  success: "none",
  cancellation: "client_cancel",
  timeout: "bounded_deadline",
  planned_worker_crash: "delete_exact_worker_pod",
  sfu_loss: "delete_exact_sfu_pod",
  provider_disconnect: "close_exact_provider_socket",
  reconnect: "reconnect_after_terminal",
} as const;

const observation = (): SarahLiveKitFailureMatrixObservation => ({
  schema: SARAH_LIVEKIT_FAILURE_MATRIX_SCHEMA,
  environment: "production",
  sourceRevision: "a".repeat(40),
  workerImageDigest: digest("worker"),
  observedAt: "2026-07-31T12:00:00.000Z",
  runDigest: digest("run"),
  retiredScenarios: [
    {
      scenario: "hold_exhaustion",
      classification: "not_applicable_removed",
      authority: "owner_waived_unmetered_v1",
      observedAtMs: 90_000,
      generationDigest: `sha256:${rawDigest("session-retired")}`,
      admissionEvidenceDigest: digest("retired-admission"),
      sessionEvidenceDigest: digest("retired-session"),
      unmeteredAuthorityCapture: authorityCapture("retired", 80),
      requiredHoldMsat: 0,
      spendableRemainingCreditMsat: null,
      reservedMsat: 0,
      chargedMsat: 0,
      ledgerMutationCount: 0,
    },
  ],
  scenarios: SARAH_LIVEKIT_FAILURE_SCENARIOS.map((scenario, scenarioIndex) => {
    const base = scenarioIndex * 10;
    return {
      scenario,
      faultAction: faultAction[scenario],
      terminalReason: terminalReason[scenario],
      terminalState: scenario === "provider_disconnect" ? "accounting_uncertain" : "released",
      providerAccountingStatus: scenario === "provider_disconnect" ? "uncertain" : "exact",
      creditMode: "owner_waived_unmetered",
      startedAtMs: 1_000 + scenarioIndex * 10_000,
      terminalAtMs: 2_000 + scenarioIndex * 10_000,
      identityDigests: {
        job: digest(`identity-${base}`),
        providerSession: digest(`identity-${base + 1}`),
        generation: `sha256:${rawDigest(`session-${scenario}`)}`,
        hold: digest(`identity-${base + 3}`),
        usage: digest(`identity-${base + 4}`),
        settlement: digest(
          scenario === "provider_disconnect"
            ? `sarah_voice_accounting_uncertain:session-${scenario}:${scenarioIndex + 1}`
            : `sarah_voice_settlement:session-${scenario}`,
        ),
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
        chargeMsat: 0,
      },
      hold: {
        reservedMsat: 0,
        chargedMsat: 0,
        releasedMsat: 0,
      },
      unmeteredAuthorityCapture: authorityCapture(scenario, scenarioIndex + 1),
      settlementChargeMsat: 0,
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
    const pathSource = readFileSync(new URL("./failure-matrix-paths.ts", import.meta.url), "utf8");
    expect(source).toContain("I_ACCEPT_EP263_SARAH_FAILURE_MATRIX");
    expect(pathSource).toContain("private failure-matrix observation must remain outside");
    expect(pathSource).toContain("receipt path must be under docs/ops/receipts/livekit");
    expect(source).toContain('flag: "wx"');
    expect(source).toContain("must be from the last 24 hours");
    expect(source).toContain("SARAH_FAILURE_MATRIX_EXPECTED_PRODUCTION_DATABASE");
    expect(source).toContain('spawn("psql"');
    expect(source).toContain(
      "WHEN session.close_reason = 'livekit_worker_heartbeat_expired' THEN 'worker_error'",
    );
    expect(source).not.toMatch(/execFile|spawnSync|fetch\(|kubectl|gcloud/u);
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
        inputTokens: 70,
        outputTokens: 77,
        chargeMsat: 0,
      },
    });
    expect(receipt.scenarios.map((scenario) => scenario.scenario)).toEqual(
      SARAH_LIVEKIT_FAILURE_SCENARIOS,
    );
    expect(
      receipt.scenarios.find((scenario) => scenario.scenario === "provider_disconnect"),
    ).toMatchObject({
      terminalState: "accounting_uncertain",
      providerAccountingStatus: "uncertain",
      exactAccounting: false,
    });
    expect(
      receipt.scenarios
        .filter((scenario) => scenario.scenario !== "provider_disconnect")
        .every((scenario) => scenario.exactAccounting),
    ).toBe(true);
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
    ).toThrow("mutated owner-waived platform credit");

    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        mutateScenario(observation(), "success", (scenario) => {
          scenario["creditMode"] = "metered";
        }),
      ),
    ).toThrow("did not use owner-waived unmetered authority");

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
        mutateScenario(observation(), "provider_disconnect", (scenario) => {
          scenario["rawMediaFindings"] = 1;
        }),
      ),
    ).toThrow("rawMediaFindings is nonzero");

    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        mutateScenario(observation(), "success", (scenario) => {
          scenario["terminalState"] = "accounting_uncertain";
          scenario["providerAccountingStatus"] = "uncertain";
        }),
      ),
    ).toThrow("must have exact released accounting");

    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        mutateScenario(observation(), "provider_disconnect", (scenario) => {
          scenario["terminalState"] = "released";
          scenario["providerAccountingStatus"] = "exact";
        }),
      ),
    ).toThrow("truthful uncertain provider accounting");
  });

  test.each(["planned_worker_crash", "sfu_loss"] as const)(
    "publishes truthful uncertain accounting for abrupt %s loss",
    (scenarioName) => {
      const input = mutateScenario(observation(), scenarioName, (scenario) => {
        scenario["terminalState"] = "accounting_uncertain";
        scenario["providerAccountingStatus"] = "uncertain";
      });

      expect(validateSarahLiveKitFailureMatrixObservation(input)).toBe(input);
      expect(
        buildSarahLiveKitFailureMatrixReceipt(input).scenarios.find(
          (scenario) => scenario.scenario === scenarioName,
        ),
      ).toMatchObject({
        terminalState: "accounting_uncertain",
        providerAccountingStatus: "uncertain",
        settlementChargeMsat: 0,
        holdReleasedMsat: 0,
        noLedgerMutation: true,
        exactAccounting: false,
      });
    },
  );

  test("keeps success, cancellation, timeout, and reconnect exact-only", () => {
    for (const scenarioName of ["success", "cancellation", "timeout", "reconnect"] as const) {
      expect(() =>
        validateSarahLiveKitFailureMatrixObservation(
          mutateScenario(observation(), scenarioName, (scenario) => {
            scenario["terminalState"] = "accounting_uncertain";
            scenario["providerAccountingStatus"] = "uncertain";
          }),
        ),
      ).toThrow("must have exact released accounting");
    }
  });

  test("requires live authority evidence for the retired hold-exhaustion row", () => {
    const changedLedger = structuredClone(observation()) as unknown as {
      retiredScenarios: Array<Record<string, unknown>>;
    };
    const changedCapture = changedLedger.retiredScenarios[0]![
      "unmeteredAuthorityCapture"
    ] as Record<string, unknown>;
    changedCapture["endLedgerStateDigest"] = rawDigest("changed-ledger");
    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        changedLedger as unknown as SarahLiveKitFailureMatrixObservation,
      ),
    ).toThrow("does not prove zero ledger mutation");

    const fabricatedHold = structuredClone(observation()) as unknown as {
      retiredScenarios: Array<Record<string, unknown>>;
    };
    fabricatedHold.retiredScenarios[0]!["reservedMsat"] = 1;
    expect(() =>
      validateSarahLiveKitFailureMatrixObservation(
        fabricatedHold as unknown as SarahLiveKitFailureMatrixObservation,
      ),
    ).toThrow("does not prove zero hold and zero ledger mutation");
  });

  test("requires every caller-supplied capture to match production authority rows", () => {
    const input = observation();
    expect(() => validateSarahLiveKitFailureMatrixAuthorityRows(input, [])).toThrow("row count");
    const captures = [
      ...input.scenarios.map((scenario) => scenario.unmeteredAuthorityCapture),
      input.retiredScenarios[0].unmeteredAuthorityCapture,
    ];
    const sessionRefs = [...SARAH_LIVEKIT_FAILURE_SCENARIOS, "retired"];
    const rows = captures.map((capture, index) => {
      const scenario = SARAH_LIVEKIT_FAILURE_SCENARIOS[index];
      const sessionRef = `session-${sessionRefs[index]}`;
      const uncertain = scenario === "provider_disconnect";
      const terminalAuthorityRef = uncertain
        ? `sarah_voice_accounting_uncertain:${sessionRef}:${index + 1}`
        : `sarah_voice_settlement:${sessionRef}`;
      return {
        ...capture,
        sessionRef,
        sessionState: uncertain ? "accounting_uncertain" : "released",
        creditMode: "owner_waived_unmetered",
        providerAccountingStatus: scenario === undefined ? null : uncertain ? "uncertain" : "exact",
        closeReason: scenario === undefined ? "completed" : terminalReason[scenario],
        reservedMsat: 0,
        chargedMsat: 0,
        settlementReceiptRef: uncertain || scenario === undefined ? null : terminalAuthorityRef,
        terminalAuthorityRef,
        inputTokens: scenario === undefined ? 0 : 10,
        outputTokens: scenario === undefined ? 0 : 11,
        cachedInputTokens: scenario === undefined ? 0 : 1,
        audioInputTokens: scenario === undefined ? 0 : 12,
        audioOutputTokens: scenario === undefined ? 0 : 13,
        usageChargeMsat: 0,
        responseCount: scenario === undefined ? 0 : 2,
        transcriptionCount: scenario === undefined ? 0 : 1,
        cancelledResponseCount: scenario === undefined || scenario === "success" ? 0 : 1,
      };
    });
    expect(() => validateSarahLiveKitFailureMatrixAuthorityRows(input, rows)).not.toThrow();
    const crashIndex = SARAH_LIVEKIT_FAILURE_SCENARIOS.indexOf("planned_worker_crash");
    const crashSessionRef = `session-${SARAH_LIVEKIT_FAILURE_SCENARIOS[crashIndex]}`;
    const crashTerminalAuthorityRef = `sarah_voice_accounting_uncertain:${crashSessionRef}:${crashIndex + 1}`;
    const uncertainCrash = mutateScenario(input, "planned_worker_crash", (scenario) => {
      scenario["terminalState"] = "accounting_uncertain";
      scenario["providerAccountingStatus"] = "uncertain";
      const identities = scenario["identityDigests"] as Record<string, unknown>;
      identities["settlement"] = digest(crashTerminalAuthorityRef);
    });
    const exactCrashRow = rows[crashIndex]!;
    rows[crashIndex] = {
      ...exactCrashRow,
      sessionState: "accounting_uncertain",
      providerAccountingStatus: "uncertain",
      settlementReceiptRef: null,
      terminalAuthorityRef: crashTerminalAuthorityRef,
    };
    expect(() =>
      validateSarahLiveKitFailureMatrixAuthorityRows(uncertainCrash, rows),
    ).not.toThrow();
    rows[crashIndex] = exactCrashRow;
    const disconnectIndex = SARAH_LIVEKIT_FAILURE_SCENARIOS.indexOf("provider_disconnect");
    const disconnectRow = rows[disconnectIndex]!;
    rows[disconnectIndex] = {
      ...disconnectRow,
      sessionState: "released",
      providerAccountingStatus: "exact",
      settlementReceiptRef: disconnectRow.terminalAuthorityRef,
    };
    expect(() => validateSarahLiveKitFailureMatrixAuthorityRows(input, rows)).toThrow(
      "provider_disconnect production terminal authority",
    );
    rows[disconnectIndex] = disconnectRow;
    rows[0] = { ...rows[0]!, inputTokens: 999 };
    expect(() => validateSarahLiveKitFailureMatrixAuthorityRows(input, rows)).toThrow(
      "success production terminal authority",
    );
    rows[0] = { ...rows[0]!, inputTokens: 10 };
    rows[0] = { ...rows[0]!, endBalanceStateDigest: rawDigest("changed-balance") };
    expect(() => validateSarahLiveKitFailureMatrixAuthorityRows(input, rows)).toThrow(
      "does not match",
    );
    rows[0] = { ...rows[0]!, endBalanceStateDigest: rows[0]!.startBalanceStateDigest };
    rows[0] = { ...rows[0]!, captureDigest: rawDigest("fabricated") };
    expect(() => validateSarahLiveKitFailureMatrixAuthorityRows(input, rows)).toThrow(
      "does not match",
    );
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

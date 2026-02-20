import {
  decodeL402ObservabilityRecordSync,
  L402ObservabilityFieldKeys,
  type L402ObservabilityFieldKey,
  type L402ObservabilityRecord,
} from "@openagentsinc/lightning-effect";
import { Effect } from "effect";

import { runSecuritySmoke, type SecuritySmokeMode } from "./securityControls.js";
import { runSettlementSmoke, type SettlementSmokeMode } from "./smokeSettlement.js";
import { runStagingSmoke, type StagingSmokeMode } from "./smokeStaging.js";

export type ObservabilitySmokeMode = "mock" | "api";

export type ObservabilitySmokeSummary = Readonly<{
  requestId: string;
  executionPath: "hosted-node";
  records: ReadonlyArray<L402ObservabilityRecord>;
  requiredFieldKeys: ReadonlyArray<L402ObservabilityFieldKey>;
  missingFieldKeys: ReadonlyArray<L402ObservabilityFieldKey>;
  correlation: {
    requestIds: ReadonlyArray<string>;
    paywallIds: ReadonlyArray<string>;
    taskIds: ReadonlyArray<string>;
    paymentProofRefs: ReadonlyArray<string>;
  };
}>;

const toMode = (mode?: ObservabilitySmokeMode): {
  readonly stagingMode: StagingSmokeMode;
  readonly settlementMode: SettlementSmokeMode;
  readonly securityMode: SecuritySmokeMode;
} => ({
  stagingMode: mode ?? "mock",
  settlementMode: mode ?? "mock",
  securityMode: mode ?? "mock",
});

const normalizeDenyReason = (value: string | undefined): string | null =>
  typeof value === "string" && value.trim().length > 0 ? value.trim() : null;

const makeRecord = (
  input: Omit<L402ObservabilityRecord, "observedAtMs"> & {
    readonly observedAtMs?: number;
  },
): L402ObservabilityRecord =>
  decodeL402ObservabilityRecordSync({
    ...input,
    observedAtMs: input.observedAtMs ?? Date.now(),
  });

const uniqueSorted = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(values)].sort((a, b) => a.localeCompare(b));

const toCorrelationSummary = (records: ReadonlyArray<L402ObservabilityRecord>) => ({
  requestIds: uniqueSorted(
    records
      .map((row) => row.requestId)
      .filter((value): value is string => typeof value === "string" && value.length > 0),
  ),
  paywallIds: uniqueSorted(
    records
      .map((row) => row.paywallId)
      .filter((value): value is string => typeof value === "string" && value.length > 0),
  ),
  taskIds: uniqueSorted(
    records
      .map((row) => row.taskId)
      .filter((value): value is string => typeof value === "string" && value.length > 0),
  ),
  paymentProofRefs: uniqueSorted(
    records
      .map((row) => row.paymentProofRef)
      .filter((value): value is string => typeof value === "string" && value.length > 0),
  ),
});

const missingKeysForRecord = (record: L402ObservabilityRecord): ReadonlyArray<L402ObservabilityFieldKey> =>
  L402ObservabilityFieldKeys.filter((key) => !(key in record));

export const runObservabilitySmoke = (input?: {
  readonly mode?: ObservabilitySmokeMode;
  readonly requestId?: string;
}) =>
  Effect.gen(function* () {
    const requestId = input?.requestId ?? "smoke:observability";
    const mode = toMode(input?.mode);

    const staging = yield* runStagingSmoke({
      mode: mode.stagingMode,
      requestId,
    });
    const settlements = yield* runSettlementSmoke({
      mode: mode.settlementMode,
    });
    const security = yield* runSecuritySmoke({
      mode: mode.securityMode,
    });

    const settlementRecord = settlements.settlements[0] ?? null;
    const settlementCorrelation =
      settlementRecord == null
        ? null
        : settlements.correlationRefs.find((row) => row.settlementId === settlementRecord.settlementId) ?? null;

    const records: Array<L402ObservabilityRecord> = [];
    records.push(
      makeRecord({
        requestId: staging.requestId,
        userId: null,
        paywallId: null,
        taskId: null,
        endpoint: "/api/lightning/paywalls/reconcile",
        quotedCostMsats: null,
        capAppliedMsats: null,
        paidAmountMsats: null,
        paymentProofRef: null,
        cacheHit: null,
        denyReason: normalizeDenyReason(staging.failureCode),
        executor: "system",
        plane: "control",
        executionPath: "hosted-node",
        desktopSessionId: null,
        desktopRuntimeStatus: null,
        walletState: null,
        nodeSyncStatus: null,
      }),
    );

    records.push(
      makeRecord({
        requestId: staging.requestId,
        userId: null,
        paywallId: null,
        taskId: null,
        endpoint: "/l402/health",
        quotedCostMsats: null,
        capAppliedMsats: null,
        paidAmountMsats: null,
        paymentProofRef: null,
        cacheHit: null,
        denyReason:
          staging.healthOk && staging.challengeOk && staging.proxyOk
            ? null
            : "gateway_probe_failed",
        executor: "gateway",
        plane: "gateway",
        executionPath: "hosted-node",
        desktopSessionId: null,
        desktopRuntimeStatus: null,
        walletState: null,
        nodeSyncStatus: null,
      }),
    );

    if (settlementRecord) {
      records.push(
        makeRecord({
          requestId: settlementRecord.requestId ?? staging.requestId,
          userId: settlementRecord.ownerId,
          paywallId: settlementRecord.paywallId,
          taskId: settlementCorrelation?.taskId ?? null,
          endpoint: `/api/lightning/paywalls/${settlementRecord.paywallId}/settlements`,
          quotedCostMsats: settlementRecord.amountMsats,
          capAppliedMsats: settlementRecord.amountMsats,
          paidAmountMsats: settlementRecord.amountMsats,
          paymentProofRef: settlementRecord.paymentProofRef,
          cacheHit: settlementRecord.existed,
          denyReason: null,
          executor: "gateway",
          plane: "settlement",
          executionPath: "hosted-node",
          desktopSessionId: null,
          desktopRuntimeStatus: null,
          walletState: null,
          nodeSyncStatus: null,
        }),
      );

      records.push(
        makeRecord({
          requestId: settlementRecord.requestId ?? staging.requestId,
          userId: settlementRecord.ownerId,
          paywallId: settlementRecord.paywallId,
          taskId: settlementCorrelation?.taskId ?? null,
          endpoint: "openagents.com/home#l402-transactions",
          quotedCostMsats: settlementRecord.amountMsats,
          capAppliedMsats: settlementRecord.amountMsats,
          paidAmountMsats: settlementRecord.amountMsats,
          paymentProofRef: settlementRecord.paymentProofRef,
          cacheHit: settlementRecord.existed,
          denyReason: null,
          executor: "system",
          plane: "ui",
          executionPath: "hosted-node",
          desktopSessionId: null,
          desktopRuntimeStatus: null,
          walletState: null,
          nodeSyncStatus: null,
        }),
      );
    }

    records.push(
      makeRecord({
        requestId: `${requestId}:security`,
        userId: "owner_security_smoke",
        paywallId: null,
        taskId: null,
        endpoint: "/api/lightning/security/gate",
        quotedCostMsats: null,
        capAppliedMsats: null,
        paidAmountMsats: null,
        paymentProofRef: null,
        cacheHit: null,
        denyReason:
          security.globalPause.allowed === false
            ? security.globalPause.denyReasonCode ?? "global_pause_active"
            : security.ownerKillSwitch.allowed === false
              ? security.ownerKillSwitch.denyReasonCode ?? "owner_kill_switch_active"
              : null,
        executor: "system",
        plane: "control",
        executionPath: "hosted-node",
        desktopSessionId: null,
        desktopRuntimeStatus: null,
        walletState: null,
        nodeSyncStatus: null,
      }),
    );

    const missingFieldKeys = uniqueSorted(
      records.flatMap((record) => missingKeysForRecord(record)),
    ) as ReadonlyArray<L402ObservabilityFieldKey>;

    const summary: ObservabilitySmokeSummary = {
      requestId,
      executionPath: "hosted-node",
      records,
      requiredFieldKeys: L402ObservabilityFieldKeys,
      missingFieldKeys,
      correlation: toCorrelationSummary(records),
    };

    return summary;
  });

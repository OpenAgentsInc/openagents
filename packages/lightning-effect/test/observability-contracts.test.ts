import { Effect } from "effect"
import { describe, expect, it } from "@effect/vitest"

import {
  decodeL402ObservabilityRecord,
  decodeL402ObservabilityRecordSync,
  encodeL402ObservabilityRecord,
  encodeL402ObservabilityRecordSync,
  L402ObservabilityFieldKeys,
  type L402ObservabilityRecord,
} from "../src/contracts/observability.js"

const sampleRecord: L402ObservabilityRecord = {
  requestId: "req_obs_1",
  userId: "user_obs_1",
  paywallId: "pw_obs_1",
  taskId: "task_obs_1",
  endpoint: "https://api.example.com/premium",
  quotedCostMsats: 2_500,
  capAppliedMsats: 5_000,
  paidAmountMsats: 2_500,
  paymentProofRef: "lightning_preimage:abc123",
  cacheHit: false,
  denyReason: null,
  executor: "gateway",
  plane: "settlement",
  executionPath: "hosted-node",
  desktopSessionId: null,
  desktopRuntimeStatus: null,
  walletState: null,
  nodeSyncStatus: null,
  observedAtMs: 1_735_000_000_000,
}

describe("l402 observability contracts", () => {
  it.effect("decodes and encodes record contracts asynchronously", () =>
    Effect.gen(function* () {
      const decoded = yield* decodeL402ObservabilityRecord(sampleRecord)
      expect(decoded.requestId).toBe(sampleRecord.requestId)
      expect(decoded.executionPath).toBe("hosted-node")
      expect(decoded.plane).toBe("settlement")

      const encoded = yield* encodeL402ObservabilityRecord(decoded)
      expect(encoded.taskId).toBe(sampleRecord.taskId)
      expect(encoded.paymentProofRef).toBe(sampleRecord.paymentProofRef)
    }),
  )

  it.effect("supports sync decode and encode", () =>
    Effect.gen(function* () {
      const decoded = decodeL402ObservabilityRecordSync(sampleRecord)
      expect(decoded.executor).toBe("gateway")
      expect(decoded.cacheHit).toBe(false)

      const encoded = encodeL402ObservabilityRecordSync(decoded)
      expect(encoded.endpoint).toBe(sampleRecord.endpoint)
      expect(encoded.observedAtMs).toBe(sampleRecord.observedAtMs)
    }),
  )

  it.effect("exposes canonical field key ordering for deterministic completeness checks", () =>
    Effect.gen(function* () {
      expect(L402ObservabilityFieldKeys[0]).toBe("requestId")
      expect(L402ObservabilityFieldKeys[L402ObservabilityFieldKeys.length - 1]).toBe("observedAtMs")
      expect(new Set(L402ObservabilityFieldKeys).size).toBe(L402ObservabilityFieldKeys.length)

      const missing = L402ObservabilityFieldKeys.filter((key) =>
        !(key in sampleRecord),
      )
      expect(missing).toEqual([])
    }),
  )

  it.effect("fails decode when local-path compatibility fields are invalid", () =>
    Effect.gen(function* () {
      const invalid = yield* Effect.either(
        decodeL402ObservabilityRecord({
          ...sampleRecord,
          executionPath: "local-node",
          desktopSessionId: "desktop-session-1",
          desktopRuntimeStatus: "running",
          walletState: "oops",
          nodeSyncStatus: "synced",
        }),
      )
      expect(invalid._tag).toBe("Left")
    }),
  )
})

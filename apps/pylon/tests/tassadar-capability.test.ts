import { mkdtemp, readFile, rm } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { describe, expect, test } from "bun:test"

import {
  TASSADAR_EXECUTOR_CAPABILITY_REF,
  TASSADAR_EXECUTOR_SELF_TEST_RECEIPT_REF_PATTERN,
  TASSADAR_EXECUTOR_WINDOW_VERSION_REF,
  TASSADAR_TS_REPLAY_CLASS_ID,
  runTassadarExecutorSelfTest,
} from "@openagentsinc/tassadar-executor"
import { loadPinnedTassadarSelfTestWorkload } from "@openagentsinc/tassadar-executor/self-test"

import {
  PYLON_TASSADAR_SELF_TEST_FAILED_BLOCKER_REF,
  declareTassadarExecutorCapability,
  mergeTassadarCapabilityRefs,
  publishableCapabilityRefs,
  writeTassadarCapabilityEvidence,
} from "../src/tassadar-capability"

const failingSelfTest = () =>
  runTassadarExecutorSelfTest({
    workload: {
      ...loadPinnedTassadarSelfTestWorkload(),
      expectedTraceDigest: "0".repeat(64),
    },
  })

describe("pylon tassadar capability declaration (W4.1)", () => {
  test("go-online self-test really executes the pinned workload and declares the receipted profile", async () => {
    const declaration = await declareTassadarExecutorCapability()
    expect(declaration.declared).toBe(true)
    expect(declaration.capabilityRefs[0]).toBe(TASSADAR_EXECUTOR_CAPABILITY_REF)
    expect(declaration.selfTestReceiptRef).toMatch(
      TASSADAR_EXECUTOR_SELF_TEST_RECEIPT_REF_PATTERN,
    )
    expect(declaration.windowVersionRef).toBe(
      TASSADAR_EXECUTOR_WINDOW_VERSION_REF,
    )
    expect(declaration.legRefs.length).toBeGreaterThan(0)
    expect(declaration.replayClassId).toBe(TASSADAR_TS_REPLAY_CLASS_ID)
    expect(declaration.matrixRow).not.toBeNull()
    expect(declaration.blockerRefs).toEqual([])
  })

  test("a failing self-test declares nothing and surfaces the typed blocker", async () => {
    const declaration = await declareTassadarExecutorCapability({
      runSelfTest: failingSelfTest,
    })
    expect(declaration.declared).toBe(false)
    expect(declaration.capabilityRefs).toEqual([])
    expect(declaration.selfTestReceiptRef).toBeNull()
    expect(declaration.matrixRow).toBeNull()
    expect(declaration.blockerRefs).toEqual([
      PYLON_TASSADAR_SELF_TEST_FAILED_BLOCKER_REF,
    ])
    expect(declaration.refusalDetail).toBe("trace_digest_mismatch")
  })

  test("merge drops stale executor claims when the self-test stops passing", async () => {
    const passed = await declareTassadarExecutorCapability()
    const staleRefs = [
      "pylon.capability.gepa.benchmark_runner.v0.3",
      TASSADAR_EXECUTOR_CAPABILITY_REF,
      "receipt.tassadar_executor.self_test.v1.aaaaaaaaaaaaaaaa",
    ]
    const merged = mergeTassadarCapabilityRefs(staleRefs, passed)
    expect(merged).toContain(TASSADAR_EXECUTOR_CAPABILITY_REF)
    expect(merged).toContain(passed.selfTestReceiptRef!)
    expect(merged).not.toContain(
      "receipt.tassadar_executor.self_test.v1.aaaaaaaaaaaaaaaa",
    )

    const failed = await declareTassadarExecutorCapability({
      runSelfTest: failingSelfTest,
    })
    const stripped = mergeTassadarCapabilityRefs(staleRefs, failed)
    expect(stripped).toEqual(["pylon.capability.gepa.benchmark_runner.v0.3"])
  })

  test("presence never publishes an unreceipted executor claim", () => {
    expect(
      publishableCapabilityRefs([
        "pylon.capability.gepa.benchmark_runner.v0.3",
        TASSADAR_EXECUTOR_CAPABILITY_REF,
      ]),
    ).toEqual(["pylon.capability.gepa.benchmark_runner.v0.3"])
  })

  test("evidence file is written public-safe under the pylon home", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-tassadar-test-"))
    try {
      const declaration = await declareTassadarExecutorCapability()
      const path = await writeTassadarCapabilityEvidence(home, declaration)
      const stored = JSON.parse(await readFile(path, "utf8"))
      expect(stored.declared).toBe(true)
      expect(stored.selfTestReceiptRef).toBe(declaration.selfTestReceiptRef)
      expect(stored.matrixRow.replayReceiptRef).toBe(
        declaration.selfTestReceiptRef,
      )
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })
})

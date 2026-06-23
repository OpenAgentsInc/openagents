/**
 * Pylon consumer of the Tassadar capability envelope (W4.1,
 * openagents#4750): the executor capability is declared on go-online
 * only after a REAL self-test — executing the pinned digest-known
 * workload through the shared executor on this device and matching the
 * compile-pinned trace digest byte-for-byte. The declaration carries
 * the self-test receipt ref; a failed or absent self-test declares
 * nothing and surfaces a typed blocker. Same no-overclaim posture as
 * the GEPA capability envelope; serving/pricing claims stay out of
 * scope.
 */
import { writeFile } from "node:fs/promises"
import { join } from "node:path"

import {
  TASSADAR_EXECUTOR_CAPABILITY_REF,
  buildTassadarExecutorCapabilityDeclaration,
  isTassadarExecutorSelfTestReceiptRef,
  stripUnreceiptedTassadarExecutorCapability,
  type TassadarCapabilityDeclaration,
  type TassadarExecutorSelfTestReceipt,
} from "@openagentsinc/tassadar-executor"
import { runPinnedTassadarExecutorSelfTest } from "@openagentsinc/tassadar-executor/self-test"

import { assertPublicProjectionSafe } from "./state.js"
import { stripUnreceiptedServingCapability } from "./serving-capability.js"

export const PYLON_TASSADAR_SELF_TEST_FAILED_BLOCKER_REF =
  "blocker.pylon.tassadar_executor_self_test_failed"

export type PylonTassadarCapabilityDeclaration = {
  declared: boolean
  capabilityRef: typeof TASSADAR_EXECUTOR_CAPABILITY_REF
  /** Refs to merge into runtime capabilityRefs (claim + receipt) when declared. */
  capabilityRefs: string[]
  blockerRefs: string[]
  selfTestReceiptRef: string | null
  windowVersionRef: string | null
  legRefs: string[]
  replayClassId: string | null
  matrixRow: Record<string, unknown> | null
  refusalDetail: string | null
}

const declarationProjection = (
  declaration: TassadarCapabilityDeclaration,
): PylonTassadarCapabilityDeclaration => {
  const projection: PylonTassadarCapabilityDeclaration = declaration.declared
    ? {
        blockerRefs: [],
        capabilityRef: TASSADAR_EXECUTOR_CAPABILITY_REF,
        capabilityRefs: [...declaration.capabilityRefs],
        declared: true,
        legRefs: [...declaration.envelope.legRefs],
        matrixRow: { ...declaration.matrixRow },
        refusalDetail: null,
        replayClassId: declaration.envelope.replayClassId,
        selfTestReceiptRef: declaration.envelope.selfTestReceipt.receiptRef,
        windowVersionRef: declaration.envelope.windowVersionRef,
      }
    : {
        blockerRefs: [PYLON_TASSADAR_SELF_TEST_FAILED_BLOCKER_REF],
        capabilityRef: TASSADAR_EXECUTOR_CAPABILITY_REF,
        capabilityRefs: [],
        declared: false,
        legRefs: [],
        matrixRow: null,
        refusalDetail: declaration.refusal.detail,
        replayClassId: null,
        selfTestReceiptRef: null,
        windowVersionRef: null,
      }
  assertPublicProjectionSafe(projection)
  return projection
}

export type TassadarSelfTestRunner = () => Promise<TassadarExecutorSelfTestReceipt>

/**
 * Runs the executor self-test and derives the publishable declaration.
 * A runner override exists for tests only; the default executes the
 * pinned committed workload for real.
 */
export async function declareTassadarExecutorCapability(
  options: { runSelfTest?: TassadarSelfTestRunner } = {},
): Promise<PylonTassadarCapabilityDeclaration> {
  const runSelfTest = options.runSelfTest ?? runPinnedTassadarExecutorSelfTest
  const receipt = await runSelfTest()
  return declarationProjection(
    buildTassadarExecutorCapabilityDeclaration(receipt),
  )
}

/**
 * Merges a declaration into existing runtime capability refs: stale
 * executor claims and receipt refs are dropped first, so a Pylon whose
 * self-test stops passing also stops advertising.
 */
export function mergeTassadarCapabilityRefs(
  existingRefs: string[],
  declaration: PylonTassadarCapabilityDeclaration,
): string[] {
  const withoutExecutorClaims = existingRefs.filter(
    (ref) =>
      ref !== TASSADAR_EXECUTOR_CAPABILITY_REF &&
      !isTassadarExecutorSelfTestReceiptRef(ref),
  )
  return [...new Set([...withoutExecutorClaims, ...declaration.capabilityRefs])]
}

/**
 * Publishable capability refs for presence (register/heartbeat): an
 * executor claim without its self-test receipt never leaves the device, and
 * a serving claim without its self-benchmark receipt is likewise stripped
 * (book P1-6, openagents#6089) — no unproven serving overclaim is published.
 */
export function publishableCapabilityRefs(refs: string[]): string[] {
  return stripUnreceiptedServingCapability([...stripUnreceiptedTassadarExecutorCapability(refs)])
}

/** Writes the local public-safe self-test evidence file under PYLON_HOME. */
export async function writeTassadarCapabilityEvidence(
  home: string,
  declaration: PylonTassadarCapabilityDeclaration,
): Promise<string> {
  const path = join(home, "tassadar-capability.json")
  assertPublicProjectionSafe(declaration)
  await writeFile(path, `${JSON.stringify(declaration, null, 2)}\n`)
  return path
}

import { describe, expect, test } from "bun:test"
import {
  PYLON_ARTIFACT_UPLOAD_CAPABILITY_REF,
  PYLON_ASSIGNMENT_CLOSEOUT_CAPABILITY_REF,
  PYLON_GEPA_BENCHMARK_RUNNER_CAPABILITY_REF,
  PYLON_GEPA_RETAINED_TERMINAL_BENCH_CAPABILITY_REF,
  PYLON_LOCAL_SANDBOX_ISOLATION_REF,
  PYLON_PROBE_RUNTIME_BACKEND_REF,
  PYLON_PROOF_RECEIPT_CAPABILITY_REF,
  admitGepaAssignmentToEnvelope,
  createDefaultGepaCapabilityEnvelope,
  createRetainedGepaAssignmentRequirements,
} from "../src/gepa-capability"
import { computeAssignmentAdmission, type PylonAssignmentLease } from "../src/assignment"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { ensurePylonLocalState, writePresenceState } from "../src/state"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"

async function withTempHome<T>(fn: (home: string) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-gepa-test-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

function fakeGepaLease(overrides: Partial<PylonAssignmentLease> = {}): PylonAssignmentLease {
  return {
    schema: "openagents.pylon.assignment_lease.v0.3",
    assignmentRef: "assignment.public.gepa.retained.test",
    leaseRef: "lease.public.gepa.retained.test",
    goal: "Run a retained GEPA benchmark metric call using public refs only.",
    paymentMode: "no-spend",
    capabilityRefs: [
      PYLON_GEPA_BENCHMARK_RUNNER_CAPABILITY_REF,
      PYLON_GEPA_RETAINED_TERMINAL_BENCH_CAPABILITY_REF,
      PYLON_ARTIFACT_UPLOAD_CAPABILITY_REF,
      PYLON_PROOF_RECEIPT_CAPABILITY_REF,
      PYLON_ASSIGNMENT_CLOSEOUT_CAPABILITY_REF,
    ],
    backendRef: PYLON_PROBE_RUNTIME_BACKEND_REF,
    gepaRequirements: createRetainedGepaAssignmentRequirements(),
    expiresAt: "2026-06-09T01:00:00.000Z",
    ...overrides,
  }
}

describe("Pylon GEPA capability envelope", () => {
  test("selects compatible GEPA-first runtime and benchmark refs without payout or training overclaim", () => {
    const envelope = createDefaultGepaCapabilityEnvelope()
    const requirements = createRetainedGepaAssignmentRequirements()
    const admission = admitGepaAssignmentToEnvelope(envelope, requirements)

    expect(admission.admissible).toBe(true)
    expect(admission.selectedCapabilityRefs).toEqual(requirements.requiredCapabilityRefs)
    expect(admission.runtimeContractRefs).toContain("probe.benchmark_assignment.v1")
    expect(admission.runtimeContractRefs).toContain("probe.benchmark_closeout.v1")
    expect(admission.payoutReadyForSettlement).toBe(false)
    expect(admission.trainingPostponed).toBe(true)
  })

  test("rejects wrong backend, missing isolation, missing artifact/proof/closeout, stale payout, and training claims", () => {
    const envelope = createDefaultGepaCapabilityEnvelope({
      backendRefs: ["pylon.backend.other.v0.3"],
      supportedIsolationProfileRefs: [],
      supportsArtifactUpload: false,
      supportsProofReceipts: false,
      supportsCloseout: false,
      payoutReadiness: { ready: true, fresh: false, observedAt: "2026-06-09T00:00:00.000Z" },
    })
    const admission = admitGepaAssignmentToEnvelope(
      envelope,
      createRetainedGepaAssignmentRequirements({
        payoutRequired: true,
        trainingClaim: true,
      }),
    )

    expect(admission.admissible).toBe(false)
    expect(admission.blockerRefs).toContain("blocker.gepa.unsupported_backend")
    expect(admission.blockerRefs).toContain("blocker.gepa.missing_isolation_profile")
    expect(admission.blockerRefs).toContain("blocker.gepa.artifact_upload_unavailable")
    expect(admission.blockerRefs).toContain("blocker.gepa.proof_receipts_unavailable")
    expect(admission.blockerRefs).toContain("blocker.gepa.closeout_unavailable")
    expect(admission.blockerRefs).toContain("blocker.gepa.payout_readiness_stale")
    expect(admission.blockerRefs).toContain("blocker.gepa.training_claim_postponed")
  })

  test("assignment admission merges GEPA envelope blockers with heartbeat and capability gates", async () => {
    await withTempHome(async (home) => {
      const summary = createBootstrapSummary(
        parseBootstrapArgs([
          "--display-name",
          "GEPA Worker",
          "--capability-ref",
          PYLON_GEPA_BENCHMARK_RUNNER_CAPABILITY_REF,
          "--capability-ref",
          PYLON_GEPA_RETAINED_TERMINAL_BENCH_CAPABILITY_REF,
          "--capability-ref",
          PYLON_ARTIFACT_UPLOAD_CAPABILITY_REF,
          "--capability-ref",
          PYLON_PROOF_RECEIPT_CAPABILITY_REF,
          "--capability-ref",
          PYLON_ASSIGNMENT_CLOSEOUT_CAPABILITY_REF,
          "--capability-ref",
          PYLON_PROBE_RUNTIME_BACKEND_REF,
        ]),
        { PYLON_HOME: home },
        "darwin",
      )
      const state = await ensurePylonLocalState(summary)
      await writeFile(
        state.paths.runtimeState,
        `${JSON.stringify({
          lifecycle: "assignment-ready",
          displayName: "GEPA Worker",
          resourceMode: "background_20",
          capabilityRefs: [
            PYLON_GEPA_BENCHMARK_RUNNER_CAPABILITY_REF,
            PYLON_GEPA_RETAINED_TERMINAL_BENCH_CAPABILITY_REF,
            PYLON_ARTIFACT_UPLOAD_CAPABILITY_REF,
            PYLON_PROOF_RECEIPT_CAPABILITY_REF,
            PYLON_ASSIGNMENT_CLOSEOUT_CAPABILITY_REF,
            PYLON_PROBE_RUNTIME_BACKEND_REF,
          ],
          blockerRefs: [],
          updatedAt: "2026-06-09T00:00:00.000Z",
        })}\n`,
      )
      await writePresenceState(state.paths, {
        registered: true,
        linked: false,
        stale: false,
        pylonRef: state.identity.pylonRef,
        registrationRef: "registration.gepa",
        linkRef: null,
        lastHeartbeatAt: "2026-06-09T00:00:00.000Z",
        heartbeatSequence: 1,
        blockerRefs: [],
        updatedAt: "2026-06-09T00:00:00.000Z",
      })

      const refreshed = await ensurePylonLocalState(summary)
      const admissible = await computeAssignmentAdmission(refreshed, fakeGepaLease(), {
        now: () => new Date("2026-06-09T00:00:30.000Z"),
        gepaEnvelope: createDefaultGepaCapabilityEnvelope(),
      })
      const blocked = await computeAssignmentAdmission(refreshed, fakeGepaLease(), {
        now: () => new Date("2026-06-09T00:00:30.000Z"),
        gepaEnvelope: createDefaultGepaCapabilityEnvelope({
          supportedIsolationProfileRefs: [PYLON_LOCAL_SANDBOX_ISOLATION_REF],
          supportsArtifactUpload: false,
          supportsCloseout: false,
        }),
      })

      expect(admissible.admissible).toBe(true)
      expect(blocked.admissible).toBe(false)
      expect(blocked.blockerRefs).toContain("blocker.gepa.artifact_upload_unavailable")
      expect(blocked.blockerRefs).toContain("blocker.gepa.closeout_unavailable")
    })
  })
})

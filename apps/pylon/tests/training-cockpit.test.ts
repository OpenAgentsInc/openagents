import { describe, expect, test } from "bun:test"
import {
  activateTrainingWindow,
  admitTrainingEvidence,
  claimPayoutTargetWarning,
  claimTrainingLease,
  closeoutTrainingWindow,
  planTrainingWindow,
  reconcileTrainingWindow,
  readTrainingStatus,
  trainingPreflightReport,
} from "../src/training-cockpit"

const TASSADAR_EXECUTOR_CAPABILITY_REF =
  "capability.tassadar_poc.numeric_model_executor"

// CL-5035: the training cockpit CLI mirrors the desktop training verbs against
// the openagents.com training HTTP API. These tests inject a recording fetch so
// they never hit the network.
type Recorded = { url: string; method: string; auth: string | null; body: unknown }

function recordingFetch(responder: (url: string) => { status?: number; json: unknown }) {
  const calls: Recorded[] = []
  const fetchFn = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input)
    const body = init?.body ? JSON.parse(String(init.body)) : null
    const headers = (init?.headers ?? {}) as Record<string, string>
    calls.push({ url, method: init?.method ?? "GET", auth: headers.authorization ?? null, body })
    const { status = 200, json } = responder(url)
    return new Response(JSON.stringify(json), { status, headers: { "content-type": "application/json" } })
  }) as unknown as typeof fetch
  return { fetchFn, calls }
}

const base = "https://openagents.com"
const nowIso = () => "2026-06-15T00:00:00.000Z"

describe("training plan", () => {
  test("posts run + window with the admin token and returns planned", async () => {
    const rec = recordingFetch((url) =>
      url.endsWith("/api/training/runs")
        ? { json: { run: { trainingRunRef: "r1" } } }
        : { json: { window: { windowRef: "win.1" } } },
    )
    const result = (await planTrainingWindow({ baseUrl: base, adminToken: "tok", fetchFn: rec.fetchFn, nowIso })) as {
      ok: boolean
      reason: string
    }
    expect(result.ok).toBe(true)
    expect(result.reason).toBe("planned")
    expect(rec.calls.map((c) => c.url)).toEqual([
      `${base}/api/training/runs`,
      `${base}/api/training/windows/plan`,
    ])
    expect(rec.calls.every((c) => c.auth === "Bearer tok")).toBe(true)
  })

  test("requires an admin token", async () => {
    await expect(planTrainingWindow({ baseUrl: base })).rejects.toThrow(/admin/)
  })
})

describe("training window transitions", () => {
  test("activate hits the activate endpoint for the window ref", async () => {
    const rec = recordingFetch(() => ({ json: { window: { windowRef: "win.1" } } }))
    const result = (await activateTrainingWindow({ baseUrl: base, adminToken: "tok", fetchFn: rec.fetchFn, nowIso }, "win.1")) as {
      ok: boolean
      reason: string
    }
    expect(result.ok).toBe(true)
    expect(result.reason).toBe("activated")
    expect(rec.calls[0]!.url).toBe(`${base}/api/training/windows/win.1/activate`)
  })

  test("reconcile + closeout target the right endpoints", async () => {
    const rec1 = recordingFetch(() => ({ json: { window: { windowRef: "win.1" } } }))
    await reconcileTrainingWindow({ baseUrl: base, adminToken: "tok", fetchFn: rec1.fetchFn, nowIso }, "win.1")
    expect(rec1.calls[0]!.url).toBe(`${base}/api/training/windows/win.1/reconcile`)

    const rec2 = recordingFetch(() => ({ json: { window: { windowRef: "win.1" } } }))
    await closeoutTrainingWindow({ baseUrl: base, adminToken: "tok", fetchFn: rec2.fetchFn, nowIso }, "win.1")
    expect(rec2.calls[0]!.url).toBe(`${base}/api/training/windows/win.1/closeout`)
  })

  test("rejects an invalid window ref", async () => {
    await expect(
      activateTrainingWindow({ baseUrl: base, adminToken: "tok" }, "bad ref!"),
    ).rejects.toThrow(/window-ref/)
  })
})

describe("training claim", () => {
  test("posts the pylon ref to the public lease endpoint (no auth header)", async () => {
    const rec = recordingFetch(() => ({ json: { lease: { leaseRef: "l1", windowRef: "win.1" } } }))
    const result = (await claimTrainingLease({ baseUrl: base, fetchFn: rec.fetchFn, nowIso }, { pylonRef: "pylon.abc" })) as {
      ok: boolean
      reason: string
    }
    expect(result.ok).toBe(true)
    expect(result.reason).toBe("claimed")
    expect(rec.calls[0]!.url).toBe(`${base}/api/training/leases/claim`)
    expect(rec.calls[0]!.auth).toBeNull()
  })

  // Gap #2 (v1.0 self-serve shakeout): a contributor that claims without a
  // registered payout target gets a clear WARNING (not a hard block) pointing at
  // `pylon wallet register-payout-target`, surfaced in the --json result.
  test("warns about an unregistered payout target without blocking the claim", async () => {
    const rec = recordingFetch(() => ({ json: { lease: { leaseRef: "l1", windowRef: "win.1" } } }))
    const result = (await claimTrainingLease(
      { baseUrl: base, fetchFn: rec.fetchFn, nowIso },
      { pylonRef: "pylon.abc", payoutTargetRegistered: false },
    )) as { ok: boolean; reason: string; payoutTargetWarning?: { warningRef: string; command: string } }
    // The claim still succeeds — warn, do not block.
    expect(result.ok).toBe(true)
    expect(result.reason).toBe("claimed")
    expect(result.payoutTargetWarning?.warningRef).toBe("warning.training.claim.payout_target_unregistered")
    expect(result.payoutTargetWarning?.command).toBe("pylon wallet register-payout-target")
  })

  test("does NOT warn when a payout target is registered", async () => {
    const rec = recordingFetch(() => ({ json: { lease: { leaseRef: "l1", windowRef: "win.1" } } }))
    const result = (await claimTrainingLease(
      { baseUrl: base, fetchFn: rec.fetchFn, nowIso },
      { pylonRef: "pylon.abc", payoutTargetRegistered: true },
    )) as { ok: boolean; payoutTargetWarning?: unknown }
    expect(result.ok).toBe(true)
    expect(result.payoutTargetWarning).toBeUndefined()
  })

  test("still warns when the claim itself fails (so the contributor sees it early)", async () => {
    const rec = recordingFetch(() => ({ status: 409, json: { reason: "no_active_window" } }))
    const result = (await claimTrainingLease(
      { baseUrl: base, fetchFn: rec.fetchFn, nowIso },
      { pylonRef: "pylon.abc", payoutTargetRegistered: false },
    )) as { ok: boolean; reason: string; payoutTargetWarning?: { warningRef: string } }
    expect(result.ok).toBe(false)
    expect(result.reason).toBe("claim_failed")
    expect(result.payoutTargetWarning?.warningRef).toBe("warning.training.claim.payout_target_unregistered")
  })

  test("claimPayoutTargetWarning is null when registered or unresolved, set when missing", () => {
    expect(claimPayoutTargetWarning(true)).toBeNull()
    expect(claimPayoutTargetWarning(undefined)).toBeNull()
    expect(claimPayoutTargetWarning(false)?.warningRef).toBe("warning.training.claim.payout_target_unregistered")
  })
})

describe("training preflight", () => {
  test("reports the exact commands for missing payout target and self-test capability", () => {
    const report = trainingPreflightReport(
      {
        blockerRefs: [],
        capabilityRefs: [],
        lifecycle: "offline",
        pylonRef: "pylon.abc",
        sparkPayoutTargetRef: null,
      },
      { baseUrl: base },
    )

    expect(report.ok).toBe(false)
    expect(report.reason).toBe("blocked")
    expect(report.checks.payoutTarget.ok).toBe(false)
    expect(report.checks.tassadarExecutorCapability.ok).toBe(false)
    expect(report.recommendedCommands).toEqual([
      `pylon wallet register-payout-target --kind spark-address --base-url ${base}`,
      "pylon provider go-online",
      `pylon presence heartbeat --base-url ${base}`,
    ])
    expect(report.authorityBoundary).toContain("Read-only local preflight")
  })

  test("is ready when a payout target and receipted executor capability exist", () => {
    const report = trainingPreflightReport(
      {
        blockerRefs: [],
        capabilityRefs: [
          TASSADAR_EXECUTOR_CAPABILITY_REF,
          "receipt.tassadar_executor.self_test.v1.aaaaaaaaaaaaaaaa",
        ],
        lifecycle: "online",
        pylonRef: "pylon.abc",
        sparkPayoutTargetRef: "payout.spark.123",
      },
      { baseUrl: base },
    )

    expect(report.ok).toBe(true)
    expect(report.reason).toBe("ready")
    expect(report.recommendedCommands).toEqual([])
    expect(report.checks.payoutTarget.payoutTargetRef).toBe("payout.spark.123")
    expect(report.checks.tassadarExecutorCapability.selfTestReceiptRefs).toEqual([
      "receipt.tassadar_executor.self_test.v1.aaaaaaaaaaaaaaaa",
    ])
  })
})

describe("training admit", () => {
  test("posts the evidence packet to the run real-gradient endpoint", async () => {
    const rec = recordingFetch(() => ({ json: { admitted: true } }))
    const result = (await admitTrainingEvidence(
      { baseUrl: base, adminToken: "tok", fetchFn: rec.fetchFn, nowIso },
      { trainingRunRef: "run.1", packet: { evidence: 1 } },
    )) as { ok: boolean; reason: string }
    expect(result.ok).toBe(true)
    expect(result.reason).toBe("admitted")
    expect(rec.calls[0]!.url).toBe(`${base}/api/training/runs/run.1/real-gradient-evidence`)
    expect(rec.calls[0]!.body).toEqual({ evidence: 1 })
  })
})

describe("training status", () => {
  test("reads the public runs projection", async () => {
    const rec = recordingFetch(() => ({ json: { runs: [{ trainingRunRef: "r1" }] } }))
    const result = (await readTrainingStatus({ baseUrl: base, fetchFn: rec.fetchFn })) as {
      ok: boolean
      runs: unknown[]
    }
    expect(result.ok).toBe(true)
    expect(rec.calls[0]!.url).toBe(`${base}/api/training/runs`)
    expect(result.runs.length).toBe(1)
  })

  test("surfaces a non-ok response as ok:false", async () => {
    const rec = recordingFetch(() => ({ status: 500, json: { error: "boom" } }))
    const result = (await readTrainingStatus({ baseUrl: base, fetchFn: rec.fetchFn })) as {
      ok: boolean
      reason: string
    }
    expect(result.ok).toBe(false)
    expect(result.reason).toBe("status_failed")
  })
})

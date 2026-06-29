import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import {
  assertWorkloadFamily,
  discoverNextUnpaired,
  parseTassadarWorkload,
  runValidatorAuto,
  submitReplayVerdict,
  submitTraceContribution,
  TASSADAR_TRACE_WORKLOAD_FAMILIES,
  type TraceClientOptions,
} from "../src/tassadar-trace-client"
import { executeTassadarNumericModel } from "@openagentsinc/tassadar-executor"

// #5054 (epic #5051), design §4.5: contributor worker/validator verbs. These
// tests inject a recording fetch + a fake executor so they never run the real
// workload or hit the network — except one end-to-end check that uses the real
// committed fixture + real executor to prove the digest plumbing.

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

// A trivial fake workload — the fake executor ignores it and returns a fixed
// digest, so the tests assert plumbing, not numeric semantics.
const fakeWorkload = {
  model: { fake: true } as never,
  steps: [[1], [2], [3]] as ReadonlyArray<ReadonlyArray<number>>,
}

function fakeExecutor(digest: string, stepCount = 3) {
  return (async () => ({
    stepCount,
    stepOutputs: [],
    traceDigest: digest,
  })) as unknown as TraceClientOptions["executor"]
}

describe("workload family + workload parsing", () => {
  test("accepts each known workload family", () => {
    for (const family of TASSADAR_TRACE_WORKLOAD_FAMILIES) {
      expect(assertWorkloadFamily(family)).toBe(family)
    }
  })

  test("rejects an unknown workload family", () => {
    expect(() => assertWorkloadFamily("nope")).toThrow(/workload-family/)
    expect(() => assertWorkloadFamily(undefined)).toThrow(/workload-family/)
  })

  test("parses a bare workload and a dispatch wrapper", () => {
    const bare = parseTassadarWorkload({ model: { a: 1 }, steps: [[1]], expectedTraceDigest: "d" })
    expect(bare.steps).toEqual([[1]])
    expect(bare.expectedTraceDigest).toBe("d")
    const wrapped = parseTassadarWorkload({ tassadar: { model: { a: 1 }, steps: [[2]] } })
    expect(wrapped.steps).toEqual([[2]])
  })

  test("restores seed_writes from the initialChannelWrites alias", () => {
    const parsed = parseTassadarWorkload({
      model: { initialChannelWrites: [[0, 1, 2]] },
      steps: [[1]],
    })
    expect((parsed.model as unknown as { seed_writes: unknown }).seed_writes).toEqual([[0, 1, 2]])
  })

  test("rejects malformed workload JSON", () => {
    expect(() => parseTassadarWorkload(null)).toThrow()
    expect(() => parseTassadarWorkload({ steps: [[1]] })).toThrow()
    expect(() => parseTassadarWorkload({ model: { a: 1 } })).toThrow()
  })
})

describe("submit-trace (worker)", () => {
  test("runs the workload and posts the trace commitment to the §4.1 route", async () => {
    const rec = recordingFetch(() => ({ json: { contribution: { state: "pending", contributionRef: "c1" } } }))
    const result = await submitTraceContribution(
      { baseUrl: base, agentToken: "agent-tok", fetchFn: rec.fetchFn, executor: fakeExecutor("digest123"), nowIso },
      { leaseRef: "lease.abc", pylonDeviceRef: "pylon_worker", workload: fakeWorkload, workloadFamily: "sudoku_trace" },
    )
    expect(result.ok).toBe(true)
    expect(result.reason).toBe("submitted")
    expect(result.traceDigest).toBe("digest123")
    expect(result.digestMatchesExpectation).toBe(true)
    expect((result.contribution as { state: string }).state).toBe("pending")
    // Hit the right agent-gated route with the agent bearer token.
    expect(rec.calls).toHaveLength(1)
    expect(rec.calls[0]!.url).toBe(`${base}/api/training/leases/lease.abc/trace-submission`)
    expect(rec.calls[0]!.method).toBe("POST")
    expect(rec.calls[0]!.auth).toBe("Bearer agent-tok")
    const body = rec.calls[0]!.body as Record<string, unknown>
    expect(body.pylonDeviceRef).toBe("pylon_worker")
    expect(body.workloadFamily).toBe("sudoku_trace")
    expect(body.traceCommitmentDigestRef).toBe("trace.tassadar.commitment.digest123")
    expect(body.sampledWindow).toEqual({ endStep: 3, startStep: 0 })
  })

  test("flags a digest mismatch against the dispatched expectation", async () => {
    const rec = recordingFetch(() => ({ json: { contribution: { state: "pending" } } }))
    const result = await submitTraceContribution(
      { baseUrl: base, agentToken: "tok", fetchFn: rec.fetchFn, executor: fakeExecutor("actual") },
      {
        leaseRef: "lease.abc",
        pylonDeviceRef: "pylon_worker",
        workload: { ...fakeWorkload, expectedTraceDigest: "expected-different" },
        workloadFamily: "kernel_trace",
      },
    )
    expect(result.ok).toBe(true)
    expect(result.digestMatchesExpectation).toBe(false)
  })

  test("surfaces a server error without throwing", async () => {
    const rec = recordingFetch(() => ({ status: 403, json: { reason: "lease belongs to another Pylon" } }))
    const result = await submitTraceContribution(
      { baseUrl: base, agentToken: "tok", fetchFn: rec.fetchFn, executor: fakeExecutor("d") },
      { leaseRef: "lease.abc", pylonDeviceRef: "pylon_worker", workload: fakeWorkload, workloadFamily: "sudoku_trace" },
    )
    expect(result.ok).toBe(false)
    expect(result.reason).toBe("submit_trace_failed")
    expect(result.error).toMatch(/another Pylon/)
  })

  test("requires an agent token", async () => {
    await expect(
      submitTraceContribution(
        { baseUrl: base, executor: fakeExecutor("d") },
        { leaseRef: "lease.abc", pylonDeviceRef: "pylon_worker", workload: fakeWorkload, workloadFamily: "sudoku_trace" },
      ),
    ).rejects.toThrow(/agent-token/)
  })

  test("rejects an invalid lease ref before any network call", async () => {
    const rec = recordingFetch(() => ({ json: {} }))
    await expect(
      submitTraceContribution(
        { baseUrl: base, agentToken: "tok", fetchFn: rec.fetchFn, executor: fakeExecutor("d") },
        { leaseRef: "bad ref!", pylonDeviceRef: "pylon_worker", workload: fakeWorkload, workloadFamily: "sudoku_trace" },
      ),
    ).rejects.toThrow(/lease-ref/)
    expect(rec.calls).toHaveLength(0)
  })
})

describe("validate (validator)", () => {
  test("replays the workload and posts the replay digest to the §4.2 route", async () => {
    const rec = recordingFetch(() => ({
      json: { challenge: { state: "Verified" }, contribution: { state: "paired" } },
    }))
    const result = await submitReplayVerdict(
      { baseUrl: base, agentToken: "validator-tok", fetchFn: rec.fetchFn, executor: fakeExecutor("digest123") },
      { leaseRef: "lease.abc", validatorDeviceRef: "pylon_validator", workload: fakeWorkload, workloadFamily: "sudoku_trace" },
    )
    expect(result.ok).toBe(true)
    expect(result.reason).toBe("verdict_submitted")
    expect(result.replayDigest).toBe("digest123")
    expect((result.challenge as { state: string }).state).toBe("Verified")
    expect(rec.calls[0]!.url).toBe(`${base}/api/training/leases/lease.abc/replay-verdict`)
    expect(rec.calls[0]!.auth).toBe("Bearer validator-tok")
    const body = rec.calls[0]!.body as Record<string, unknown>
    expect(body.validatorDeviceRef).toBe("pylon_validator")
    expect(body.replayDigestRef).toBe("trace.tassadar.replay.digest123")
    expect(body.workloadFamily).toBe("sudoku_trace")
  })

  test("surfaces a self-validation rejection from the server", async () => {
    const rec = recordingFetch(() => ({
      status: 403,
      json: { reason: "exact_trace_replay requires a validator device distinct from the worker Pylon." },
    }))
    const result = await submitReplayVerdict(
      { baseUrl: base, agentToken: "tok", fetchFn: rec.fetchFn, executor: fakeExecutor("d") },
      { leaseRef: "lease.abc", validatorDeviceRef: "pylon_same", workload: fakeWorkload, workloadFamily: "sudoku_trace" },
    )
    expect(result.ok).toBe(false)
    expect(result.reason).toBe("validate_failed")
    expect(result.error).toMatch(/distinct/)
  })

  test("requires an agent token", async () => {
    await expect(
      submitReplayVerdict(
        { baseUrl: base, executor: fakeExecutor("d") },
        { leaseRef: "lease.abc", validatorDeviceRef: "pylon_v", workload: fakeWorkload, workloadFamily: "sudoku_trace" },
      ),
    ).rejects.toThrow(/agent-token/)
  })
})

describe("validator auto-discovery + auto-run (#5121)", () => {
  const discovered = {
    contributionRef: "contribution.tassadar_executor_trace.lease.xyz.kernel_trace",
    leaseRef: "lease.xyz",
    sampledWindow: { endStep: 32, startStep: 0 },
    trainingRunRef: "run.tassadar.executor.20260615",
    windowRef: "training.window.w1",
    workerPylonDeviceRef: "device.worker.9",
    workloadFamily: "kernel_trace",
  }

  test("discoverNextUnpaired GETs the endpoint with validatorDeviceRef + bearer", async () => {
    const rec = recordingFetch(() => ({ json: { contribution: discovered } }))
    const result = await discoverNextUnpaired(
      { agentToken: "validator-tok", baseUrl: base, fetchFn: rec.fetchFn },
      { validatorDeviceRef: "device.validator.1" },
    )
    expect(result.ok).toBe(true)
    expect(result.contribution?.leaseRef).toBe("lease.xyz")
    expect(rec.calls).toHaveLength(1)
    expect(rec.calls[0]!.method).toBe("GET")
    expect(rec.calls[0]!.auth).toBe("Bearer validator-tok")
    expect(rec.calls[0]!.url).toBe(
      `${base}/api/training/contributions/next-unpaired?validatorDeviceRef=device.validator.1`,
    )
  })

  test("discoverNextUnpaired returns null contribution when nothing is pending", async () => {
    const rec = recordingFetch(() => ({ json: { contribution: null } }))
    const result = await discoverNextUnpaired(
      { agentToken: "tok", baseUrl: base, fetchFn: rec.fetchFn },
      { validatorDeviceRef: "device.validator.1" },
    )
    expect(result.ok).toBe(true)
    expect(result.contribution).toBeNull()
  })

  test("discoverNextUnpaired surfaces a server error without throwing", async () => {
    const rec = recordingFetch(() => ({ json: { reason: "nope" }, status: 500 }))
    const result = await discoverNextUnpaired(
      { agentToken: "tok", baseUrl: base, fetchFn: rec.fetchFn },
      { validatorDeviceRef: "device.validator.1" },
    )
    expect(result.ok).toBe(false)
    expect(result.contribution).toBeNull()
  })

  test("runValidatorAuto discovers then replays + submits the verdict", async () => {
    const rec = recordingFetch(url =>
      url.includes("next-unpaired")
        ? { json: { contribution: discovered } }
        : { json: { challenge: { state: "Verified" }, contribution: { state: "paired" } } },
    )
    const result = await runValidatorAuto(
      {
        agentToken: "validator-tok",
        baseUrl: base,
        executor: fakeExecutor("digestZ"),
        fetchFn: rec.fetchFn,
      },
      { validatorDeviceRef: "device.validator.1", workload: fakeWorkload },
    )
    expect(result.ok).toBe(true)
    expect(result.paired).toBe(true)
    expect(result.discoveredContributionRef).toBe(discovered.contributionRef)
    // Two calls: GET discovery, then POST the verdict against the discovered lease.
    expect(rec.calls).toHaveLength(2)
    expect(rec.calls[0]!.method).toBe("GET")
    expect(rec.calls[1]!.url).toBe(`${base}/api/training/leases/lease.xyz/replay-verdict`)
    const body = rec.calls[1]!.body as Record<string, unknown>
    expect(body.validatorDeviceRef).toBe("device.validator.1")
    expect(body.workloadFamily).toBe("kernel_trace")
  })

  test("runValidatorAuto is idle (no verdict POST) when nothing is pending", async () => {
    const rec = recordingFetch(() => ({ json: { contribution: null } }))
    const result = await runValidatorAuto(
      { agentToken: "tok", baseUrl: base, executor: fakeExecutor("d"), fetchFn: rec.fetchFn },
      { validatorDeviceRef: "device.validator.1", workload: fakeWorkload },
    )
    expect(result.ok).toBe(true)
    expect(result.paired).toBe(false)
    expect(result.reason).toBe("idle_no_pending")
    expect(rec.calls).toHaveLength(1)
  })

  test("runValidatorAuto surfaces a discovery failure", async () => {
    const rec = recordingFetch(() => ({ json: { reason: "boom" }, status: 500 }))
    const result = await runValidatorAuto(
      { agentToken: "tok", baseUrl: base, executor: fakeExecutor("d"), fetchFn: rec.fetchFn },
      { validatorDeviceRef: "device.validator.1", workload: fakeWorkload },
    )
    expect(result.ok).toBe(false)
    expect(result.reason).toBe("discover_failed")
    expect(rec.calls).toHaveLength(1)
  })
})

describe("worker + validator agree on the real fixture digest (end to end)", () => {
  test("the real executor produces a matching digest for worker and validator", async () => {
    const fixture = JSON.parse(
      readFileSync(
        new URL("../../../packages/tassadar-executor/fixtures/tassadar-poc-loop-sum-v1.json", import.meta.url),
        "utf8",
      ),
    ) as { model: unknown; steps: unknown; expectedTraceDigest: string }
    const workload = parseTassadarWorkload(fixture)

    const workerRec = recordingFetch(() => ({ json: { contribution: { state: "pending" } } }))
    const worker = await submitTraceContribution(
      { baseUrl: base, agentToken: "tok", fetchFn: workerRec.fetchFn, executor: executeTassadarNumericModel },
      { leaseRef: "lease.real", pylonDeviceRef: "pylon_worker", workload, workloadFamily: "sudoku_trace" },
    )
    expect(worker.ok).toBe(true)
    expect(worker.traceDigest).toBe(fixture.expectedTraceDigest)

    const validatorRec = recordingFetch(() => ({ json: { challenge: { state: "Verified" } } }))
    const validator = await submitReplayVerdict(
      { baseUrl: base, agentToken: "tok", fetchFn: validatorRec.fetchFn, executor: executeTassadarNumericModel },
      { leaseRef: "lease.real", validatorDeviceRef: "pylon_validator", workload, workloadFamily: "sudoku_trace" },
    )
    expect(validator.ok).toBe(true)
    // Worker commitment and validator replay digests match -> exact_trace_replay
    // would verify on the server.
    expect(validator.replayDigest).toBe(worker.traceDigest)
  })
})

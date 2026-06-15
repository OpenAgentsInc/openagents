import { describe, expect, test } from "bun:test"
import {
  activateTrainingWindow,
  admitTrainingEvidence,
  claimTrainingLease,
  closeoutTrainingWindow,
  planTrainingWindow,
  reconcileTrainingWindow,
  readTrainingStatus,
} from "../src/training-cockpit"

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

import { describe, expect, test } from "bun:test"
import { assertPublicProjectionSafe } from "../src/state"
import {
  formatPublicActivityCliText,
  parsePublicActivityCliArgs,
  runPublicActivityCliCommand,
} from "../src/public-activity-cli"

type Recorded = { url: string; method: string }

function recordingFetch(responder: (url: string) => unknown) {
  const calls: Recorded[] = []
  const fetchFn = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input)
    calls.push({ url, method: init?.method ?? "GET" })
    return new Response(JSON.stringify(responder(url)), {
      status: 200,
      headers: { "content-type": "application/json" },
    })
  }) as unknown as typeof fetch
  return { fetchFn, calls }
}

const baseUrl = "https://openagents.test"
const nowIso = () => "2026-06-18T12:00:00.000Z"

const timelineFixture = {
  schemaVersion: "openagents.public_activity_timeline.v1",
  generatedAt: "2026-06-18T12:00:00.000Z",
  nextCursor: "cursor.2",
  events: [
    {
      cursor: "cursor.1",
      eventRef: "activity.training.verification.1",
      kind: "verification_verified",
      sourceKind: "training_verification",
      sourceRefs: ["training.verification.challenge.challenge.1"],
      blockerRefs: [],
      ts: "2026-06-18T11:59:00.000Z",
    },
    {
      cursor: "cursor.2",
      eventRef: "activity.training.settlement.1",
      kind: "real_bitcoin_moved",
      sourceKind: "settlement_receipt",
      sourceRefs: ["receipt.training.run.demo.1"],
      blockerRefs: [],
      ts: "2026-06-18T12:00:00.000Z",
    },
  ],
}

const settlementsFixture = {
  schemaVersion: "openagents.public_training_run_settlements.v1",
  settlementRows: [
    {
      trainingRunRef: "training.run.demo",
      receiptRef: "receipt.training.run.demo.1",
      amountSats: 1000,
      realBitcoinMoved: true,
      sourceRefs: ["receipt.training.run.demo.1"],
    },
  ],
}

const runSummaryFixture = {
  schemaVersion: "openagents.public_tassadar_run_summary.v1",
  trainingRunRef: "training.run.demo",
  receiptRefs: ["receipt.training.run.demo.1"],
  verificationChallengeRefs: ["training.verification.challenge.challenge.1"],
}

const challengeFixture = {
  schemaVersion: "openagents.public_verification_challenge.v1",
  challengeRef: "training.verification.challenge.challenge.1",
  sourceRefs: ["training.run.demo"],
}

const proofReplayFixture = {
  schemaVersion: "openagents.proof_replay_bundle.v1",
  replayRef: "first-real-settlement",
  sourceRefs: ["receipt.training.run.demo.1"],
}

const promisesFixture = {
  schemaVersion: "openagents.public_product_promises.v1",
  promises: [
    {
      promiseId: "openagents.training.settlement_visibility.v1",
      status: "live",
    },
  ],
}

describe("public activity CLI", () => {
  test("activity --json fetches filtered public timeline events with dereferenceable refs", async () => {
    const rec = recordingFetch(() => timelineFixture)
    const result = await runPublicActivityCliCommand(
      "activity",
      [
        "--json",
        "--since",
        "cursor.0",
        "--filter",
        "verify",
        "--source",
        "training_verification",
        "--limit",
        "20",
        "--base-url",
        baseUrl,
      ],
      { fetchFn: rec.fetchFn, nowIso },
    )

    expect(result.ok).toBe(true)
    expect(result.authority).toBe("observation_only")
    expect(result.output.kind).toBe("activity")
    expect(rec.calls).toHaveLength(1)
    const url = new URL(rec.calls[0]!.url)
    expect(`${url.origin}${url.pathname}`).toBe(`${baseUrl}/api/public/activity-timeline`)
    expect(url.searchParams.get("since")).toBe("cursor.0")
    expect(url.searchParams.get("limit")).toBe("20")
    expect(url.searchParams.getAll("kind")).toEqual([
      "verification_queued",
      "verification_rejected",
      "verification_verified",
    ])
    expect(url.searchParams.getAll("source")).toEqual(["training_verification"])
    assertPublicProjectionSafe(result)
  })

  test("timeline requires a closed range and includes range params", async () => {
    await expect(
      runPublicActivityCliCommand("timeline", ["--from", "2026-06-18T00:00:00Z"]),
    ).rejects.toThrow(/requires --from and --to/)

    const rec = recordingFetch(() => timelineFixture)
    const result = await runPublicActivityCliCommand(
      "timeline",
      [
        "--json",
        "--from",
        "2026-06-18T00:00:00Z",
        "--to",
        "2026-06-18T23:59:59Z",
        "--base-url",
        baseUrl,
      ],
      { fetchFn: rec.fetchFn, nowIso },
    )
    const url = new URL(rec.calls[0]!.url)
    expect(result.output.kind).toBe("timeline")
    expect(url.searchParams.get("from")).toBe("2026-06-18T00:00:00Z")
    expect(url.searchParams.get("to")).toBe("2026-06-18T23:59:59Z")
  })

  test("receipts requires --run and maps receipt refs to public URLs", async () => {
    await expect(runPublicActivityCliCommand("receipts", ["--json"])).rejects.toThrow(
      /requires --run/,
    )

    const rec = recordingFetch(() => settlementsFixture)
    const result = await runPublicActivityCliCommand(
      "receipts",
      ["--json", "--run", "training.run.demo", "--base-url", baseUrl],
      { fetchFn: rec.fetchFn, nowIso },
    )
    expect(result.output.kind).toBe("receipts")
    if (result.output.kind !== "receipts") throw new Error("unexpected output")
    expect(rec.calls[0]!.url).toBe(
      `${baseUrl}/api/public/training/runs/training.run.demo/settlements`,
    )
    expect(result.output.receiptUrls).toEqual([
      `${baseUrl}/api/public/nexus-pylon/receipts/receipt.training.run.demo.1`,
    ])
    assertPublicProjectionSafe(result)
  })

  test("evidence-pack fetches all public evidence documents and refs", async () => {
    const rec = recordingFetch((url) => {
      if (url.includes("/api/public/tassadar-run-summary")) return runSummaryFixture
      if (url.includes("/api/public/training/runs/")) return settlementsFixture
      if (url.includes("/api/public/product-promises")) return promisesFixture
      if (url.includes("/api/public/proof-replays")) return proofReplayFixture
      if (url.includes("/api/public/training/verification-challenges/")) return challengeFixture
      throw new Error(`unexpected url ${url}`)
    })

    const result = await runPublicActivityCliCommand(
      "evidence-pack",
      [
        "--json",
        "--run",
        "training.run.demo",
        "--challenge-ref",
        "training.verification.challenge.challenge.1",
        "--replay-ref",
        "first-real-settlement",
        "--base-url",
        baseUrl,
      ],
      { fetchFn: rec.fetchFn, nowIso },
    )

    expect(result.output.kind).toBe("evidence-pack")
    if (result.output.kind !== "evidence-pack") throw new Error("unexpected output")
    expect(rec.calls.map((call) => call.url)).toEqual([
      `${baseUrl}/api/public/tassadar-run-summary?run=training.run.demo`,
      `${baseUrl}/api/public/training/runs/training.run.demo/settlements`,
      `${baseUrl}/api/public/product-promises`,
      `${baseUrl}/api/public/proof-replays?ref=first-real-settlement`,
      `${baseUrl}/api/public/training/verification-challenges/training.verification.challenge.challenge.1`,
    ])
    expect(result.output.refs.receiptRefs).toEqual(["receipt.training.run.demo.1"])
    expect(result.output.refs.verificationChallengeUrls).toEqual([
      `${baseUrl}/api/public/training/verification-challenges/training.verification.challenge.challenge.1`,
    ])
    expect(result.output.refs.promiseIds).toEqual([
      "openagents.training.settlement_visibility.v1",
    ])
    assertPublicProjectionSafe(result)
  })

  test("text formatting is useful without JSON parsing", async () => {
    const timelineResult = await runPublicActivityCliCommand(
      "activity",
      ["--base-url", baseUrl],
      { fetchFn: recordingFetch(() => timelineFixture).fetchFn, nowIso },
    )
    expect(formatPublicActivityCliText(timelineResult)).toContain(
      "verification_verified training_verification activity.training.verification.1",
    )

    const receiptsResult = await runPublicActivityCliCommand(
      "receipts",
      ["--run", "training.run.demo", "--base-url", baseUrl],
      { fetchFn: recordingFetch(() => settlementsFixture).fetchFn, nowIso },
    )
    expect(formatPublicActivityCliText(receiptsResult)).toContain(
      "receipt.training.run.demo.1 amount=1000 realBitcoinMoved=true",
    )
  })

  test("watch mode can be bounded for scripts", async () => {
    const rec = recordingFetch(() => timelineFixture)
    await runPublicActivityCliCommand(
      "activity",
      ["--watch", "--max-iterations", "2", "--base-url", baseUrl],
      { fetchFn: rec.fetchFn, nowIso },
    )
    expect(rec.calls).toHaveLength(2)
    expect(new URL(rec.calls[1]!.url).searchParams.get("since")).toBe("cursor.2")
  })

  test("env base URL fallback is accepted", () => {
    const parsed = parsePublicActivityCliArgs("activity", ["--json"], {
      PYLON_OPENAGENTS_BASE_URL: "https://agent.example/",
    })
    expect(parsed.baseUrl).toBe("https://agent.example")
    expect(parsed.json).toBe(true)
  })
})

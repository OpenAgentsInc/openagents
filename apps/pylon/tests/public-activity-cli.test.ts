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

const generatedReplayFixture = {
  bundleRef: "proof_replay_bundle.public_activity.demo",
  schemaVersion: "proof_replay_bundle.v1",
  generatedAt: "2026-06-18T12:01:00.000Z",
  title: "Generated Public Activity Replay",
  socialDisplayTime: "Generated from public activity timeline",
  sourceAuthority: "worker_d1_public",
  privacyLevel: "public_safe",
  claimScope: "evidence_presentation_only",
  sourceRefs: [
    {
      ref: "https://openagents.test/api/public/activity-timeline?from=2026-06-18T00%3A00%3A00Z&to=2026-06-18T01%3A00%3A00Z",
      kind: "api",
      url: "https://openagents.test/api/public/activity-timeline?from=2026-06-18T00%3A00%3A00Z&to=2026-06-18T01%3A00%3A00Z",
      observedAt: "2026-06-18T12:01:00.000Z",
    },
  ],
  staleness: {
    mode: "live_at_read",
    generatedAt: "2026-06-18T12:01:00.000Z",
  },
  generatedFrom: {
    schemaVersion: "openagents.public_activity_generated_replay.v1",
    authority: "evidence_presentation_only",
    caveatRefs: [
      "caveat.public.proof_replay.generated_from_activity_timeline_observation_only",
    ],
    input: {
      actorRefs: ["pylon.demo", "validator.demo"],
      filterKinds: ["verification_verified"],
      filterSources: ["training_verification"],
      from: "2026-06-18T00:00:00Z",
      limit: 50,
      runRefs: ["training.run.demo"],
      since: "cursor.0",
      to: "2026-06-18T01:00:00Z",
      windowRefs: ["training.window.demo"],
    },
    route: "/api/public/proof-replays",
    source: {
      route: "/api/public/activity-timeline",
      url: "https://openagents.test/api/public/activity-timeline?from=2026-06-18T00%3A00%3A00Z&to=2026-06-18T01%3A00%3A00Z",
    },
  },
  actors: [
    {
      actorRef: "pylon.demo",
      avatarRole: "contributor",
      displayName: "Demo Pylon",
    },
  ],
  stages: [
    {
      stageRef: "stage.timeline.verification",
      stageKind: "proof_gate",
      label: "Verification",
      sourceRefs: ["training.verification.challenge.challenge.1"],
    },
  ],
  events: [
    {
      eventRef: "replay.event.1",
      kind: "actor_focused_pylon",
      sequenceIndex: 0,
      timelineSecond: 0,
      observedAt: "2026-06-18T00:00:05.000Z",
      actorRefs: ["pylon.demo"],
      targetRefs: ["stage.timeline.verification"],
      sourceRefs: ["pylon.demo.registration"],
      displayText: "Demo Pylon became assignment ready.",
      caveat: "caveat.public.activity_timeline.source_lag",
      stateAfter: "assignment_ready",
    },
    {
      eventRef: "replay.event.2",
      kind: "proof_verified",
      sequenceIndex: 1,
      timelineSecond: 6,
      observedAt: "2026-06-18T00:00:20.000Z",
      actorRefs: ["validator.demo"],
      targetRefs: ["training.verification.challenge.challenge.1"],
      sourceRefs: ["training.verification.challenge.challenge.1"],
      displayText: "Validator verified the replay digest.",
    },
  ],
  flows: [
    {
      flowRef: "flow.demo.1",
      flowKind: "fleet_readiness_track",
      fromRef: "pylon.demo",
      toRef: "stage.timeline.fleet",
      sourceRefs: ["pylon.demo.registration"],
    },
  ],
  cameraCues: [],
  captions: [
    {
      captionRef: "caption.demo.1",
      sequenceIndex: 0,
      timelineSecond: 0,
      text: "Fleet readiness event",
      sourceRefs: ["pylon.demo.registration"],
    },
    {
      captionRef: "caption.demo.2",
      sequenceIndex: 1,
      timelineSecond: 6,
      text: "Verification event",
      sourceRefs: ["training.verification.challenge.challenge.1"],
    },
  ],
  gaps: [
    {
      gapRef: "gap.source_lag.training_verification",
      reason: "Training verification source lagged.",
      affectedRefs: ["training_verification"],
      sourceRefs: ["blocker.public.activity_timeline.source_lag.training_verification"],
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

  test("replay fetches a generated public activity bundle and projects JSON event track", async () => {
    const rec = recordingFetch((url) => {
      if (url.includes("/api/public/proof-replays")) return generatedReplayFixture
      throw new Error(`unexpected url ${url}`)
    })

    const result = await runPublicActivityCliCommand(
      "replay",
      [
        "--format",
        "json",
        "--from",
        "2026-06-18T00:00:00Z",
        "--to",
        "2026-06-18T01:00:00Z",
        "--run",
        "training.run.demo",
        "--window",
        "training.window.demo",
        "--pair",
        "pylon.demo:validator.demo",
        "--kind",
        "verification_verified",
        "--source",
        "training_verification",
        "--since",
        "cursor.0",
        "--limit",
        "50",
        "--base-url",
        baseUrl,
      ],
      { fetchFn: rec.fetchFn, nowIso },
    )

    expect(result.json).toBe(true)
    expect(result.output.kind).toBe("replay")
    if (result.output.kind !== "replay") throw new Error("unexpected output")
    const url = new URL(rec.calls[0]!.url)
    expect(`${url.origin}${url.pathname}`).toBe(`${baseUrl}/api/public/proof-replays`)
    expect(url.searchParams.get("mode")).toBe("activity-timeline")
    expect(url.searchParams.get("from")).toBe("2026-06-18T00:00:00Z")
    expect(url.searchParams.get("to")).toBe("2026-06-18T01:00:00Z")
    expect(url.searchParams.get("runRef")).toBe("training.run.demo")
    expect(url.searchParams.get("windowRef")).toBe("training.window.demo")
    expect(url.searchParams.getAll("actorRef")).toEqual([
      "pylon.demo",
      "validator.demo",
    ])
    expect(url.searchParams.getAll("kind")).toEqual(["verification_verified"])
    expect(url.searchParams.getAll("source")).toEqual(["training_verification"])
    expect(url.searchParams.get("since")).toBe("cursor.0")
    expect(url.searchParams.get("limit")).toBe("50")
    expect(result.output.eventTrack).toEqual({
      schema: "openagents.pylon.public_replay_event_track.v1",
      bundleRef: "proof_replay_bundle.public_activity.demo",
      title: "Generated Public Activity Replay",
      generatedAt: "2026-06-18T12:01:00.000Z",
      sourceAuthority: "worker_d1_public",
      privacyLevel: "public_safe",
      claimScope: "evidence_presentation_only",
      generatedFrom: generatedReplayFixture.generatedFrom,
      staleness: generatedReplayFixture.staleness,
      sourceRefs: generatedReplayFixture.sourceRefs,
      events: [
        {
          sequenceIndex: 0,
          eventRef: "replay.event.1",
          kind: "actor_focused_pylon",
          timelineSecond: 0,
          timestamp: "2026-06-18T00:00:05.000Z",
          displayText: "Demo Pylon became assignment ready.",
          actorRefs: ["pylon.demo"],
          targetRefs: ["stage.timeline.verification"],
          sourceRefs: ["pylon.demo.registration"],
          caveatRefs: ["caveat.public.activity_timeline.source_lag"],
          captions: ["Fleet readiness event"],
          amountSats: null,
          rail: null,
          stateBefore: null,
          stateAfter: "assignment_ready",
        },
        {
          sequenceIndex: 1,
          eventRef: "replay.event.2",
          kind: "proof_verified",
          timelineSecond: 6,
          timestamp: "2026-06-18T00:00:20.000Z",
          displayText: "Validator verified the replay digest.",
          actorRefs: ["validator.demo"],
          targetRefs: ["training.verification.challenge.challenge.1"],
          sourceRefs: ["training.verification.challenge.challenge.1"],
          caveatRefs: [],
          captions: ["Verification event"],
          amountSats: null,
          rail: null,
          stateBefore: null,
          stateAfter: null,
        },
      ],
      gaps: [
        {
          gapRef: "gap.source_lag.training_verification",
          reason: "Training verification source lagged.",
          affectedRefs: ["training_verification"],
          sourceRefs: ["blocker.public.activity_timeline.source_lag.training_verification"],
        },
      ],
      captions: [
        {
          captionRef: "caption.demo.1",
          sequenceIndex: 0,
          timelineSecond: 0,
          text: "Fleet readiness event",
          sourceRefs: ["pylon.demo.registration"],
        },
        {
          captionRef: "caption.demo.2",
          sequenceIndex: 1,
          timelineSecond: 6,
          text: "Verification event",
          sourceRefs: ["training.verification.challenge.challenge.1"],
        },
      ],
    })
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

    const replayResult = await runPublicActivityCliCommand(
      "replay",
      [
        "--from",
        "2026-06-18T00:00:00Z",
        "--to",
        "2026-06-18T01:00:00Z",
        "--base-url",
        baseUrl,
      ],
      { fetchFn: recordingFetch(() => generatedReplayFixture).fetchFn, nowIso },
    )
    const replayText = formatPublicActivityCliText(replayResult)
    expect(replayText).toContain(
      "2026-06-18T00:00:05.000Z +0s #0 actor_focused_pylon replay.event.1",
    )
    expect(replayText).toContain("refs=pylon.demo.registration")
    expect(replayText).toContain("caveats=caveat.public.activity_timeline.source_lag")
    expect(replayText).toContain("captions=Fleet readiness event")
    expect(replayText).toContain(
      "gap gap.source_lag.training_verification: Training verification source lagged.",
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

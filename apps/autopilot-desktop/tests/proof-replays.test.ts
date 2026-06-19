import { describe, expect, test } from "bun:test"
import {
  FIRST_REAL_SETTLEMENT_REPLAY_SLUG,
  LAUNCH_RECOGNITION_REPLAY_SLUG,
  type ProofReplayBundle,
} from "@openagentsinc/proof-replay"

import {
  desktopProofReplayCatalog,
  generatedProofReplayBundleEndpoint,
  loadDesktopProofReplayProjection,
} from "../src/shared/proof-replays"

const bundle = {
  actors: [
    {
      actorRef: "actor.worker",
      avatarRole: "contributor",
      displayName: "Contributor",
      pylonRef: "pylon.public",
    },
    {
      actorRef: "actor.treasury",
      avatarRole: "settlement_terminal",
      displayName: "Treasury",
    },
  ],
  bundleRef: "proof_replay_bundle.test.desktop",
  cameraCues: [
    {
      cueRef: "cue.overview",
      durationSecond: 12,
      focusRefs: ["stage.run"],
      mode: "overview",
      sourceRefs: ["run.test"],
      startSecond: 0,
    },
  ],
  captions: [
    {
      captionRef: "caption.title",
      sequenceIndex: 0,
      sourceRefs: ["run.test"],
      text: "Desktop replay",
      timelineSecond: 0,
    },
  ],
  claimScope: "evidence_presentation_only",
  events: [
    {
      actorRefs: ["actor.worker"],
      displayText: "Verified challenge",
      eventRef: "event.verified",
      kind: "proof_verified",
      sequenceIndex: 0,
      sourceRefs: ["challenge.test"],
      targetRefs: ["stage.proof"],
      timelineSecond: 3,
    },
    {
      actorRefs: ["actor.treasury"],
      amountSats: 1000,
      displayText: "1,000 sats settled",
      eventRef: "event.zap",
      kind: "payment_zap_confirmed",
      rail: "spark_treasury",
      sequenceIndex: 1,
      sourceRefs: ["receipt.test.real"],
      targetRefs: ["actor.worker"],
      timelineSecond: 10,
    },
  ],
  flows: [
    {
      amountSats: 1000,
      flowKind: "payment_movement",
      flowRef: "flow.payment",
      fromRef: "actor.treasury",
      rail: "spark_treasury",
      sourceRefs: ["receipt.test.real"],
      toRef: "actor.worker",
    },
  ],
  gaps: [],
  generatedAt: "2026-06-18T02:38:00.000Z",
  privacyLevel: "public_safe",
  schemaVersion: "proof_replay_bundle.v1",
  sourceAuthority: "worker_d1_public",
  sourceRefs: [
    { kind: "run", ref: "run.test" },
    { kind: "challenge", ref: "challenge.test" },
    { kind: "receipt", ref: "receipt.test.real" },
  ],
  stages: [
    {
      label: "Tassadar",
      sourceRefs: ["run.test"],
      stageKind: "run_core",
      stageRef: "stage.run",
    },
    {
      label: "Proof gate",
      sourceRefs: ["challenge.test"],
      stageKind: "proof_gate",
      stageRef: "stage.proof",
    },
  ],
  title: "Tassadar Run 1",
} satisfies ProofReplayBundle

const jsonResponse = (payload: unknown, status = 200): Response =>
  new Response(JSON.stringify(payload), {
    headers: { "content-type": "application/json" },
    status,
  })

describe("desktop proof replays", () => {
  test("catalog exposes first settlement, recognition, and social handoff URLs", () => {
    const catalog = desktopProofReplayCatalog("https://openagents.com")

    expect(catalog.map(entry => entry.slug)).toEqual([
      FIRST_REAL_SETTLEMENT_REPLAY_SLUG,
      LAUNCH_RECOGNITION_REPLAY_SLUG,
    ])
    expect(catalog[0]?.websitePath).toBe(
      "https://openagents.com/tassadar/replay/first-real-settlement",
    )
    expect(catalog[0]?.socialPath).toBe(
      "https://openagents.com/tassadar/replay/first-real-settlement?camera=social&duration=60&hud=social",
    )
    expect(catalog[1]?.bundleEndpoint).toBe(
      "https://openagents.com/api/public/proof-replays?ref=launch-recognition-payments",
    )
  })

  test("loads and summarizes a public-safe replay bundle over the shared endpoint", async () => {
    const calls: Array<string> = []
    const projection = await loadDesktopProofReplayProjection(
      FIRST_REAL_SETTLEMENT_REPLAY_SLUG,
      {
        fetcher: async input => {
          calls.push(String(input))
          return jsonResponse(bundle)
        },
      },
    )

    expect(calls).toEqual([
      "https://openagents.com/api/public/tassadar-replays/first-real-settlement",
    ])
    expect(projection.ok).toBe(true)
    expect(projection.cacheState).toBe("live_https")
    expect(projection.summary?.confirmedZapSats).toBe(1000)
    expect(projection.summary?.eventCount).toBe(2)
  })

  test("loads generated public activity replay bundles with bounded filters", async () => {
    const generatedBundle = {
      ...bundle,
      bundleRef: "proof_replay_bundle.public_activity.test",
      generatedFrom: {
        caveatRefs: [
          "caveat.public.proof_replay.generated_from_activity_timeline_observation_only",
        ],
        input: {
          actorRefs: ["pylon.448ba824b5fc879f3a59", "pylon.treasury"],
          filterKinds: ["real_bitcoin_moved"],
          filterSources: ["settlement_receipt"],
          from: "2026-06-18T12:00:00.000Z",
          limit: 10,
          runRefs: ["run.tassadar.executor.20260615"],
          since: "2026-06-18T12:00:00.000Z:settlement_receipt:event.1",
          to: "2026-06-18T12:05:00.000Z",
          windowRefs: ["training.window.tassadar.executor.20260615.w1"],
        },
        source: {
          route: "/api/public/activity-timeline",
          url: "https://openagents.com/api/public/activity-timeline?from=2026-06-18T12%3A00%3A00.000Z&to=2026-06-18T12%3A05%3A00.000Z",
        },
        sourceLag: [{ sourceKind: "forum", status: "stale" }],
      },
      gaps: [
        {
          affectedRefs: ["forum"],
          gapRef: "gap.source_lag.1.forum",
          reason: "Public activity source forum is stale",
          sourceRefs: [
            "caveat.public_activity_timeline.source_lag.forum",
          ],
        },
      ],
      title: "Generated Public Activity Replay",
    } satisfies ProofReplayBundle & {
      generatedFrom: Record<string, unknown>
    }
    const filters = {
      actorRef: "pylon.448ba824b5fc879f3a59",
      from: "2026-06-18T12:00:00.000Z",
      kind: "real_bitcoin_moved",
      limit: 10,
      pairRef: "pylon.448ba824b5fc879f3a59+pylon.treasury",
      runRef: "run.tassadar.executor.20260615",
      since: "2026-06-18T12:00:00.000Z:settlement_receipt:event.1",
      source: "settlement_receipt",
      to: "2026-06-18T12:05:00.000Z",
      windowRef: "training.window.tassadar.executor.20260615.w1",
    }
    const calls: Array<string> = []
    const projection = await loadDesktopProofReplayProjection(
      { mode: "generated", filters },
      {
        fetcher: async input => {
          calls.push(String(input))
          return jsonResponse(generatedBundle)
        },
      },
    )

    expect(calls).toEqual([generatedProofReplayBundleEndpoint(filters)])
    const calledUrl = new URL(calls[0] ?? "")
    expect(calledUrl.searchParams.getAll("actorRef")).toEqual([
      "pylon.448ba824b5fc879f3a59",
      "pylon.treasury",
    ])
    expect(calledUrl.searchParams.get("source")).toBe("settlement_receipt")
    expect(calledUrl.searchParams.get("since")).toBe(
      "2026-06-18T12:00:00.000Z:settlement_receipt:event.1",
    )
    expect(projection.ok).toBe(true)
    expect(projection.entry).toBe(null)
    expect(projection.request.mode).toBe("generated")
    expect(projection.sourceUrl).toContain("mode=activity-timeline")
    expect(projection.filterLabel).toContain("run.tassadar.executor.20260615")
    expect(projection.generatedFrom?.input?.filterKinds).toEqual([
      "real_bitcoin_moved",
    ])
    expect(projection.generatedFrom?.input?.filterSources).toEqual([
      "settlement_receipt",
    ])
    expect(projection.generatedFrom?.sourceLag).toHaveLength(1)
    expect(projection.caveatRefs).toEqual(
      expect.arrayContaining([
        "caveat.public.proof_replay.generated_from_activity_timeline_observation_only",
        "caveat.public_activity_timeline.source_lag.forum",
      ]),
    )
  })

  test("fails generated replay requests closed without a valid bounded range", async () => {
    const projection = await loadDesktopProofReplayProjection({
      filters: {
        from: "",
        to: "",
      },
      mode: "generated",
    })

    expect(projection.ok).toBe(false)
    expect(projection.bundle).toBe(null)
    expect(projection.blockerRefs).toContain(
      "desktop.proof_replay.generated_range_required",
    )
    expect(projection.sourceUrl).toContain("mode=activity-timeline")
  })

  test("blocks unsafe replay material before desktop rendering", async () => {
    const projection = await loadDesktopProofReplayProjection(
      FIRST_REAL_SETTLEMENT_REPLAY_SLUG,
      {
        fetcher: async () =>
          jsonResponse({
            ...bundle,
            captions: [
              {
                captionRef: "caption.private",
                sequenceIndex: 0,
                sourceRefs: ["source.private"],
                text: "payment_hash should not render",
                timelineSecond: 0,
              },
            ],
          }),
      },
    )

    expect(projection.ok).toBe(false)
    expect(projection.bundle).toBe(null)
    expect(projection.blockerRefs).toContain(
      "desktop.proof_replay.shipment_gate_blocked",
    )
    expect(projection.cacheLabel).toContain("no offline snapshot")
  })
})

import { describe, expect, test } from "bun:test"

import type { TrainingRunsResponse } from "../src/shared/rpc"
import {
  VERSE_TASSADAR_BULLETIN_ITEM_ID,
  verseTassadarBulletinOverlayProjection,
  verseTassadarBulletinWorldItem,
  withVerseBulletinBoardLayer,
} from "../src/shared/verse-bulletin-board"
import { initialModel } from "../src/ui/model"
import { ChangedVerseWorldItemProximity } from "../src/ui/message"
import { update } from "../src/ui/update"

const projection = (): TrainingRunsResponse => ({
  fetchedAt: "2026-06-21T17:10:00.000Z",
  ok: true,
  runs: [],
  sourceUrl: "https://openagents.test/api/training/runs",
  summaries: [],
  tassadarSummary: {
    generatedAt: "2026-06-21T17:10:00.000Z",
    runRef: "run.tassadar.executor.20260615",
    runState: "active",
    bulletin: {
      title: "Tassadar Run Board",
      headline: "Tassadar is active: 5 pylons, 2 active.",
      summary:
        "Tassadar is active with public training windows and verified work.",
      statusLine: "active · 5 pylons, 2 active",
      onBoardLines: ["Status: active", "5 pylons, 2 active", "2,100 sats paid"],
      metrics: {
        acceptedTraceCount: 1,
        activePylonCount: 2,
        activeWindowCount: 2,
        realSettlementCount: 1,
        settledSats: 2100,
        totalPylonCount: 5,
        verifiedWorkCount: 9,
      },
      latestActivity: [{
        label: "latest update",
        text: "One verified replay pair was accepted.",
      }],
      sourceRefs: ["run.tassadar.executor.20260615"],
    },
  },
})

describe("Verse Tassadar bulletin board", () => {
  test("projects server-owned bulletin copy into a walk-up world item", () => {
    const item = verseTassadarBulletinWorldItem(projection())

    expect(item).toMatchObject({
      id: VERSE_TASSADAR_BULLETIN_ITEM_ID,
      kind: "bulletin_board",
      label: "Tassadar Run Board",
      status: "active",
      position: [-0.95, 1.78, 0.04],
      yaw: -0.04,
      interactionRadius: 3.8,
    })
    expect(item?.lines).toEqual([
      "Status: active",
      "5 pylons, 2 active",
      "2,100 sats paid",
    ])
    expect(item?.sourceRefs).toContain("route:/api/public/tassadar-run-summary")
  })

  test("renders a physical board before the public summary fetch completes", () => {
    const item = verseTassadarBulletinWorldItem(null)

    expect(item).toMatchObject({
      id: VERSE_TASSADAR_BULLETIN_ITEM_ID,
      kind: "bulletin_board",
      label: "Tassadar Board",
      status: "queued",
      position: [-0.95, 1.78, 0.04],
    })
    expect(item.lines).toEqual(["Loading Tassadar run"])
    expect(item.sourceRefs).toContain("route:/api/public/tassadar-run-summary")
  })

  test("adds the board to existing Verse visualization options", () => {
    const out = withVerseBulletinBoardLayer({ nodes: [] }, null)

    expect(out.worldItems?.map(item => item.id)).toEqual([
      VERSE_TASSADAR_BULLETIN_ITEM_ID,
    ])
  })

  test("opens overlay projection only while the player is near the board", () => {
    expect(verseTassadarBulletinOverlayProjection(projection(), null)).toBeNull()

    const overlay = verseTassadarBulletinOverlayProjection(
      projection(),
      VERSE_TASSADAR_BULLETIN_ITEM_ID,
    )

    expect(overlay?.headline).toBe("Tassadar is active: 5 pylons, 2 active.")
    expect(overlay?.metrics.find(metric => metric.label === "sats")?.value).toBe(
      "2,100",
    )
    expect(overlay?.latestActivity[0]?.text).toContain("verified replay")
  })

  test("stores and clears Verse world-item proximity in the Foldkit model", () => {
    const [near] = update(
      initialModel,
      ChangedVerseWorldItemProximity({ itemId: VERSE_TASSADAR_BULLETIN_ITEM_ID }),
    )
    expect(near.nearVerseWorldItemId).toBe(VERSE_TASSADAR_BULLETIN_ITEM_ID)

    const [away] = update(
      near,
      ChangedVerseWorldItemProximity({ itemId: null }),
    )
    expect(away.nearVerseWorldItemId).toBeNull()
  })
})

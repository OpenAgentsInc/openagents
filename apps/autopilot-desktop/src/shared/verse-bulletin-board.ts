import type {
  TrainingRunNodeStatus,
  TrainingRunVisualizationOptions,
  TrainingRunWorldItemDefinition,
} from "@openagentsinc/three-effect/core"

import type {
  PublicTassadarRunSummary,
  TassadarRunBulletin,
  TrainingRunsResponse,
} from "./rpc.js"

export const VERSE_TASSADAR_BULLETIN_ITEM_ID = "verse:bulletin:tassadar-run"

export type VerseBulletinMetric = Readonly<{
  label: string
  value: string
}>

export type VerseBulletinActivity = Readonly<{
  label: string
  text: string
}>

export type VerseBulletinOverlayProjection = Readonly<{
  headline: string
  metrics: readonly VerseBulletinMetric[]
  summary: string
  title: string
  latestActivity: readonly VerseBulletinActivity[]
}>

const compact = (value: string | undefined, fallback: string): string => {
  const text = value?.trim()
  return text === undefined || text.length === 0 ? fallback : text
}

const lines = (values: ReadonlyArray<string | undefined>): readonly string[] =>
  values
    .map(value => value?.trim() ?? "")
    .filter((value, index, all) => value.length > 0 && all.indexOf(value) === index)

const numberText = (value: number | undefined): string =>
  new Intl.NumberFormat("en-US").format(
    typeof value === "number" && Number.isFinite(value) ? value : 0,
  )

const statusFromRunState = (
  runState: string | undefined,
): TrainingRunNodeStatus => {
  switch (runState) {
    case "active":
      return "active"
    case "reconciled":
      return "verified"
    case "sealed":
      return "sealed"
    case "blocked":
      return "blocked"
    case "planned":
      return "planned"
    default:
      return "queued"
  }
}

export const tassadarSummaryForVerse = (
  projection: TrainingRunsResponse | null,
): PublicTassadarRunSummary | null =>
  projection?.tassadarSummary === undefined
    ? null
    : projection.tassadarSummary

export const verseTassadarBulletinWorldItem = (
  projection: TrainingRunsResponse | null,
): TrainingRunWorldItemDefinition | null => {
  const summary = tassadarSummaryForVerse(projection)
  const bulletin = summary?.bulletin
  if (summary === null || bulletin === undefined) return null

  const title = compact(bulletin.title, "Tassadar Run Board")
  const headline = compact(bulletin.headline, "Tassadar status")
  const detail = compact(bulletin.summary, headline)
  const onBoardLines = lines([
    ...(bulletin.onBoardLines ?? []),
    bulletin.statusLine,
    headline,
  ]).slice(0, 3)

  return {
    id: VERSE_TASSADAR_BULLETIN_ITEM_ID,
    kind: "bulletin_board",
    label: title,
    title,
    detail,
    lines: onBoardLines.length > 0 ? onBoardLines : [headline],
    // Left edge of the Tassadar lot: visible from spawn, but still something the
    // player walks up to rather than a HUD-only widget.
    position: [-3.35, 0.82, 0.05],
    yaw: 0.42,
    interactionRadius: 3.2,
    status: statusFromRunState(summary.runState),
    sourceRefs: lines([
      summary.runRef,
      ...(bulletin.sourceRefs ?? []),
      "route:/api/public/tassadar-run-summary",
    ]),
  }
}

export const withVerseBulletinBoardLayer = (
  base: TrainingRunVisualizationOptions,
  projection: TrainingRunsResponse | null,
): TrainingRunVisualizationOptions => {
  const item = verseTassadarBulletinWorldItem(projection)
  if (item === null) return base
  return {
    ...base,
    worldItems: [...(base.worldItems ?? []), item],
  }
}

const metricRows = (
  bulletin: TassadarRunBulletin,
): readonly VerseBulletinMetric[] => {
  const metrics = bulletin.metrics
  if (metrics === undefined) return []
  return [
    ["pylons", metrics.totalPylonCount],
    ["active", metrics.activePylonCount],
    ["sats", metrics.settledSats],
    ["windows", metrics.activeWindowCount],
    ["traces", metrics.acceptedTraceCount],
    ["verified", metrics.verifiedWorkCount],
  ].map(([label, value]) => ({
    label: String(label),
    value: numberText(value as number | undefined),
  }))
}

export const verseTassadarBulletinOverlayProjection = (
  projection: TrainingRunsResponse | null,
  nearWorldItemId: string | null,
): VerseBulletinOverlayProjection | null => {
  if (nearWorldItemId !== VERSE_TASSADAR_BULLETIN_ITEM_ID) return null
  const bulletin = tassadarSummaryForVerse(projection)?.bulletin
  if (bulletin === undefined) return null
  return {
    title: compact(bulletin.title, "Tassadar Run Board"),
    headline: compact(bulletin.headline, "Tassadar status"),
    summary: compact(bulletin.summary, "No bulletin copy is published yet."),
    metrics: metricRows(bulletin),
    latestActivity: (bulletin.latestActivity ?? [])
      .map(item => ({
        label: compact(item.label, "latest"),
        text: compact(item.text, "No activity text published."),
      }))
      .slice(0, 3),
  }
}

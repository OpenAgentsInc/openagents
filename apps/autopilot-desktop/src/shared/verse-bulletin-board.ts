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
import {
  appendVerseVisualization,
  compactVerseLines,
} from "./verse-scene-helpers.js"

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

const numberText = (value: number | undefined): string =>
  new Intl.NumberFormat("en-US").format(
    typeof value === "number" && Number.isFinite(value) ? value : 0,
  )

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null

const metricValue = (
  summary: PublicTassadarRunSummary | null,
  keys: readonly string[],
): number | undefined => {
  const metrics = isRecord(summary?.metrics) ? summary.metrics : {}
  for (const key of keys) {
    const raw = metrics[key]
    if (typeof raw === "number" && Number.isFinite(raw)) return raw
    if (isRecord(raw) && typeof raw.value === "number" && Number.isFinite(raw.value)) {
      return raw.value
    }
  }
  return undefined
}

const synthesizedBulletin = (
  summary: PublicTassadarRunSummary | null,
): TassadarRunBulletin | undefined => {
  if (summary === null) return undefined
  if (summary.bulletin !== undefined) return summary.bulletin
  if (summary.runRef === undefined && summary.runState === undefined) return undefined

  const status = compact(summary.runState, "planned")
  const totalPylons = metricValue(summary, [
    "totalPylonCount",
    "assignedContributorCount",
    "qualifiedContributorCount",
  ]) ?? 0
  const activePylons = metricValue(summary, [
    "activePylonCount",
    "qualifiedContributorCount",
    "activeWindowCount",
  ]) ?? 0
  const activeWindows = metricValue(summary, ["activeWindowCount"]) ?? 0
  const settledSats = metricValue(summary, [
    "settledSats",
    "providerConfirmedSettledPayoutSats",
  ]) ?? 0
  const verifiedWork = metricValue(summary, ["verifiedWorkCount"]) ?? 0
  const acceptedTraceCount = metricValue(summary, [
    "acceptedTraceCount",
    "verifiedWorkCount",
  ]) ?? 0
  const pylonLine = `${numberText(totalPylons)} pylons, ${numberText(activePylons)} active`
  const windowLine =
    activeWindows > 0
      ? `${numberText(activeWindows)} active windows are visible.`
      : "No active window is visible."
  const workLine =
    verifiedWork > 0
      ? `${numberText(verifiedWork)} verified work items have landed.`
      : "Verified work is pending."
  const settlementLine =
    settledSats > 0
      ? `${numberText(settledSats)} sats paid`
      : "settlement pending"

  return {
    title: "Tassadar Run Board",
    headline: `Tassadar is ${status}: ${pylonLine}.`,
    summary: [
      `Tassadar is ${status}.`,
      windowLine,
      workLine,
      settledSats > 0 ? `${settlementLine}.` : "Settlement is pending.",
    ].join(" "),
    statusLine: `${status} · ${pylonLine}`,
    onBoardLines: [`Status: ${status}`, pylonLine, settlementLine],
    metrics: {
      acceptedTraceCount,
      activePylonCount: activePylons,
      activeWindowCount: activeWindows,
      realSettlementCount: settledSats > 0 ? 1 : 0,
      settledSats,
      totalPylonCount: totalPylons,
      verifiedWorkCount: verifiedWork,
    },
    latestActivity: [
      {
        label: "latest update",
        text:
          summary.generatedAt === undefined
            ? "The public Tassadar run summary is live."
            : `Latest public summary was generated at ${summary.generatedAt}.`,
      },
    ],
    sourceRefs: compactVerseLines([
      summary.runRef,
      ...(summary.sourceRefs ?? []),
      "route:/api/public/tassadar-run-summary",
    ]),
  }
}

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

const bulletinBoardPosition = [-0.95, 1.78, 0.04] as const
const bulletinBoardYaw = -0.04
const bulletinBoardInteractionRadius = 3.8

export const verseTassadarBulletinWorldItem = (
  projection: TrainingRunsResponse | null,
): TrainingRunWorldItemDefinition => {
  const summary = tassadarSummaryForVerse(projection)
  const bulletin = synthesizedBulletin(summary)
  const title = compact(bulletin?.title, "Tassadar Board")
  const headline = compact(bulletin?.headline, "Loading Tassadar run")
  const detail = compact(
    bulletin?.summary,
    "Waiting for the public Tassadar run summary from openagents.com.",
  )
  const onBoardLines = compactVerseLines([
    ...(bulletin?.onBoardLines ?? []),
    bulletin?.statusLine,
    headline,
  ]).slice(0, 3)

  return {
    id: VERSE_TASSADAR_BULLETIN_ITEM_ID,
    kind: "bulletin_board",
    label: title,
    title,
    detail,
    lines: onBoardLines.length > 0 ? onBoardLines : [headline],
    // In the first-render camera lane: this must read as a physical board
    // before any public summary fetch completes.
    position: bulletinBoardPosition,
    yaw: bulletinBoardYaw,
    interactionRadius: bulletinBoardInteractionRadius,
    status: statusFromRunState(summary?.runState),
    sourceRefs: compactVerseLines([
      summary?.runRef,
      ...(bulletin?.sourceRefs ?? []),
      "route:/api/public/tassadar-run-summary",
    ]),
  }
}

export const withVerseBulletinBoardLayer = (
  base: TrainingRunVisualizationOptions,
  projection: TrainingRunsResponse | null,
): TrainingRunVisualizationOptions => {
  const item = verseTassadarBulletinWorldItem(projection)
  return appendVerseVisualization(base, { worldItems: [item] })
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
  const bulletin = synthesizedBulletin(tassadarSummaryForVerse(projection))
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

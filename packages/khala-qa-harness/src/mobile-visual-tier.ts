import { mkdir, writeFile } from "node:fs/promises"
import { join } from "node:path"

import {
  evaluateKhalaVisualBaseline,
  type KhalaVisualBaselineResult,
} from "./visual-baseline.js"

export type KhalaMobileVisualTierCapture = Readonly<{
  colorScheme: "dark" | "light"
  device: string
  id: string
  png: Buffer
  source: "fixture" | "ios-simulator-storybook" | "maestro-checkpoint"
  storyId?: string
}>

export type KhalaMobileVisualTierBlessing = Readonly<{
  captureId: string
  reason: string
  reviewedBy: string
}>

export type KhalaMobileVisualTierReport = Readonly<{
  baselineDir: string
  blessings: readonly KhalaMobileVisualTierBlessing[]
  changed: readonly KhalaVisualBaselineResult[]
  generatedAt: string
  ok: boolean
  results: readonly KhalaVisualBaselineResult[]
  schema: "openagents.khala_mobile.visual_tier_report.v1"
  simulatorTruth: "not_claimed" | "captured"
}>

export type KhalaMobileVisualTierInput = Readonly<{
  baselineDir: string
  bless?: boolean
  blessings?: readonly KhalaMobileVisualTierBlessing[]
  captures: readonly KhalaMobileVisualTierCapture[]
  candidateDir: string
  now?: () => string
  requireBaseline?: boolean
  simulatorTruth?: "not_claimed" | "captured"
}>

export async function runKhalaMobileVisualTier(
  input: KhalaMobileVisualTierInput,
): Promise<KhalaMobileVisualTierReport> {
  const results: KhalaVisualBaselineResult[] = []
  const blessingByCaptureId = new Map(
    (input.blessings ?? []).map(blessing => [blessing.captureId, blessing]),
  )

  await mkdir(input.candidateDir, { recursive: true })

  for (const capture of input.captures) {
    const screenshotPath = join(input.candidateDir, `${capture.id}.png`)
    await writeFile(screenshotPath, capture.png)
    const hasBlessing = blessingByCaptureId.has(capture.id)
    const baselineInput = {
      baselineDir: input.baselineDir,
      bless: input.bless === true || hasBlessing,
      capture: {
        colorScheme: capture.colorScheme,
        harness: capture.source,
        id: capture.id,
        reducedMotion: "no-preference" as const,
        screenshotPath,
        viewport: capture.device,
      },
      ...(input.now === undefined ? {} : { now: input.now }),
      ...(input.requireBaseline === undefined ? {} : { requireBaseline: input.requireBaseline }),
    }

    results.push(
      await evaluateKhalaVisualBaseline(baselineInput),
    )
  }

  const changed = results.filter(result => result.status === "changed")
  return {
    baselineDir: input.baselineDir,
    blessings: input.blessings ?? [],
    changed,
    generatedAt: input.now?.() ?? new Date().toISOString(),
    ok: results.every(result => result.ok),
    results,
    schema: "openagents.khala_mobile.visual_tier_report.v1",
    simulatorTruth: input.simulatorTruth ?? "not_claimed",
  }
}

export const khalaMobileVisualTierStarterCaptures = (): readonly KhalaMobileVisualTierCapture[] => [
  {
    colorScheme: "dark",
    device: "iphone-17-pro-max",
    id: "khala.mobile.story.primitives.empty-state.loading.dark",
    png: Buffer.from(
      "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
      "base64",
    ),
    source: "fixture",
    storyId: "khala-primitives-emptystate--loading",
  },
  {
    colorScheme: "dark",
    device: "iphone-17-pro-max",
    id: "khala.mobile.story.components.button.filled.dark",
    png: Buffer.from(
      "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNgYAAAAAMAASsJTYQAAAAASUVORK5CYII=",
      "base64",
    ),
    source: "fixture",
    storyId: "khala-components-button--filled",
  },
]

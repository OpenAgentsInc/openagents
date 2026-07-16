/**
 * QA-3 (#8908): the shared contract between the main-process visual-baseline
 * probe (main.ts), the renderer fixture mount
 * (renderer/visual-baseline-fixtures.ts), and the gate script
 * (scripts/visual-baseline-smoke.ts). Deliberately dependency-free: main
 * must be able to import the state list without pulling any renderer module
 * into its bundle.
 */

/** Fixed capture geometry: the probe window and every baseline share it. */
export const VISUAL_BASELINE_WINDOW = { width: 1280, height: 800 } as const

/** Forced Chromium device scale for the probe (Retina-independent pixels). */
export const VISUAL_BASELINE_DEVICE_SCALE_FACTOR = 1

/** The fixed shell-state capture set, in capture order. */
export const VISUAL_BASELINE_STATES = [
  "composer-idle",
  "thread-plan-card",
  "approval-card",
  "reasoning-disclosure",
  "full-auto-running",
] as const
export type VisualBaselineStateName = (typeof VISUAL_BASELINE_STATES)[number]

export const isVisualBaselineStateName = (value: string): value is VisualBaselineStateName =>
  (VISUAL_BASELINE_STATES as ReadonlyArray<string>).includes(value)

/** One captured state as reported in the probe's public-safe receipt line. */
export type VisualBaselineCaptureReceipt = Readonly<{
  state: VisualBaselineStateName
  file: string
  sha256: string
  width: number
  height: number
}>

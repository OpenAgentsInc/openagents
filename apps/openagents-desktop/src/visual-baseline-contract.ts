/**
 * QA-3 (#8908): the shared contract between the main-process visual-baseline
 * probe (main.ts), the renderer fixture mount
 * (renderer/visual-baseline-fixtures.ts), and the gate script
 * (scripts/visual-baseline-smoke.ts). Deliberately dependency-free: main
 * must be able to import the state list without pulling any renderer module
 * into its bundle.
 */

/** Fixed capture geometry: the probe window and every baseline share it. */
export const VISUAL_BASELINE_WINDOW = { width: 1280, height: 800 } as const;

export const visualBaselineViewportForState = (state: string): Readonly<{ width: number; height: number }> =>
  state === "responsive-standard" ? { width: 900, height: 760 }
    : state === "responsive-minimum" ? { width: 480, height: 720 }
      : VISUAL_BASELINE_WINDOW;

/** Forced Chromium device scale for the probe (Retina-independent pixels). */
export const VISUAL_BASELINE_DEVICE_SCALE_FACTOR = 1;

/** Stable stdout receipt identity consumed by the QA-1 Desktop lane. */
export const QA_DESKTOP_VISUAL_RECEIPT_SCHEMA = "openagents.qa.desktop-visual-lane.v1" as const;
export const QA_DESKTOP_VISUAL_LANE = "desktop" as const;

/** Production-shell states built through the real Desktop shell projection. */
export const VISUAL_BASELINE_SHELL_STATES = [
  "composer-idle",
  "thread-plan-card",
  "approval-card",
  "reasoning-disclosure",
  "full-auto-running",
  "surface-tabs",
  "files-rich-diff",
  "terminal-workbench",
  "browser-preview",
  "settings-routed",
  "remote-connect",
  "responsive-standard",
  "responsive-minimum",
] as const;
export type VisualBaselineShellStateName = (typeof VISUAL_BASELINE_SHELL_STATES)[number];

/** Shared #8870 fixture-catalog pages, split so every variant remains legible. */
export const VISUAL_BASELINE_WORKBENCH_STATES = [
  "workbench-messages-reasoning",
  "workbench-commands",
  "workbench-files",
  "workbench-tools-mcp-dynamic",
  "workbench-tools-web-image",
  "workbench-plans-approvals",
  "workbench-agents",
  "workbench-context",
  "workbench-notices-long-tail",
  "workbench-shell",
  "workbench-frame",
] as const;
export type VisualBaselineWorkbenchStateName = (typeof VISUAL_BASELINE_WORKBENCH_STATES)[number];

/** The complete fixed capture set, in capture order. */
export const VISUAL_BASELINE_STATES = [
  ...VISUAL_BASELINE_SHELL_STATES,
  ...VISUAL_BASELINE_WORKBENCH_STATES,
] as const;
export type VisualBaselineStateName = (typeof VISUAL_BASELINE_STATES)[number];

export const isVisualBaselineStateName = (value: string): value is VisualBaselineStateName =>
  (VISUAL_BASELINE_STATES as ReadonlyArray<string>).includes(value);

export const isVisualBaselineShellStateName = (
  value: VisualBaselineStateName,
): value is VisualBaselineShellStateName =>
  (VISUAL_BASELINE_SHELL_STATES as ReadonlyArray<string>).includes(value);

export const isVisualBaselineWorkbenchStateName = (
  value: VisualBaselineStateName,
): value is VisualBaselineWorkbenchStateName =>
  (VISUAL_BASELINE_WORKBENCH_STATES as ReadonlyArray<string>).includes(value);

/** One captured state as reported in the probe's public-safe receipt line. */
export type VisualBaselineCaptureReceipt = Readonly<{
  state: VisualBaselineStateName;
  file: string;
  sha256: string;
  width: number;
  height: number;
}>;

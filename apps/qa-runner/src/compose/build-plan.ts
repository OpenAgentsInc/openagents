// `buildComposePlan` — the pure, deterministic core of the compose layer.
//
// Given resolved run metadata (single, or a before/after pair) it produces a
// fully-specified ComposePlan: a title card describing the scenario + verdict,
// the clip segment(s) with step/keystroke labels and brand framing, and an
// optional side-by-side layout. It does NO I/O and reads NO clock — colors,
// sizes, durations, and text are derived purely from the input, so the same
// input always yields byte-identical plans (verified by tests).
//
// The matching loaders (`loadRunMeta`) and executor (`ffmpeg.ts`) live in
// sibling files; this file is intentionally render-free.

import type {
  ClipSegment,
  ComposeInput,
  ComposePlan,
  ComposeRunMeta,
  ComposeStep,
  RGBA,
  TextOverlay,
  TitleCardSegment,
  VerdictBadge,
} from "./plan.ts";

const DEFAULT_WIDTH = 1280;
const DEFAULT_HEIGHT = 720;
const DEFAULT_TITLE_SECONDS = 2.5;
const DEFAULT_BRAND = "OpenAgents";

// A small deterministic palette. No theming randomness.
const COLOR_BG: RGBA = { r: 13, g: 17, b: 23, a: 1 }; // near-black slate
const COLOR_TEXT: RGBA = { r: 235, g: 237, b: 240, a: 1 };
const COLOR_MUTED: RGBA = { r: 148, g: 161, b: 178, a: 1 };
const COLOR_PASS: RGBA = { r: 34, g: 197, b: 94, a: 1 };
const COLOR_FAIL: RGBA = { r: 239, g: 68, b: 68, a: 1 };
const COLOR_PILL_BG: RGBA = { r: 13, g: 17, b: 23, a: 0.66 };

/** Format a duration in ms as a compact "X.Ys" / "Xm Ys" string. */
export function formatDuration(durationMs: number): string {
  const totalSeconds = Math.max(0, Math.round(durationMs / 100) / 10);
  if (totalSeconds < 60) return `${totalSeconds}s`;
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = Math.round(totalSeconds - minutes * 60);
  return `${minutes}m ${seconds}s`;
}

/** Truncate a label to a bounded length for overlay safety (deterministic). */
export function clampLabel(text: string, max = 64): string {
  if (text.length <= max) return text;
  return `${text.slice(0, Math.max(0, max - 1)).trimEnd()}…`;
}

function verdictColor(verdict: "pass" | "fail"): RGBA {
  return verdict === "pass" ? COLOR_PASS : COLOR_FAIL;
}

function verdictText(verdict: "pass" | "fail"): string {
  return verdict === "pass" ? "PASS" : "FAIL";
}

/**
 * Build the per-step strip: a single line summarizing the labelled steps as
 * keystroke-style pills, e.g. "1 navigate · 2 readText · 3 assert ✓".
 * Deterministic ordering by step index.
 */
export function stepStripText(steps: ReadonlyArray<ComposeStep>): string {
  return [...steps]
    .sort((a, b) => a.index - b.index)
    .map((s) => `${s.index + 1} ${s.kind}${s.status === "failed" ? " ✗" : ""}`)
    .join("  ·  ");
}

function brandOverlay(brand: string, width: number, height: number): TextOverlay {
  void width;
  void height;
  return {
    text: brand,
    x: 0.97,
    y: 0.94,
    fontSize: 22,
    color: COLOR_MUTED,
    align: "right",
  };
}

function titleCard(
  meta: ComposeRunMeta,
  width: number,
  height: number,
  durationSeconds: number,
  brand: string,
): TitleCardSegment {
  void width;
  void height;
  const overlays: TextOverlay[] = [
    {
      text: clampLabel(meta.scenarioTitle, 56),
      x: 0.5,
      y: 0.34,
      fontSize: 54,
      color: COLOR_TEXT,
      align: "center",
    },
  ];
  if (meta.goal !== undefined && meta.goal.length > 0) {
    overlays.push({
      text: clampLabel(meta.goal, 88),
      x: 0.5,
      y: 0.46,
      fontSize: 26,
      color: COLOR_MUTED,
      align: "center",
    });
  }
  overlays.push({
    text: `${meta.targetName}  ·  ${meta.brain}  ·  ${formatDuration(meta.durationMs)}`,
    x: 0.5,
    y: 0.56,
    fontSize: 24,
    color: COLOR_MUTED,
    align: "center",
  });
  overlays.push(brandOverlay(brand, width, height));

  const badge: VerdictBadge = {
    text: verdictText(meta.verdict),
    verdict: meta.verdict,
    x: 0.5,
    y: 0.7,
  };

  return {
    kind: "title-card",
    durationSeconds,
    background: COLOR_BG,
    overlays,
    badge,
  };
}

function clipOverlaysForSingle(
  meta: ComposeRunMeta,
  brand: string,
  showStepStrip: boolean,
  width: number,
  height: number,
): { overlays: TextOverlay[]; badge: VerdictBadge } {
  const overlays: TextOverlay[] = [
    {
      // Top-left scenario chip.
      text: clampLabel(meta.scenarioTitle, 48),
      x: 0.03,
      y: 0.05,
      fontSize: 28,
      color: COLOR_TEXT,
      box: { color: COLOR_PILL_BG, padding: 12 },
      align: "left",
    },
  ];
  if (showStepStrip && meta.steps.length > 0) {
    overlays.push({
      text: clampLabel(stepStripText(meta.steps), 96),
      x: 0.5,
      y: 0.93,
      fontSize: 22,
      color: COLOR_TEXT,
      box: { color: COLOR_PILL_BG, padding: 10 },
      align: "center",
    });
  }
  overlays.push(brandOverlay(brand, width, height));
  const badge: VerdictBadge = {
    text: verdictText(meta.verdict),
    verdict: meta.verdict,
    x: 0.97,
    y: 0.06,
  };
  return { overlays, badge };
}

function variantChip(meta: ComposeRunMeta, leftSide: boolean): TextOverlay {
  return {
    text: clampLabel(meta.variantLabel ?? (leftSide ? "Before" : "After"), 24),
    x: leftSide ? 0.03 : 0.97,
    y: 0.05,
    fontSize: 26,
    color: COLOR_TEXT,
    box: { color: COLOR_PILL_BG, padding: 10 },
    align: leftSide ? "left" : "right",
  };
}

function clipOverlaysForSideBySide(
  before: ComposeRunMeta,
  after: ComposeRunMeta,
  brand: string,
  width: number,
  height: number,
): { overlays: TextOverlay[]; badge: VerdictBadge | undefined } {
  const overlays: TextOverlay[] = [
    variantChip(before, true),
    variantChip(after, false),
    {
      // Per-side verdict mini-labels along the bottom.
      text: `${verdictText(before.verdict)}  →  ${verdictText(after.verdict)}`,
      x: 0.5,
      y: 0.93,
      fontSize: 24,
      color: COLOR_TEXT,
      box: { color: COLOR_PILL_BG, padding: 10 },
      align: "center",
    },
    brandOverlay(brand, width, height),
  ];
  return { overlays, badge: undefined };
}

/**
 * The pure core. Resolve a ComposeInput into a fully-specified ComposePlan.
 * Throws on malformed input (no single and no complete before/after pair).
 */
export function buildComposePlan(input: ComposeInput): ComposePlan {
  const style = input.style ?? {};
  const width = style.width ?? DEFAULT_WIDTH;
  const height = style.height ?? DEFAULT_HEIGHT;
  const titleSeconds = style.titleCardSeconds ?? DEFAULT_TITLE_SECONDS;
  const brand = style.brand ?? DEFAULT_BRAND;
  const showStepStrip = style.showStepStrip ?? true;

  const hasPair = input.before !== undefined && input.after !== undefined;

  if (hasPair) {
    const before = input.before as ComposeRunMeta;
    const after = input.after as ComposeRunMeta;

    // Title card describes the comparison; verdict shown is the AFTER verdict
    // (the state we are demonstrating we reached).
    const titleMeta: ComposeRunMeta = {
      ...after,
      scenarioTitle: `${after.scenarioTitle} — Before / After`,
    };
    const title = titleCard(titleMeta, width, height, titleSeconds, brand);

    const { overlays, badge } = clipOverlaysForSideBySide(
      before,
      after,
      brand,
      width,
      height,
    );
    const clip: ClipSegment = {
      kind: "clip",
      layout: "side-by-side",
      sources: [
        { dir: "before", path: before.video },
        { dir: "after", path: after.video },
      ],
      overlays,
      ...(badge !== undefined ? { badge } : {}),
    };

    return {
      schemaVersion: "openagents.qa_runner.compose_plan.v1",
      width,
      height,
      brand,
      layout: "side-by-side",
      segments: [title, clip],
    };
  }

  const meta = input.single;
  if (meta === undefined) {
    throw new Error(
      "buildComposePlan: provide `single`, or both `before` and `after`",
    );
  }

  const title = titleCard(meta, width, height, titleSeconds, brand);
  const { overlays, badge } = clipOverlaysForSingle(
    meta,
    brand,
    showStepStrip,
    width,
    height,
  );
  const clip: ClipSegment = {
    kind: "clip",
    layout: "single",
    sources: [{ dir: "run", path: meta.video }],
    overlays,
    badge,
  };

  return {
    schemaVersion: "openagents.qa_runner.compose_plan.v1",
    width,
    height,
    brand,
    layout: "single",
    segments: [title, clip],
  };
}

// Re-export the verdict color helper so the executor can share one palette.
export { verdictColor };

// Compose plan model — the typed, data-driven intermediate representation that
// sits between a completed run directory and the ffmpeg executor.
//
// EPIC #6187: "Polished video compose layer for shareable demos."
//
// The headline deliverable is `buildComposePlan(meta) -> ComposePlan`: a PURE,
// DETERMINISTIC function that turns run metadata (result.json +
// session-trace.json, optionally for a before/after pair) into a fully-resolved
// description of the polished video. It performs NO rendering, NO I/O, and NO
// timestamps-from-clock; given the same inputs it always yields the same plan.
// A separate thin executor (`ffmpeg.ts`) consumes the plan and shells out to
// ffmpeg via drawtext / overlay / concat. This split keeps the interesting
// logic unit-testable without a renderer in the loop.
//
// LICENSE NOTE: we deliberately do NOT use Remotion. Remotion requires a paid
// company license for orgs with >3 employees, which conflicts with keeping this
// pipeline fully OSS. An ffmpeg-only compositor (drawtext/overlay/concat) is the
// chosen path; see README.md in this directory.

import { Schema as S } from "effect";

// ---------------------------------------------------------------------------
// Inputs: the public-safe metadata the planner reads.
// ---------------------------------------------------------------------------

/** The minimal verdict the title card and badge render from. */
export const ComposeVerdict = S.Literals(["pass", "fail"]);
export type ComposeVerdict = typeof ComposeVerdict.Type;

/**
 * One labelled step in the run, as shown in the step strip / keystroke pills.
 * Sourced from result.json `steps[]`.
 */
export const ComposeStep = S.Struct({
  index: S.Number,
  kind: S.String,
  label: S.String,
  status: S.Literals(["ok", "failed"]),
});
export type ComposeStep = typeof ComposeStep.Type;

/**
 * The public-safe description of a single completed run, as the planner sees it.
 * This is a projection of result.json (+ session-trace.json for the goal) — it
 * carries NO secrets, prompts, tokens, or cookie values, only what the title
 * card and overlays need.
 */
export const ComposeRunMeta = S.Struct({
  /** Human title — usually the scenario name / target name. */
  scenarioTitle: S.String,
  /** One-line goal/subtitle from the session trace (optional). */
  goal: S.optional(S.String),
  verdict: ComposeVerdict,
  targetName: S.String,
  targetBaseUrl: S.String,
  brain: S.String,
  durationMs: S.Number,
  steps: S.Array(ComposeStep),
  /** Relative path to the source clip inside the run dir. */
  video: S.String,
  /** Relative paths to per-step screenshots inside the run dir. */
  screenshots: S.Array(S.String),
  /**
   * A short label identifying this variant in a before/after layout
   * (e.g. "Before" / "After", or a branch name). Optional; set by the planner.
   */
  variantLabel: S.optional(S.String),
});
export type ComposeRunMeta = typeof ComposeRunMeta.Type;

/**
 * Input to `buildComposePlan`. Either a single run, or a before/after pair.
 * `style` carries presentation knobs with deterministic defaults so the plan
 * never depends on ambient state.
 */
export const ComposeInput = S.Struct({
  single: S.optional(ComposeRunMeta),
  before: S.optional(ComposeRunMeta),
  after: S.optional(ComposeRunMeta),
  style: S.optional(
    S.Struct({
      /** Output frame size. Defaults to 1280x720. */
      width: S.optional(S.Number),
      height: S.optional(S.Number),
      /** Seconds the title card holds before the clip. Default 2.5. */
      titleCardSeconds: S.optional(S.Number),
      /** Brand wordmark drawn in the corner. Default "OpenAgents". */
      brand: S.optional(S.String),
      /** Whether to draw the per-step strip overlay. Default true. */
      showStepStrip: S.optional(S.Boolean),
    }),
  ),
});
export type ComposeInput = typeof ComposeInput.Type;

// ---------------------------------------------------------------------------
// Output: the resolved ComposePlan — a description of segments + overlays.
// ---------------------------------------------------------------------------

export const RGBA = S.Struct({
  r: S.Number,
  g: S.Number,
  b: S.Number,
  a: S.Number,
});
export type RGBA = typeof RGBA.Type;

/** A drawtext overlay anchored within the frame. */
export const TextOverlay = S.Struct({
  text: S.String,
  /** Normalized [0,1] anchor within the frame. */
  x: S.Number,
  y: S.Number,
  fontSize: S.Number,
  color: RGBA,
  /** Optional rounded background box behind the text (a "pill"). */
  box: S.optional(S.Struct({ color: RGBA, padding: S.Number })),
  /** Anchor alignment for x: "left" | "center" | "right". */
  align: S.Literals(["left", "center", "right"]),
});
export type TextOverlay = typeof TextOverlay.Type;

/** The colored verdict badge (PASS / FAIL). */
export const VerdictBadge = S.Struct({
  text: S.String,
  verdict: ComposeVerdict,
  x: S.Number,
  y: S.Number,
});
export type VerdictBadge = typeof VerdictBadge.Type;

/** A title card segment rendered from a solid background + overlays. */
export const TitleCardSegment = S.Struct({
  kind: S.Literal("title-card"),
  durationSeconds: S.Number,
  background: RGBA,
  overlays: S.Array(TextOverlay),
  badge: S.optional(VerdictBadge),
});
export type TitleCardSegment = typeof TitleCardSegment.Type;

/**
 * A clip segment. `sources` is one entry for a single layout, or two entries
 * for a side-by-side before/after layout (left, then right). Each source is a
 * relative path under its run directory (resolved by the executor against the
 * matching `--run`/`--before`/`--after` dir).
 */
export const ClipSegment = S.Struct({
  kind: S.Literal("clip"),
  layout: S.Literals(["single", "side-by-side"]),
  sources: S.Array(
    S.Struct({
      /** Which run dir this source belongs to. */
      dir: S.Literals(["run", "before", "after"]),
      path: S.String,
    }),
  ),
  /** Overlays drawn on top of the (possibly composited) clip. */
  overlays: S.Array(TextOverlay),
  badge: S.optional(VerdictBadge),
});
export type ClipSegment = typeof ClipSegment.Type;

export const ComposeSegment = S.Union([TitleCardSegment, ClipSegment]);
export type ComposeSegment = typeof ComposeSegment.Type;

export const ComposePlan = S.Struct({
  schemaVersion: S.Literal("openagents.qa_runner.compose_plan.v1"),
  width: S.Number,
  height: S.Number,
  brand: S.String,
  /** "single" or "side-by-side" — the top-level shape of the plan. */
  layout: S.Literals(["single", "side-by-side"]),
  segments: S.Array(ComposeSegment),
});
export type ComposePlan = typeof ComposePlan.Type;

export const decodeComposePlan = S.decodeUnknownSync(ComposePlan);

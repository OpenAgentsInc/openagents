/** Pure positioning math for the `switch` variant of `../components/toggle`.
 *
 * Ported from Arcade's `Toggle.tsx` `Switch` component (see
 * `docs/design/2026-07-05-arcade-ui-harvest-audit.md` §2.8 and issue #8398).
 * The knob slides between two **percentage anchors** (`0%`/`100%` of the
 * track's own width via CSS-style `start`) offset by the knob's own measured
 * width (via `onLayout`) plus any inner padding, rather than computing an
 * absolute pixel `translateX` against an assumed/hardcoded track width. That
 * makes the knob rest exactly flush against the track's edge (minus padding)
 * regardless of what width the track actually renders at — no off-by-a-few-px
 * drift if the track is resized or restyled later. */

/** Percentage string accepted by RN's `start`/`end`/`left`/`right` layout
 * props (e.g. `"0%"`, `"100%"`). */
export type PercentAnchor = `${number}%`

export type ToggleKnobPositionInput = Readonly<{
  /** Whether the switch is in its "on" state. */
  on: boolean
  /** The knob's own measured width in points (e.g. from `onLayout`). Must be
   * a non-negative finite number; falls back to `0` otherwise so a not-yet-
   * measured knob still renders at a sane (flush-left) rest position instead
   * of `NaN`-ing the layout. */
  knobWidth: number
  /** Inner padding reserved on the track's start edge (left in LTR) that the
   * "off" position should sit inside of. Defaults to `0`. */
  offsetLeft?: number
  /** Inner padding reserved on the track's end edge (right in LTR) that the
   * "on" position should sit inside of. Defaults to `0`. */
  offsetRight?: number
}>

export type ToggleKnobPosition = Readonly<{
  /** `start` anchor for the knob's own `position: "absolute"` layout —
   * `"0%"` (flush with the track's start edge) when off, `"100%"` (flush
   * with the track's end edge) when on. */
  start: PercentAnchor
  /** `marginStart` pulling the knob back from its `100%` anchor by its own
   * width (plus the end-edge offset) when on, or pushing it in from its `0%`
   * anchor by the start-edge offset when off. */
  marginStart: number
}>

const finiteOrZero = (value: number): number => (Number.isFinite(value) ? value : 0)

/** Computes the switch knob's target `start`/`marginStart` layout pair for a
 * given `on` state. Callers drive each field through their own animation
 * (e.g. Reanimated's `withTiming`) — this function only returns the resting
 * target, not an animated value. */
export const toggleKnobTargetPosition = ({
  knobWidth,
  offsetLeft = 0,
  offsetRight = 0,
  on
}: ToggleKnobPositionInput): ToggleKnobPosition => {
  const safeKnobWidth = Math.max(0, finiteOrZero(knobWidth))
  const safeOffsetLeft = finiteOrZero(offsetLeft)
  const safeOffsetRight = finiteOrZero(offsetRight)

  // `-0 - 0` is `-0` in JS; normalize it back to `0` so a not-yet-measured
  // (or clamped) knob never reports a signed-zero margin.
  const onMarginStart = -safeKnobWidth - safeOffsetRight || 0

  return {
    marginStart: on ? onMarginStart : safeOffsetLeft,
    start: on ? "100%" : "0%"
  }
}

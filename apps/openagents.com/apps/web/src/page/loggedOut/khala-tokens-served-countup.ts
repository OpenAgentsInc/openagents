// Smooth eased count-up for the public "Khala Tokens Served" counter (#6324).
//
// The server now broadcasts the running total in steady ≤3/sec steps (see
// `workers/api/src/sync-broadcast-throttle.ts`). This module makes the VISIBLE
// number ease smoothly BETWEEN those ~334ms updates instead of snapping, so the
// counter reads as a continuous live count-up — and a larger-than-usual delta
// (e.g. the first post-burst reconcile) animates over a bounded, capped duration
// rather than an instant giant jump or a multi-minute crawl.
//
// Design notes:
// - Pure interpolation (`easedCountUpValue`) is unit-tested in isolation.
// - The DOM animator (`animateKhalaCountUp`) is reduced-motion-safe: it SNAPS to
//   the target when `prefers-reduced-motion: reduce` (or when the platform has no
//   `requestAnimationFrame`, e.g. SSR/headless), so it never depends on motion to
//   show the right number.
// - The animator only touches the inner display node's text; the authoritative
//   target stays on the element's `data-value` attribute (read by tests + the
//   headless proof), so smoothing is purely additive and never changes the value
//   the counter converges to.

// easeOutCubic: fast start, gentle settle — reads as a lively but smooth count-up.
const easeOutCubic = (t: number): number => 1 - Math.pow(1 - t, 3)

// Bounds for a single update's animation (ms). Each ~334ms server step should
// animate into the next without stutter, so the floor is short; a large delta is
// capped so it never crawls for minutes.
export const KHALA_COUNTUP_MIN_DURATION_MS = 300
export const KHALA_COUNTUP_MAX_DURATION_MS = 800

// Pick a pleasant, bounded duration for animating `from`→`to`. Small deltas use
// the short floor; the duration grows with the (log of the) delta and is capped.
export const khalaCountUpDurationMs = (from: number, to: number): number => {
  const delta = Math.abs(to - from)
  if (delta <= 0) {
    return 0
  }
  // Log scale so a 9,000,000 post-burst delta and a 5,000 normal delta both land
  // inside the bounded band rather than one being instant and the other crawling.
  const scaled = Math.log10(delta + 1) * 150
  return Math.min(
    KHALA_COUNTUP_MAX_DURATION_MS,
    Math.max(KHALA_COUNTUP_MIN_DURATION_MS, Math.round(scaled)),
  )
}

// Eased interpolated value at fractional progress `t` (clamped to [0,1]) between
// `from` and `to`. Always lands EXACTLY on `to` at t>=1 (no rounding drift), and
// is monotonic for to>=from so the visible number never ticks backward mid-tween.
export const easedCountUpValue = (
  from: number,
  to: number,
  t: number,
): number => {
  if (t <= 0) {
    return from
  }
  if (t >= 1) {
    return to
  }
  const eased = easeOutCubic(t)
  return Math.round(from + (to - from) * eased)
}

export interface KhalaCountUpAnimator {
  // Animate the inner display node from its current shown value to `target`.
  readonly animateTo: (target: number) => void
  // Cancel any in-flight animation (e.g. on teardown).
  readonly cancel: () => void
}

export interface KhalaCountUpDeps {
  // Render a numeric value to its display string (thousands separators etc.).
  readonly format: (value: number) => string
  // Apply the rendered text to the DOM. Injected so the animator is testable.
  readonly setText: (text: string) => void
  // requestAnimationFrame / cancelAnimationFrame, injected for tests + SSR safety.
  // The frame callback receives the high-resolution frame timestamp (the same
  // value the platform `requestAnimationFrame` passes), which the animator uses
  // as its clock — so it reads no raw wall-clock or performance-now primitive.
  readonly requestFrame?: (callback: (timeMs: number) => void) => number
  readonly cancelFrame?: (handle: number) => void
  // Whether the user prefers reduced motion. When true, the animator snaps.
  readonly prefersReducedMotion?: () => boolean
}

// Build a count-up animator. The animator tracks the currently-shown value so a
// new target mid-animation re-eases from where the digits actually are (no jump).
export const makeKhalaCountUpAnimator = (
  deps: KhalaCountUpDeps,
  initialValue: number,
): KhalaCountUpAnimator => {
  const requestFrame = deps.requestFrame
  const cancelFrame = deps.cancelFrame
  const prefersReducedMotion = deps.prefersReducedMotion ?? (() => false)

  let shownValue = initialValue
  let frameHandle: number | null = null

  const cancel = (): void => {
    if (frameHandle !== null && cancelFrame !== undefined) {
      cancelFrame(frameHandle)
    }
    frameHandle = null
  }

  const snap = (target: number): void => {
    cancel()
    shownValue = target
    deps.setText(deps.format(target))
  }

  const animateTo = (target: number): void => {
    if (target === shownValue) {
      return
    }
    // Snap when motion is unwanted or unavailable (SSR/headless/no rAF).
    if (requestFrame === undefined || prefersReducedMotion()) {
      snap(target)
      return
    }

    cancel()
    const from = shownValue
    const duration = khalaCountUpDurationMs(from, target)
    if (duration <= 0) {
      snap(target)
      return
    }
    // The clock is the rAF frame timestamp: the first frame establishes `start`,
    // and each subsequent frame's timestamp gives elapsed time. No raw time
    // primitive is read here.
    let start: number | null = null

    const step = (frameTimeMs: number): void => {
      if (start === null) {
        start = frameTimeMs
      }
      const elapsed = frameTimeMs - start
      const t = Math.min(1, elapsed / duration)
      const value = easedCountUpValue(from, target, t)
      shownValue = value
      deps.setText(deps.format(value))
      if (t >= 1) {
        frameHandle = null
        return
      }
      frameHandle = requestFrame(step)
    }

    frameHandle = requestFrame(step)
  }

  return { animateTo, cancel }
}

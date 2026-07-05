/** Shared motion/timing token scale for `clients/khala-mobile`.
 *
 * Arcade's own animation timings are scattered — a `timing.ts` that defines
 * only `{quick:300}` and is barely used, with nearly every animated component
 * hardcoding its own duration literal (200ms here, 800ms there, 2000ms
 * elsewhere) instead of sharing a scale. Import from here instead of
 * hardcoding a new `duration:` literal so Khala doesn't repeat that drift.
 * See `docs/design/2026-07-05-arcade-ui-harvest-audit.md` §3.
 *
 * Per `docs/design/starcraft.md`, most interactive feedback motion should
 * stay under ~160ms; only ambient "this surface is alive" loops are allowed
 * to run slower. */

/** Snappy press/feedback motion — press states, `TouchableFeedback`-style
 * crossfades, toggle knobs. Stay under ~160ms per the StarCraft doc. */
export const MOTION_FAST = 140

/** Frame/panel transitions — Arwes `Frame` unfold, sheet/banner open-close. */
export const MOTION_MEDIUM = 240

/** Breathing/glow ambient loops (e.g. `BackgroundGradient`) — reserved for
 * factional/"living" ambient flavor, not interactive feedback. Pair with
 * `withRepeat(..., -1, true)` for an infinite yoyo. */
export const MOTION_AMBIENT = 2000

/** Per-list-item stagger increment for entrance animations (e.g.
 * `FadeIn.delay(MOTION_STAGGER_MS * index)`). Tighter than Arcade's own
 * loose 100ms — the audit explicitly recommends tightening this down to
 * something snappier for Khala. */
export const MOTION_STAGGER_MS = 60

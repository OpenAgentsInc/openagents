// Browser controller that drives the smooth "Khala Tokens Served" count-up
// (#6324). It is installed ONCE at app boot (entry.ts) and is fully decoupled
// from the Foldkit update loop: it watches the document for the counter display
// node (`[data-counter-display="khala-tokens-served"]`) and, whenever that node's
// `data-value` target changes, eases its visible text from the currently shown
// value up to the new target via `makeKhalaCountUpAnimator`.
//
// Why a DOM controller rather than threading per-frame state through the model:
// the count-up is a presentation detail with no bearing on app state, and easing
// at ~60fps does not belong in the Elm message loop. The authoritative value
// lives on `data-value` (set by the view from the model); this controller only
// animates how the digits get there. It is reduced-motion-safe (the animator
// snaps) and guarded so it is a no-op outside a real browser.

import {
  type KhalaCountUpAnimator,
  makeKhalaCountUpAnimator,
} from './khala-tokens-served-countup'

const DISPLAY_SELECTOR = '[data-counter-display="khala-tokens-served"]'

// Parse the `data-value` target. The view formats with thousands separators, so
// strip non-digits before parsing. Returns null for the em-dash placeholder.
export const parseCounterTargetValue = (raw: string | null): number | null => {
  if (raw === null) {
    return null
  }
  const digits = raw.replace(/[^0-9]/g, '')
  if (digits.length === 0) {
    return null
  }
  const value = Number(digits)
  return Number.isFinite(value) ? value : null
}

const numberFormatter = new Intl.NumberFormat('en-US')
const formatCounter = (value: number): string => numberFormatter.format(value)

const prefersReducedMotion = (): boolean => {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
    return true
  }
  return window.matchMedia('(prefers-reduced-motion: reduce)').matches
}

// Attach the animator to a single display node and ease it toward its current
// (and any future) `data-value`. Returns a teardown function.
const attachToDisplayNode = (node: HTMLElement): (() => void) => {
  let animator: KhalaCountUpAnimator | null = null

  const ensureAnimator = (seed: number): KhalaCountUpAnimator => {
    if (animator === null) {
      animator = makeKhalaCountUpAnimator(
        {
          format: formatCounter,
          setText: text => {
            node.textContent = text
          },
          requestFrame: callback =>
            window.requestAnimationFrame(callback),
          cancelFrame: handle => window.cancelAnimationFrame(handle),
          prefersReducedMotion,
        },
        seed,
      )
    }
    return animator
  }

  const syncToTarget = (): void => {
    const target = parseCounterTargetValue(node.getAttribute('data-value'))
    if (target === null) {
      return
    }
    // Seed the animator's starting value from the text currently shown (so the
    // first ease starts from what the user sees). When the currently shown text
    // is the em-dash placeholder (or otherwise unparseable), there is nothing to
    // ease from: snap straight to the target so the first real value appears.
    const shownFromText = parseCounterTargetValue(node.textContent)
    if (shownFromText === null) {
      const animator = ensureAnimator(target)
      node.textContent = formatCounter(target)
      // Re-seed from the target via a 0→target ease only if we want motion on
      // first paint; here we land directly on the target (placeholder → value).
      animator.animateTo(target)
      return
    }
    ensureAnimator(shownFromText).animateTo(target)
  }

  // On attach, if a real initial target is already shown, leave it; otherwise the
  // first `syncToTarget` below handles placeholder → value. We do NOT pre-create
  // the animator here so the seed always reflects the actually-shown text.
  syncToTarget()

  const observer = new MutationObserver(() => {
    syncToTarget()
  })
  observer.observe(node, {
    attributes: true,
    attributeFilter: ['data-value'],
  })

  return () => {
    observer.disconnect()
    animator?.cancel()
  }
}

// Install the controller. Watches the whole document so it activates whenever the
// counter is rendered (the landing/khala/stats routes), and survives route
// re-renders. Safe to call once at boot; a no-op outside the browser.
export const installKhalaTokensServedCountUp = (): (() => void) => {
  if (
    typeof document === 'undefined' ||
    typeof MutationObserver === 'undefined'
  ) {
    return () => {}
  }

  const attached = new WeakSet<HTMLElement>()
  const teardowns = new Set<() => void>()

  const attachAll = (): void => {
    const nodes = document.querySelectorAll<HTMLElement>(DISPLAY_SELECTOR)
    nodes.forEach(node => {
      if (attached.has(node)) {
        return
      }
      attached.add(node)
      teardowns.add(attachToDisplayNode(node))
    })
  }

  attachAll()

  const documentObserver = new MutationObserver(() => {
    attachAll()
  })
  documentObserver.observe(document.body, {
    childList: true,
    subtree: true,
  })

  return () => {
    documentObserver.disconnect()
    teardowns.forEach(teardown => teardown())
    teardowns.clear()
  }
}

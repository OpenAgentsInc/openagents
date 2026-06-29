// Presentation controller for the public /chat "Latest" affordance.
//
// The Foldkit model owns the transcript and the click action. This controller
// owns only a browser-measured visibility detail: show the jump button when the
// transcript region is scrollable and the latest turn/end sentinel is currently
// below the viewport. If there is nowhere useful to jump, the control stays
// hidden rather than pretending to be disabled.

const ROOT_SELECTOR = '[data-khala-chat]'
const SCROLL_REGION_SELECTOR = '[data-khala-chat-scroll-region]'
const LATEST_BUTTON_SELECTOR = '[data-khala-chat-latest-button]'

const BOTTOM_THRESHOLD_PX = 28
const OVERFLOW_EPSILON_PX = 4

export const khalaChatLatestButtonIsActionable = (
  region: HTMLElement,
): boolean => {
  const overflow =
    region.scrollHeight > region.clientHeight + OVERFLOW_EPSILON_PX
  const distanceFromBottom =
    region.scrollHeight - region.scrollTop - region.clientHeight

  return overflow && distanceFromBottom > BOTTOM_THRESHOLD_PX
}

const buttonForRegion = (region: HTMLElement): HTMLButtonElement | null => {
  const root = region.closest(ROOT_SELECTOR)
  const scope: ParentNode = root ?? document
  return scope.querySelector<HTMLButtonElement>(LATEST_BUTTON_SELECTOR)
}

const syncRegion = (region: HTMLElement): void => {
  const button = buttonForRegion(region)
  if (button === null) {
    return
  }

  const actionable = khalaChatLatestButtonIsActionable(region)
  button.hidden = !actionable
  button.dataset.khalaChatLatestActionable = actionable ? 'true' : 'false'
  if (actionable) {
    button.removeAttribute('aria-hidden')
  } else {
    button.setAttribute('aria-hidden', 'true')
  }
}

const attachToRegion = (region: HTMLElement): (() => void) => {
  let frame: number | null = null

  const scheduleSync = (): void => {
    if (frame !== null) {
      return
    }
    frame = window.requestAnimationFrame(() => {
      frame = null
      syncRegion(region)
    })
  }

  syncRegion(region)
  region.addEventListener('scroll', scheduleSync, { passive: true })

  const resizeObserver =
    typeof ResizeObserver === 'undefined'
      ? null
      : new ResizeObserver(() => scheduleSync())
  resizeObserver?.observe(region)

  const mutationObserver = new MutationObserver(() => scheduleSync())
  mutationObserver.observe(region, {
    childList: true,
    subtree: true,
    characterData: true,
  })

  return () => {
    region.removeEventListener('scroll', scheduleSync)
    resizeObserver?.disconnect()
    mutationObserver.disconnect()
    if (frame !== null) {
      window.cancelAnimationFrame(frame)
      frame = null
    }
  }
}

export const installKhalaChatLatestButtonController = (): (() => void) => {
  if (
    typeof document === 'undefined' ||
    typeof MutationObserver === 'undefined'
  ) {
    return () => {}
  }

  const attached = new WeakSet<HTMLElement>()
  const teardowns = new Set<() => void>()

  const attachAll = (): void => {
    const regions = document.querySelectorAll<HTMLElement>(
      SCROLL_REGION_SELECTOR,
    )
    regions.forEach(region => {
      if (attached.has(region)) {
        syncRegion(region)
        return
      }
      attached.add(region)
      teardowns.add(attachToRegion(region))
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

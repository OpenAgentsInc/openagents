/**
 * DomService - Browser implementation
 */

import { Effect } from "effect"
import { DomError, type DomService, type DomSwapMode } from "./dom.js"
import type { TemplateResult } from "../template/types.js"
import { renderToString as templateToString } from "../template/render.js"

type FocusSnapshot = {
  readonly element: HTMLElement
  readonly id: string | null
  readonly name: string | null
  readonly selection: {
    readonly start: number | null
    readonly end: number | null
    readonly direction: HTMLInputElement["selectionDirection"]
  } | null
}

type ScrollSnapshot = {
  readonly id: string
  readonly top: number
  readonly left: number
}

const scrollMemory = new Map<string, ScrollSnapshot>()

const escapeSelector = (value: string): string => {
  if (typeof CSS !== "undefined" && typeof CSS.escape === "function") {
    return CSS.escape(value)
  }
  return value.replace(/[^a-zA-Z0-9_-]/g, "\\$&")
}

const captureFocus = (target: Element): FocusSnapshot | null => {
  const active = document.activeElement
  if (!(active instanceof HTMLElement)) {
    return null
  }
  if (!target.contains(active)) {
    return null
  }

  const selection =
    active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement
      ? {
          start: active.selectionStart,
          end: active.selectionEnd,
          direction: active.selectionDirection,
        }
      : null

  return {
    element: active,
    id: active.id || null,
    name: active.getAttribute("name"),
    selection,
  }
}

const restoreFocus = (snapshot: FocusSnapshot | null) => {
  if (!snapshot) {
    return
  }
  if (snapshot.element.isConnected && document.activeElement === snapshot.element) {
    return
  }

  const selector = snapshot.id
    ? `#${escapeSelector(snapshot.id)}`
    : snapshot.name
      ? `[name="${escapeSelector(snapshot.name)}"]`
      : null

  if (!selector) {
    return
  }

  const candidate = document.querySelector(selector)
  if (!(candidate instanceof HTMLElement)) {
    return
  }

  try {
    candidate.focus({ preventScroll: true })
  } catch {
    return
  }

  if (
    snapshot.selection &&
    (candidate instanceof HTMLInputElement ||
      candidate instanceof HTMLTextAreaElement)
  ) {
    const { start, end, direction } = snapshot.selection
    if (start !== null && end !== null) {
      const dir = direction === null ? undefined : direction
      try {
        candidate.setSelectionRange(start, end, dir)
      } catch {
        return
      }
    }
  }
}

const captureScroll = (target: Element) => {
  const nodes = target.querySelectorAll<HTMLElement>("[data-scroll-id]")
  nodes.forEach((node) => {
    const id = node.getAttribute("data-scroll-id")
    if (!id) {
      return
    }
    scrollMemory.set(id, {
      id,
      top: node.scrollTop,
      left: node.scrollLeft,
    })
  })
}

const restoreScroll = (target: Element | Document) => {
  const root = target instanceof Document ? target : target
  const nodes = root.querySelectorAll<HTMLElement>("[data-scroll-id]")
  nodes.forEach((node) => {
    const id = node.getAttribute("data-scroll-id")
    if (!id) {
      return
    }
    const snapshot = scrollMemory.get(id)
    if (!snapshot) {
      return
    }
    node.scrollTop = snapshot.top
    node.scrollLeft = snapshot.left
  })
}

const swapImpl = (
  target: Element,
  content: TemplateResult,
  mode: DomSwapMode
) =>
  Effect.gen(function* () {
    try {
      const focusSnapshot = captureFocus(target)
      captureScroll(target)
      const html = templateToString(content)
      switch (mode) {
        case "inner":
          target.innerHTML = html
          break
        case "outer":
        case "replace":
          target.outerHTML = html
          break
        case "beforeend":
          target.insertAdjacentHTML("beforeend", html)
          break
        case "afterbegin":
          target.insertAdjacentHTML("afterbegin", html)
          break
        case "delete":
          target.remove()
          break
      }

      if (mode === "inner" || mode === "outer" || mode === "replace") {
        restoreFocus(focusSnapshot)
        const restoreRoot = target.parentElement ?? document
        restoreScroll(mode === "inner" ? target : restoreRoot)
      }
    } catch (error) {
      return yield* Effect.fail(
        new DomError(`Failed to swap: ${String(error)}`, error)
      )
    }
  })

export const DomServiceLive: DomService = {
  query: (selector: string) =>
    Effect.gen(function* () {
      const element = document.querySelector(selector)
      if (!element) {
        return yield* Effect.fail(
          new DomError(`Element not found: ${selector}`)
        )
      }
      return element
    }),

  queryOption: (selector: string) =>
    Effect.gen(function* () {
      const element = document.querySelector(selector)
      return element ?? null
    }),

  queryAll: (selector: string) =>
    Effect.gen(function* () {
      const elements = Array.from(document.querySelectorAll(selector))
      return elements
    }),

  render: (container: Element, content: TemplateResult) =>
    swapImpl(container, content, "inner"),

  swap: (target: Element, content: TemplateResult, mode: DomSwapMode = "inner") =>
    swapImpl(target, content, mode),

  delegate: (container: Element, selector: string, event: string, handler: (e: Event, target: Element) => void) =>
    Effect.sync(() => {
      const listener = (e: Event) => {
        const target = e.target as Element
        if (target && target.matches(selector)) {
          handler(e, target)
        } else {
          const closest = target.closest?.(selector)
          if (closest) {
            handler(e, closest)
          }
        }
      }
      container.addEventListener(event, listener)
    }).pipe(Effect.asVoid),
}

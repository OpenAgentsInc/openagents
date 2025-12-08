/**
 * Effuse DOM Service - Live Implementation
 *
 * Browser implementation of DomService.
 */

import { Effect, Layer } from "effect"
import { DomServiceTag, DomError, type DomService } from "./dom.js"
import type { TemplateResult } from "../template/types.js"

/**
 * Create the live DomService implementation.
 *
 * @param root - Optional root element for queries (defaults to document)
 */
const makeDomService = (root: Document | Element = document): DomService => ({
  query: <T extends Element>(selector: string) =>
    Effect.gen(function* () {
      const element = root.querySelector<T>(selector)
      if (!element) {
        return yield* Effect.fail(
          new DomError("element_not_found", `Element not found: ${selector}`)
        )
      }
      return element
    }),

  queryOption: <T extends Element>(selector: string) =>
    Effect.succeed(root.querySelector<T>(selector)),

  queryId: <T extends Element>(id: string) =>
    Effect.gen(function* () {
      const element = document.getElementById(id) as T | null
      if (!element) {
        return yield* Effect.fail(
          new DomError("element_not_found", `Element not found: #${id}`)
        )
      }
      return element
    }),

  render: (element: Element, content: TemplateResult) =>
    Effect.gen(function* () {
      try {
        element.innerHTML = content.toString()
      } catch (error) {
        return yield* Effect.fail(
          new DomError(
            "render_failed",
            `Failed to render: ${error instanceof Error ? error.message : String(error)}`
          )
        )
      }
    }),

  listen: <K extends keyof HTMLElementEventMap>(
    element: Element,
    event: K,
    handler: (e: HTMLElementEventMap[K]) => void,
    options?: AddEventListenerOptions
  ) =>
    Effect.sync(() => {
      const listener = handler as EventListener
      element.addEventListener(event, listener, options)
      return () => element.removeEventListener(event, listener, options)
    }),

  delegate: <K extends keyof HTMLElementEventMap>(
    container: Element,
    selector: string,
    event: K,
    handler: (e: HTMLElementEventMap[K], target: Element) => void
  ) =>
    Effect.sync(() => {
      const bunLog = typeof window !== "undefined" ? (window as any).bunLog : null
      if (bunLog) {
        bunLog(`[DomService] Setting up delegate for "${event}" on container "${container.id || container.className || container.tagName}", selector="${selector}"`)
      }

      const listener = (e: Event) => {
        if (bunLog && selector.includes("runFullBenchmark")) {
          bunLog(`[DomService] ${event} event received, target:`, e.target)
        }

        // Handle text nodes - get the parent element if target is a text node
        let element: Element | null = null
        if (e.target instanceof Element) {
          element = e.target
        } else if (e.target instanceof Node && e.target.parentElement) {
          element = e.target.parentElement
        }

        if (bunLog && selector.includes("runFullBenchmark") && element) {
          bunLog(`[DomService] Resolved element:`, element.tagName, element.className)
        }

        const target = element?.closest(selector)

        if (bunLog && selector.includes("runFullBenchmark")) {
          bunLog(`[DomService] closest("${selector}") result:`, target, `container.contains:`, target ? container.contains(target) : false)
        }

        if (target && container.contains(target)) {
          if (bunLog && selector.includes("runFullBenchmark")) {
            bunLog(`[DomService] Calling handler for ${event}`)
          }
          handler(e as HTMLElementEventMap[K], target)
        }
      }
      container.addEventListener(event, listener)
      return () => container.removeEventListener(event, listener)
    }),

  createFragment: (content: TemplateResult) =>
    Effect.gen(function* () {
      try {
        const template = document.createElement("template")
        template.innerHTML = content.toString()
        return template.content
      } catch (error) {
        return yield* Effect.fail(
          new DomError(
            "render_failed",
            `Failed to create fragment: ${error instanceof Error ? error.message : String(error)}`
          )
        )
      }
    }),
})

/**
 * Layer providing the live DomService implementation.
 * Uses Effect.sync to defer document access until runtime.
 */
export const DomServiceLive = Layer.effect(
  DomServiceTag,
  Effect.sync(() => makeDomService())
)

/**
 * Create a DomService layer scoped to a specific root element.
 */
export const DomServiceScoped = (root: Element) =>
  Layer.succeed(DomServiceTag, makeDomService(root))

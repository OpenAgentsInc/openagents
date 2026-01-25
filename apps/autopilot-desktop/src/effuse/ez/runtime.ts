/**
 * Hypermedia action runtime.
 */

import { Effect, Fiber } from "effect"
import type { DomService } from "../services/dom.js"
import { DomServiceTag, type DomSwapMode } from "../services/dom.js"
import { EzRegistryTag } from "./registry.js"
import type { EzAction } from "./types.js"

type EzTrigger = "click" | "submit" | "change" | "input"

const SUPPORTED_TRIGGERS: readonly EzTrigger[] = [
  "click",
  "submit",
  "change",
  "input",
]

const isTrigger = (value: string): value is EzTrigger =>
  SUPPORTED_TRIGGERS.includes(value as EzTrigger)

const getDefaultTrigger = (el: Element): EzTrigger => {
  if (el instanceof HTMLFormElement) {
    return "submit"
  }
  if (
    el instanceof HTMLInputElement ||
    el instanceof HTMLSelectElement ||
    el instanceof HTMLTextAreaElement
  ) {
    return "change"
  }
  return "click"
}

const parseTrigger = (el: Element): EzTrigger => {
  const attr = el.getAttribute("data-ez-trigger")
  if (attr && isTrigger(attr)) {
    return attr
  }
  return getDefaultTrigger(el)
}

const parseSwapMode = (value: string | null): DomSwapMode => {
  switch (value) {
    case "inner":
    case "outer":
    case "beforeend":
    case "afterbegin":
    case "delete":
    case "replace":
      return value
    default:
      return "inner"
  }
}

const parseTarget = (
  root: Element,
  actionEl: Element,
  targetSpec: string | null
): Element | null => {
  if (!targetSpec || targetSpec === "this") {
    return actionEl
  }

  if (targetSpec.startsWith("closest(") && targetSpec.endsWith(")")) {
    const selector = targetSpec.slice("closest(".length, -1).trim()
    return actionEl.closest(selector)
  }

  if (targetSpec.startsWith("find(") && targetSpec.endsWith(")")) {
    const selector = targetSpec.slice("find(".length, -1).trim()
    return actionEl.querySelector(selector)
  }

  return root.querySelector(targetSpec) ?? document.querySelector(targetSpec)
}

const collectParams = (actionEl: Element, event: Event): Record<string, string> => {
  const params: Record<string, string> = {}

  if (event.type === "submit") {
    const form =
      (event.target instanceof HTMLFormElement
        ? event.target
        : actionEl instanceof HTMLFormElement
          ? actionEl
          : actionEl.closest("form")) ?? null

    if (form) {
      const formData = new FormData(form)
      for (const [key, value] of formData.entries()) {
        params[key] = typeof value === "string" ? value : value.name
      }
    }
  } else if (
    actionEl instanceof HTMLInputElement ||
    actionEl instanceof HTMLSelectElement ||
    actionEl instanceof HTMLTextAreaElement
  ) {
    if (actionEl.name) {
      params[actionEl.name] = actionEl.value
    }
  }

  const vals = actionEl.getAttribute("data-ez-vals")
  if (vals) {
    try {
      const parsed = JSON.parse(vals) as Record<string, unknown>
      for (const [key, value] of Object.entries(parsed)) {
        params[key] = typeof value === "string" ? value : JSON.stringify(value)
      }
    } catch (error) {
      console.warn("[Effuse/Ez] Failed to parse data-ez-vals:", error)
    }
  }

  return params
}

const setDisabled = (
  actionEl: Element,
  disabled: boolean,
  previous: { disabled: boolean | null; aria: string | null }
) => {
  if ("disabled" in actionEl) {
    if (disabled) {
      previous.disabled = (actionEl as HTMLButtonElement).disabled
      ;(actionEl as HTMLButtonElement).disabled = true
    } else if (previous.disabled !== null) {
      ;(actionEl as HTMLButtonElement).disabled = previous.disabled
    } else {
      ;(actionEl as HTMLButtonElement).disabled = false
    }
  }

  if (disabled) {
    previous.aria = actionEl.getAttribute("aria-disabled")
    actionEl.setAttribute("aria-disabled", "true")
  } else if (previous.aria !== null) {
    actionEl.setAttribute("aria-disabled", previous.aria)
  } else {
    actionEl.removeAttribute("aria-disabled")
  }
}

const maybeConfirm = (actionEl: Element): boolean => {
  const confirmText = actionEl.getAttribute("data-ez-confirm")
  if (!confirmText) {
    return true
  }
  return window.confirm(confirmText)
}

const runAction = (
  dom: DomService,
  registry: Map<string, EzAction>,
  root: Element,
  actionEl: Element,
  event: Event
) =>
  Effect.gen(function* () {
    const actionName = actionEl.getAttribute("data-ez")
    if (!actionName) {
      return
    }

    const action = registry.get(actionName)
    if (!action) {
      console.warn(`[Effuse/Ez] No action registered for "${actionName}"`)
      return
    }

    const params = collectParams(actionEl, event)
    const targetSpec = actionEl.getAttribute("data-ez-target")
    const swapMode = parseSwapMode(actionEl.getAttribute("data-ez-swap"))
    const target = parseTarget(root, actionEl, targetSpec)
    const shouldDisable = actionEl.hasAttribute("data-ez-disable")
    const previous = { disabled: null as boolean | null, aria: null as string | null }

    if (shouldDisable) {
      setDisabled(actionEl, true, previous)
    }

    try {
      const result = yield* action({ event, el: actionEl, params, dom })
      if (result && target) {
        yield* dom.swap(target, result, swapMode)
      }
    } catch (error) {
      console.error("[Effuse/Ez] Action failed:", error)
    } finally {
      if (shouldDisable) {
        setDisabled(actionEl, false, previous)
      }
    }
  })

export const mountEzRuntime = (root: Element) =>
  Effect.gen(function* () {
    const dom = yield* DomServiceTag
    const registry = yield* EzRegistryTag
    const inflight = new WeakMap<Element, Fiber.RuntimeFiber<unknown, unknown>>()

    const handleEvent = (event: Event, actionEl: Element) => {
      if (!maybeConfirm(actionEl)) {
        return
      }

      const trigger = parseTrigger(actionEl)
      if (event.type !== trigger) {
        return
      }

      if (event.type === "submit") {
        event.preventDefault()
      }

      const previous = inflight.get(actionEl)
      if (previous) {
        Effect.runFork(Fiber.interrupt(previous))
      }

      const fiber = Effect.runFork(runAction(dom, registry, root, actionEl, event))
      inflight.set(actionEl, fiber)
    }

    for (const trigger of SUPPORTED_TRIGGERS) {
      yield* dom.delegate(root, "[data-ez]", trigger, handleEvent)
    }
  })

export const mountEzRuntimeWith = (
  root: Element,
  registry: Map<string, EzAction>
) => mountEzRuntime(root).pipe(Effect.provideService(EzRegistryTag, registry))

/**
 * Composer focus on open (#8787, owner verbatim: "the text input should be
 * focused immediately on open. so i can start typing right away.").
 *
 * DOM-level oracles: at shell-mount the composer holds document.activeElement
 * and the first keystroke routes to it; the retry loop survives a composer
 * that (re)mounts on a later commit; the guarded settle pass reclaims only
 * UNOWNED focus — late hydration and window re-activation never steal focus
 * the user placed elsewhere. The built-Electron smoke proves the same journey
 * with a real Chromium keystroke.
 */
import { describe, expect, test } from "vite-plus/test"
import { Window } from "../../../openagents.com/apps/web/node_modules/happy-dom"

import {
  findComposerInput,
  focusIsUnowned,
  makeComposerFocuser,
  makeComposerFocusSettler,
} from "./composer-focus.ts"

type TimerQueue = Array<() => void>

/** Deterministic timer: collect callbacks, drain on demand. */
const makeTimers = () => {
  const queue: TimerQueue = []
  const drain = (): void => {
    while (queue.length > 0) queue.shift()!()
  }
  return { schedule: (callback: () => void) => { queue.push(callback); return 0 }, drain, queue }
}

const makeShell = (withComposer = true) => {
  const window = new Window({ url: "http://localhost/" })
  const document = window.document as unknown as Document
  const root = document.createElement("div")
  document.body.appendChild(root)
  if (withComposer) mountComposer(root)
  return { window, document, root }
}

const mountComposer = (root: HTMLElement): HTMLTextAreaElement => {
  const host = root.ownerDocument.createElement("div")
  host.setAttribute("data-en-key", "shell-input")
  const textarea = root.ownerDocument.createElement("textarea")
  host.appendChild(textarea)
  root.appendChild(host)
  return textarea
}

describe("composer focused on open (#8787)", () => {
  test("focus lands on the composer at shell-mount and the first keystroke routes to it", () => {
    const { document, root } = makeShell()
    const timers = makeTimers()
    makeComposerFocuser({ root, setTimeout: timers.schedule })()
    timers.drain()
    const composer = findComposerInput(root)!
    expect(document.activeElement).toBe(composer)
    // First keystroke, zero clicks: key events dispatched at the document's
    // active element reach the composer.
    let keys = 0
    composer.addEventListener("keydown", () => { keys += 1 })
    ;(document.activeElement as HTMLElement).dispatchEvent(
      new (root.ownerDocument.defaultView as any).KeyboardEvent("keydown", { key: "k", bubbles: true }),
    )
    expect(keys).toBe(1)
  })

  test("retries across commits: a composer that mounts on a later commit still receives focus", () => {
    const { document, root } = makeShell(false)
    const timers = makeTimers()
    makeComposerFocuser({ root, setTimeout: timers.schedule })()
    // Two empty commits pass before the composer exists.
    timers.queue.shift()!()
    timers.queue.shift()!()
    const composer = mountComposer(root)
    timers.drain()
    expect(document.activeElement).toBe(composer)
  })

  test("settle claims UNOWNED focus for the composer (post-hydration, window re-activate)", () => {
    const { document, root } = makeShell()
    const timers = makeTimers()
    expect(focusIsUnowned(document)).toBe(true)
    makeComposerFocusSettler({ root, setTimeout: timers.schedule })()
    timers.drain()
    expect(document.activeElement).toBe(findComposerInput(root))
  })

  test("settle NEVER steals focus the user placed elsewhere", () => {
    const { document, root } = makeShell()
    const search = document.createElement("input")
    search.setAttribute("data-en-key", "history-search-field")
    root.appendChild(search)
    search.focus()
    expect(focusIsUnowned(document)).toBe(false)
    const timers = makeTimers()
    makeComposerFocusSettler({ root, setTimeout: timers.schedule })()
    timers.drain()
    expect(document.activeElement).toBe(search)
  })

  test("a disabled or missing composer never traps the retry loop", () => {
    const { root } = makeShell(false)
    const timers = makeTimers()
    makeComposerFocuser({ root, setTimeout: timers.schedule })()
    let steps = 0
    while (timers.queue.length > 0 && steps < 100) { timers.queue.shift()!(); steps += 1 }
    // Bounded: the loop gives up after its retry budget instead of spinning.
    expect(steps).toBeLessThanOrEqual(21)
  })
})

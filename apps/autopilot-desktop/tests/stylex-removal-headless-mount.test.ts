// #6046: headless-mount smoke + StyleX-removal regression.
//
// Before this change, the desktop renderer's import graph could not mount in a
// headless context without the `OA_STYLEX_RUNTIME_FALLBACK` hack: StyleX's
// `stylex.create(...)` executed `window`-dependent code at module-evaluation
// time and threw `window is not defined`. That blocked the app-replica /
// headless-pixel harness (#6045) and forced a runtime shim.
//
// StyleX is now removed from the runtime. The ONLY browser-global dependency
// left in the renderer graph is snabbdom (foldkit's vdom), which a real headless
// chromium provides natively and the bun-test preload provides as a minimal
// shim. Critically, this test does NOT set `OA_STYLEX_RUNTIME_FALLBACK` — the
// graph must import and render without it.

import { describe, expect, test } from "bun:test"

import { ChangedShellInput } from "../src/ui/message"
import { initialModel } from "../src/ui/model"
import { update } from "../src/ui/update"
import { view } from "../src/ui/view"

const treeContainsClass = (node: unknown, className: string): boolean => {
  if (node === null || typeof node !== "object") return false
  const vnode = node as {
    children?: unknown[]
    data?: { class?: Record<string, boolean> }
  }
  if (vnode.data?.class?.[className]) return true
  return Array.isArray(vnode.children)
    ? vnode.children.some(child => treeContainsClass(child, className))
    : false
}

describe("#6046 StyleX removal — headless mount", () => {
  test("the OA_STYLEX_RUNTIME_FALLBACK shim is no longer needed (and not set here)", () => {
    // The preload no longer sets it; nothing in the graph should require it.
    expect(process.env.OA_STYLEX_RUNTIME_FALLBACK).toBeUndefined()
  })

  test("the renderer import graph mounts and renders the shell without throwing", () => {
    // Reaching this line at all means importing `view`/`update`/`model` (which
    // pulls @openagentsinc/ui + @openagentsinc/autopilot-ui) did not throw the
    // StyleX `window is not defined` error at module load.
    const document = view({ ...initialModel, pane: "shell" })
    expect(document.body).toBeDefined()
    // The shell chrome uses the plain-CSS class names that replaced the deleted
    // desktop-stylex.ts StyleX module.
    expect(treeContainsClass(document.body, "app-shell-shell")).toBe(true)
    expect(treeContainsClass(document.body, "shell-pane")).toBe(true)
    expect(treeContainsClass(document.body, "shell-bar")).toBe(true)
    expect(treeContainsClass(document.body, "shell-input")).toBe(true)
  })

  test("the network surface renders the ported app-shell class", () => {
    const document = view({ ...initialModel, pane: "network" })
    expect(treeContainsClass(document.body, "app-shell-network")).toBe(true)
  })

  test("the reducer still runs over the freshly-imported graph", () => {
    // A trivial update proves the update module imported cleanly too.
    const [next] = update(initialModel, ChangedShellInput({ value: "hi" }))
    expect(next).toBeDefined()
    expect(next.shellInput).toBe("hi")
  })
})

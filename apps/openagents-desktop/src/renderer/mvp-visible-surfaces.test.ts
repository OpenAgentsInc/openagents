/**
 * UX-4 (#8790) composition oracle: the MVP visible-surface allowlist is
 * enforced against the ACTUAL rendered shell — every reachable workspace
 * state renders only allowlisted dock items and screens, and the oracle
 * demonstrably fails on a planted non-MVP surface.
 *
 * Owner statement (rc.10 review, 2026-07-14, verbatim): "You need to clean
 * all this up and make a pass to remove everything from the sidebar and all
 * UI that's not specifically called for in our MVP spec."
 */
import { describe, expect, test } from "bun:test"

import {
  desktopShellView,
  initialDesktopShellState,
  withCommandPalette,
  withWorkspace,
  type DesktopShellState,
} from "./shell.ts"
import {
  collectRenderedDockItemIds,
  desktopMvpSurfaceViolations,
  forbiddenVisibleSurfaceKeys,
  mvpAllowedDockItemIds,
  mvpDockSurfaces,
  mvpRemovedDockItemIds,
} from "./mvp-visible-surfaces.ts"

const testThread = { id: "test-thread", title: "New chat", updatedAt: "2026-07-10T18:04:00.000Z", notes: [] } as const
const baseState: DesktopShellState = {
  ...initialDesktopShellState("electron/darwin", "18:04"),
  threads: [testThread],
  activeThreadId: testThread.id,
}

/** JSON round-trip: the oracle reads only serializable structure. */
const clone = (view: unknown): ReturnType<typeof desktopShellView> =>
  JSON.parse(JSON.stringify(view)) as ReturnType<typeof desktopShellView>

type MutableNode = Record<string, unknown>

const findNode = (root: unknown, key: string): MutableNode | null => {
  if (Array.isArray(root)) {
    for (const item of root) {
      const found = findNode(item, key)
      if (found !== null) return found
    }
    return null
  }
  if (typeof root !== "object" || root === null) return null
  const node = root as MutableNode
  if (node.key === key) return node
  for (const [prop, child] of Object.entries(node)) {
    if (prop === "_tag") continue
    const found = findNode(child, key)
    if (found !== null) return found
  }
  return null
}

describe("MVP visible-surface composition oracle (UX-4 #8790)", () => {
  test("every allowlisted dock surface carries its MVP authority citation", () => {
    for (const surface of mvpDockSurfaces) {
      expect(surface.authority.length).toBeGreaterThan(10)
    }
    expect(mvpAllowedDockItemIds).toEqual([
      "workspace-new-chat",
      "workspace-chat",
      "workspace-product-spec",
      "workspace-assurance-spec",
      "workspace-home",
      "shell-settings-toggle",
    ])
    // The swept dock affordances stay off the allowlist AND on the forbidden list.
    for (const removed of mvpRemovedDockItemIds) {
      expect(mvpAllowedDockItemIds).not.toContain(removed)
      expect(forbiddenVisibleSurfaceKeys).toContain(removed)
    }
  })

  test("the rendered dock is exactly the allowlist, in order", () => {
    expect(collectRenderedDockItemIds(desktopShellView(baseState))).toEqual([...mvpAllowedDockItemIds])
  })

  const reachableWorkspaces = ["chat", "home", "files", "product-spec", "assurance-spec", "review", "settings"] as const

  for (const workspace of reachableWorkspaces) {
    test(`workspace "${workspace}" renders zero visible-surface violations`, () => {
      expect(desktopMvpSurfaceViolations(desktopShellView(withWorkspace(baseState, workspace)))).toEqual([])
    })
  }

  test("the empty-history welcome state renders zero violations", () => {
    const empty: DesktopShellState = { ...initialDesktopShellState("electron/darwin", "18:04") }
    expect(desktopMvpSurfaceViolations(desktopShellView(empty))).toEqual([])
  })

  test("the open command palette renders zero violations", () => {
    expect(desktopMvpSurfaceViolations(desktopShellView(withCommandPalette(baseState, true)))).toEqual([])
  })

  for (const legacy of ["fleet", "terminal", "inbox"] as const) {
    test(`a state forced to legacy workspace "${legacy}" renders no non-MVP screen`, () => {
      // The shell falls back to Project home; no Fleet/Terminal/Inbox surface
      // may render even when internal state names one.
      const view = desktopShellView(withWorkspace(baseState, legacy))
      expect(desktopMvpSurfaceViolations(view)).toEqual([])
      expect(findNode(view, "workspace-home-panel")).not.toBeNull()
    })
  }

  test("FALSIFIER: a planted non-MVP dock item fails the oracle", () => {
    const planted = clone(desktopShellView(baseState))
    const nav = findNode(planted, "sidebar-navigation")
    expect(nav).not.toBeNull()
    const sections = nav!.sections as Array<{ id: string; items: Array<{ id: string }> }>
    sections[0]!.items.push({ id: "workspace-fleet" })
    const violations = desktopMvpSurfaceViolations(planted)
    expect(violations.some(violation => violation.includes("workspace-fleet"))).toBe(true)
  })

  test("FALSIFIER: a planted removed dock affordance fails the oracle", () => {
    const planted = clone(desktopShellView(baseState))
    const nav = findNode(planted, "sidebar-navigation")
    const sections = nav!.sections as Array<{ id: string; items: Array<{ id: string }> }>
    sections[0]!.items.push({ id: "workspace-files" })
    const violations = desktopMvpSurfaceViolations(planted)
    expect(violations.some(violation => violation.includes("workspace-files"))).toBe(true)
  })

  test("FALSIFIER: a planted forbidden screen key fails the oracle", () => {
    const planted = clone(desktopShellView(baseState))
    const root = findNode(planted, "shell-root")
    expect(root).not.toBeNull()
    const children = root!.children as Array<unknown>
    children.push({ _tag: "Stack", key: "fleet-desk", children: [] })
    const violations = desktopMvpSurfaceViolations(planted)
    expect(violations.some(violation => violation.includes("fleet-desk"))).toBe(true)
  })

  test("FALSIFIER: losing an allowlisted dock item fails the oracle (no silent shrink)", () => {
    const planted = clone(desktopShellView(baseState))
    const nav = findNode(planted, "sidebar-navigation")
    const sections = nav!.sections as Array<{ id: string; items: Array<{ id: string }> }>
    sections[0]!.items = sections[0]!.items.filter(item => item.id !== "workspace-home")
    const violations = desktopMvpSurfaceViolations(planted)
    expect(violations.some(violation => violation.includes("workspace-home"))).toBe(true)
  })
})

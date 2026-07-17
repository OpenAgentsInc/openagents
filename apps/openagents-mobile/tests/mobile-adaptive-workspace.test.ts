import { describe, expect, test } from "vite-plus/test"
import { Effect, Stream } from "@effect-native/core/effect"
import { IntentRef, StaticPayload } from "@effect-native/core"

import {
  clampMobileWorkspaceSidebar,
  mobileWorkspaceActiveDescendant,
  mobileWorkspaceLayoutMode,
} from "../src/screens/mobile-adaptive-workspace"
import {
  buildHomeProgram,
  renderHomeView,
} from "../src/screens/home-core"

const settle = Effect.gen(function* () {
  yield* Effect.promise<void>(() => new Promise(resolve => setTimeout(resolve, 0)))
  yield* Effect.yieldNow
})

const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), option => {
    if (option._tag !== "Some") throw new Error("expected state")
    return option.value
  })

describe("contract openagents_mobile.adaptive_workspace.v1", () => {
  test("classifies phone/tablet widths and clamps hostile sidebar sizes", () => {
    expect(mobileWorkspaceLayoutMode(390)).toBe("compact")
    expect(mobileWorkspaceLayoutMode(767)).toBe("compact")
    expect(mobileWorkspaceLayoutMode(768)).toBe("regular")
    expect(mobileWorkspaceLayoutMode(1_024)).toBe("regular")
    expect(mobileWorkspaceLayoutMode(Number.NaN)).toBe("compact")
    expect(clampMobileWorkspaceSidebar(-1)).toBe(240)
    expect(clampMobileWorkspaceSidebar(299.6)).toBe(300)
    expect(clampMobileWorkspaceSidebar(9_000)).toBe(360)
    expect(clampMobileWorkspaceSidebar(Number.NaN)).toBe(288)
  })

  test("phone keeps drawer/detail exclusive while tablet mounts one persistent split", () => {
    const phone = buildHomeProgram({ workspaceWidth: 390 })
    const tablet = buildHomeProgram({ workspaceWidth: 820 })
    const phoneView = JSON.stringify(renderHomeView(phone.initialState))
    const tabletView = JSON.stringify(renderHomeView(tablet.initialState))
    expect(phone.initialState.workspaceLayoutMode).toBe("compact")
    expect(phoneView).not.toContain("workspace-regular-split")
    expect(phoneView).toContain('"key":"home-root"')
    expect(phoneView).not.toContain('"key":"drawer-root"')
    expect(tablet.initialState.workspaceLayoutMode).toBe("regular")
    expect(tabletView).toContain('"_tag":"SplitPane"')
    expect(tabletView).toContain('"key":"workspace-regular-split"')
    expect(tabletView).toContain('"id":"navigation"')
    expect(tabletView).toContain('"id":"detail"')
    expect(tabletView.match(/"key":"home-root"/gu)).toHaveLength(1)
  })

  test("navigation toggles are layout-aware and resize is exact, bounded, and focus-accounted", async () => {
    const phone = buildHomeProgram({ workspaceWidth: 390 })
    phone.chrome.toggleDrawer()
    await Effect.runPromise(settle)
    const phoneOpen = await Effect.runPromise(lastState(phone))
    expect(phoneOpen).toMatchObject({ drawerOpen: true, workspaceFocusTarget: "navigation" })
    expect(mobileWorkspaceActiveDescendant("compact", true, false, "navigation")).toBe("drawer-root")

    const tablet = buildHomeProgram({ workspaceWidth: 820 })
    tablet.chrome.toggleDrawer()
    await Effect.runPromise(settle)
    expect(await Effect.runPromise(lastState(tablet))).toMatchObject({
      drawerOpen: false,
      workspaceSidebarCollapsed: true,
      workspaceFocusTarget: "transcript",
    })
    await Effect.runPromise(tablet.report(IntentRef(
      "WorkspaceSidebarResized",
      StaticPayload({ paneId: "navigation", size: 999 }),
    )) as Effect.Effect<unknown>)
    await Effect.runPromise(settle)
    expect(await Effect.runPromise(lastState(tablet))).toMatchObject({
      workspaceSidebarWidth: 360,
      workspaceSidebarCollapsed: false,
      workspaceFocusTarget: "navigation",
    })
    expect(mobileWorkspaceActiveDescendant("regular", false, false, "navigation"))
      .toBe("drawer-root")
  })

  test("orientation changes preserve the live task state instead of rebuilding the program", async () => {
    const program = buildHomeProgram({ workspaceWidth: 390 })
    program.khala.draftChanged("Keep this draft through rotation")
    await Effect.runPromise(settle)
    program.workspace.setWidth(820)
    await Effect.runPromise(settle)
    expect(await Effect.runPromise(lastState(program))).toMatchObject({
      workspaceLayoutMode: "regular",
      drawerOpen: false,
      workspaceSidebarCollapsed: false,
      workspaceFocusTarget: "transcript",
      khala: { draft: "Keep this draft through rotation" },
    })
  })

  test("route-aware navigation copy distinguishes drawer from persistent sidebar", () => {
    const phone = JSON.stringify(renderHomeView(buildHomeProgram({ workspaceWidth: 390 }).initialState))
    const tablet = JSON.stringify(renderHomeView(buildHomeProgram({ workspaceWidth: 820 }).initialState))
    expect(phone).toContain("Go to workspace navigation")
    expect(tablet).toContain("Hide workspace navigation")
  })
})

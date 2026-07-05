import { describe, expect, test } from "bun:test"
import type { NavigationState } from "@react-navigation/native"

import {
  KHALA_NAVIGATION_PERSISTENCE_DECISION,
  decideBackAction,
  getActiveRouteName,
  routeNameSummary,
} from "../src/navigators/navigationUtilities"

describe("Khala mobile navigation hardening", () => {
  test("back behavior is explicit for Android stack/drawer routes", () => {
    expect(
      decideBackAction({ canExitRoute: true, canGoBack: false, isAndroid: true }),
    ).toBe("exit_app")
    expect(
      decideBackAction({ canExitRoute: false, canGoBack: true, isAndroid: true }),
    ).toBe("go_back")
    expect(
      decideBackAction({ canExitRoute: false, canGoBack: false, isAndroid: true }),
    ).toBe("ignore")
    expect(
      decideBackAction({ canExitRoute: true, canGoBack: true, isAndroid: false }),
    ).toBe("ignore")
  })

  test("active route helper returns only route names, not params", () => {
    const state = {
      index: 0,
      key: "root",
      routeNames: ["Home"],
      routes: [
        {
          key: "home",
          name: "Home",
          state: {
            index: 1,
            key: "drawer",
            routeNames: ["Threads", "Settings"],
            routes: [
              { key: "threads", name: "Threads" },
              {
                key: "settings",
                name: "Settings",
                params: {
                  token: "should-not-surface",
                },
              },
            ],
            stale: false,
            type: "drawer",
          },
        },
      ],
      stale: false,
      type: "stack",
    } as unknown as NavigationState

    expect(getActiveRouteName(state)).toBe("Settings")
    expect(routeNameSummary(state)).toEqual({ activeRouteName: "Settings" })
    expect(JSON.stringify(routeNameSummary(state))).not.toContain("should-not-surface")
  })

  test("navigation persistence is explicitly disabled until a safe snapshot exists", () => {
    expect(KHALA_NAVIGATION_PERSISTENCE_DECISION.enabled).toBe(false)
    expect(KHALA_NAVIGATION_PERSISTENCE_DECISION.reason).toContain("not persisted")
    expect(KHALA_NAVIGATION_PERSISTENCE_DECISION.reason).toContain("private")
  })
})

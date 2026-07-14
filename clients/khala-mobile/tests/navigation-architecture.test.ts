import { readFile } from "node:fs/promises"
import { describe, expect, test } from "vite-plus/test"

import appConfig from "../app.json"
import packageJson from "../package.json"

const mobileRoot = new URL("../", import.meta.url)

describe("Khala mobile navigation architecture", () => {
  test("uses an explicit app entry instead of Expo Router", () => {
    expect(packageJson.main).toBe("index.tsx")
    expect(packageJson.dependencies).not.toHaveProperty("expo-router")
    expect(appConfig.expo.plugins).not.toContain("expo-router")
    expect(appConfig.expo).not.toHaveProperty("experiments")
  })

  test("keeps route ownership in typed React Navigation sources", async () => {
    const legacyRouteFiles = await Array.fromAsync(
      glob("app/**/*.{ts,tsx}", { cwd: mobileRoot.pathname }),
    )
    expect(legacyRouteFiles).toEqual([])

    const [entry, app, navigator, navigationTypes] = await Promise.all([
      readFile(new URL("index.tsx", mobileRoot), "utf8"),
      readFile(new URL("src/app.tsx", mobileRoot), "utf8"),
      readFile(new URL("src/navigators/AppNavigator.tsx", mobileRoot), "utf8"),
      readFile(new URL("src/navigators/navigationTypes.ts", mobileRoot), "utf8"),
    ])

    expect(entry).toContain("registerRootComponent(App)")
    expect(app).toContain("<AppNavigator />")
    expect(navigator).toContain("createNativeStackNavigator<AppStackParamList>()")
    expect(navigator).toContain("createDrawerNavigator<AppDrawerParamList>()")
    expect(navigationTypes).toContain("ThreadMessages: {")
    expect([entry, app, navigator, navigationTypes].join("\n")).not.toContain("expo-router")
  })
})
import { glob } from "node:fs/promises"

import { describe, expect, test } from "bun:test"

import appJson from "../app.json"
import packageJson from "../package.json"

const mobileRoot = new URL("../", import.meta.url)
const read = (path: string) => Bun.file(new URL(path, mobileRoot)).text()

describe("Khala mobile local asset generation", () => {
  test("exposes a local-only asset generation script", async () => {
    expect(packageJson.scripts).toHaveProperty("assets:generate")
    expect(packageJson.scripts["assets:generate"]).toBe("bash scripts/generate-assets.sh")

    const script = await read("scripts/generate-assets.sh")
    expect(script).toContain("sips")
    expect(script).not.toContain("eas ")
    expect(script).not.toContain("expo submit")
  })

  test("app config points at generated local icon and splash assets", () => {
    expect(appJson.expo.icon).toBe("./assets/images/icon.png")
    expect(appJson.expo.splash).toEqual({
      backgroundColor: "#02060d",
      image: "./assets/images/splash-icon.png",
      resizeMode: "contain",
    })
    expect(appJson.expo.android.adaptiveIcon).toEqual({
      backgroundColor: "#02060d",
      foregroundImage: "./assets/images/adaptive-icon.png",
    })
  })
})

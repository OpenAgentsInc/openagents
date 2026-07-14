import { describe, expect, test } from "vite-plus/test"
import { readFileSync } from "node:fs"
import { resolve } from "node:path"

const root = resolve(import.meta.dirname, "../../..")
const catalogPath = "apps/native-sdk-effect-native-spike/assurance/mvp-assurance-criteria.test.ts"
const source = readFileSync(resolve(root, catalogPath), "utf8")

describe("Native SDK MVP criterion catalog", () => {
  test("owns one exact candidate/falsifier pair for every frozen criterion", () => {
    for (let index = 1; index <= 18; index += 1) {
      const criterion = `CW-AC-${String(index).padStart(2, "0")}`
      expect(source.match(new RegExp(`criterion: "${criterion}"`, "gu"))).toHaveLength(1)
    }
    expect(source).toContain("`${contract.criterion} candidate evidence remains bound`")
    expect(source).toContain("`${contract.criterion} missing-anchor falsifier is rejected`")
  })

  test("requires target integration and does not inherit Electron release evidence", () => {
    expect(source).toContain("contract.nativeIntegrationAnchors.length > 0")
    expect(source).not.toContain("rc9")
    expect(source).not.toContain("openagents-desktop-mvp")
    expect(source).not.toContain("candidate-receipt.md")
  })
})

import { describe, expect, test } from "bun:test"

import packageJson from "../package.json"

const mobileRoot = new URL("../", import.meta.url)
const read = (path: string) => Bun.file(new URL(path, mobileRoot)).text()

describe("Khala mobile architecture guardrails", () => {
  test("package exposes the local dependency-cruiser check", async () => {
    expect(packageJson.scripts).toHaveProperty("architecture:check")
    expect(packageJson.scripts["architecture:check"]).toContain("depcruise")
    expect(packageJson.devDependencies).toHaveProperty("dependency-cruiser")

    const config = await read(".dependency-cruiser.cjs")
    for (const ruleName of [
      "no-circular",
      "no-production-imports-from-tests",
      "no-non-package-json",
      "not-to-unresolvable",
      "not-to-dev-dep-from-production",
      "domain-must-not-import-routes",
      "native-modules-through-adapter",
    ]) {
      expect(config).toContain(ruleName)
    }
  })

  test("local templates cover screens, components, navigators, and contract oracles", async () => {
    const templates = await Promise.all([
      read("templates/screen/NAME-screen.tsx.ejs"),
      read("templates/component/NAME.tsx.ejs"),
      read("templates/component-test/NAME.test.tsx.ejs"),
      read("templates/component-stories/NAME.stories.tsx.ejs"),
      read("templates/navigator/NAMENavigator.tsx.ejs"),
      read("templates/screen-contract/NAME-contract.test.ts.ejs"),
      read("templates/screen-maestro/NAME-screen.yaml.ejs"),
      read("templates/screen-mount-test/NAME-screen.test.tsx.ejs"),
      read("templates/screen-stories/NAME-screen.stories.tsx.ejs"),
      read("templates/screen-visual/NAME-screen.ts.ejs"),
      read("templates/ux-contract-oracle/NAME.test.ts.ejs"),
      read("templates/README.md"),
    ])
    const joined = templates.join("\n")

    expect(joined).toContain("KhalaScreen")
    expect(joined).toContain("KhalaText")
    expect(joined).toContain("createNativeStackNavigator")
    expect(joined).toContain("khala_mobile.PRODUCT_AREA.BEHAVIOR.v1")
    expect(joined).toContain("does not depend on the Ignite CLI")
    expect(joined).toContain("visual-baseline registration")
    expect(joined).toContain("mount test")
  })
})

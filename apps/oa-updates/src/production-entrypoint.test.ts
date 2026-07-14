import { readFileSync } from "node:fs"
import { resolve } from "node:path"
import { describe, expect, test } from "vite-plus/test"

const appRoot = resolve(import.meta.dirname, "..")

describe("oa-updates production entrypoint", () => {
  test("builds and serves the seeding entrypoint rather than the bare registry", () => {
    const packageJson = JSON.parse(
      readFileSync(resolve(appRoot, "package.json"), "utf8"),
    ) as { scripts: Record<string, string> }

    expect(packageJson.scripts["build:server"]).toContain("src/serve.ts")
    expect(packageJson.scripts["serve"]).toContain("src/serve.ts")
    expect(packageJson.scripts["build:server"]).not.toContain("src/server.ts")
    expect(packageJson.scripts["serve"]).not.toContain("src/server.ts")

    for (const dockerfile of ["Dockerfile", "Dockerfile.incremental"]) {
      expect(readFileSync(resolve(appRoot, dockerfile), "utf8")).toContain(
        'CMD ["node", "dist-server/serve.mjs"]',
      )
    }

    expect(readFileSync(resolve(appRoot, "src/server.ts"), "utf8")).not.toContain(
      "Runtime.isMain(import.meta.url)",
    )
    expect(readFileSync(resolve(appRoot, "src/serve.ts"), "utf8")).toContain(
      "Runtime.isMain(import.meta.url)",
    )
  })
})

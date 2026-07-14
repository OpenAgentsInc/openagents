import { describe, expect, test } from "vite-plus/test"
import { readFileSync } from "node:fs"

const source = readFileSync(new URL("../scripts/run-mvp-assurance.ts", import.meta.url), "utf8")

describe("MVP assurance runner conversion contract", () => {
  test("binds the admitted run to Node 24, pnpm, and Vite Plus", () => {
    expect(source).toContain('runtime: "Node 24.13.1"')
    expect(source).toContain('"ENV-OA-DESKTOP-MVP-VITE-PLUS-1"')
    expect(source).toContain('adapter_ref: "openagents.vite_plus_test.v1"')
    expect(source).toContain('spawnSync("pnpm", ["--dir", "apps/openagents-desktop", "run", "verify"]')
    expect(source).not.toContain("ENV-OA-DESKTOP-MVP-BUN")
    expect(source).not.toContain("openagents.bun_test")
    expect(source).not.toContain("process.execPath")
    expect(source).not.toContain('includes("0 fail")')
  })
})

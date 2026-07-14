import { describe, expect, test } from "vite-plus/test"
import { readFileSync } from "node:fs"

const config = readFileSync(new URL("../vite.config.ts", import.meta.url), "utf8")

describe("Vite Plus repository isolation", () => {
  test("never discovers tests from nested agent worktrees", () => {
    expect(config).toContain('"**/.worktrees/**"')
    expect(config).toContain('"**/.claude/worktrees/**"')
  })
})

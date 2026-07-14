import assert from "node:assert/strict"
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { test } from "node:test"
import { execFileSync } from "node:child_process"

import { scanVp2NodeRuntime } from "./vp2-node-runtime-guard.mjs"

test("VP-2 guard rejects Bun runtime source and permits the named SQLite oracle", () => {
  const root = mkdtempSync(join(tmpdir(), "vp2-node-runtime-guard-"))
  try {
    execFileSync("git", ["init", "--quiet"], { cwd: root })
    const write = (path, source) => {
      const absolute = join(root, path)
      mkdirSync(join(absolute, ".."), { recursive: true })
      writeFileSync(absolute, source)
    }
    write("apps/example/src/server.ts", "Bun.serve({ fetch() {} })\n")
    write("packages/sqlite-runtime/src/bun-database.ts", 'import("bun:sqlite")\n')
    write("packages/example/src/server.test.ts", 'import { test } from "vite-plus/test"\n')
    execFileSync("git", ["add", "."], { cwd: root })

    assert.deepEqual(scanVp2NodeRuntime(root), [
      { category: "bun-global", path: "apps/example/src/server.ts", line: 1 },
    ])
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})

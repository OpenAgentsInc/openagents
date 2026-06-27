import { spawnSync } from "node:child_process"
import { mkdtempSync, readFileSync, rmSync, writeFileSync, chmodSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, test } from "node:test"
import assert from "node:assert/strict"

import {
  CODEX_RG_GUARD_EXCLUDE_GLOBS,
  guardedRipgrepArgs,
  installRipgrepGuard,
  sanitizeRipgrepArgs,
} from "./install-rg-guard.mjs"

describe("codex-fleet rg guard", () => {
  test("sanitizes unrestricted flags before end-of-options", () => {
    assert.deepEqual(sanitizeRipgrepArgs(["-uu", "--no-ignore", "--hidden", "needle", "--", "--hidden"]), [
      "needle",
      "--",
      "--hidden",
    ])
  })

  test("adds guard globs before end-of-options", () => {
    const args = guardedRipgrepArgs(["needle", ".", "--", "--no-ignore"])
    const endOfOptions = args.indexOf("--")
    assert.ok(endOfOptions > 0)
    for (const glob of CODEX_RG_GUARD_EXCLUDE_GLOBS) {
      const index = args.indexOf(glob)
      assert.ok(index > 0, `missing ${glob}`)
      assert.ok(index < endOfOptions, `${glob} must precede --`)
    }
    assert.deepEqual(args.slice(endOfOptions), ["--", "--no-ignore"])
  })

  test("installed wrapper delegates sanitized args to the real rg", () => {
    const root = mkdtempSync(join(tmpdir(), "codex-fleet-rg-guard-"))
    try {
      const realRg = join(root, "real-rg")
      const out = join(root, "args.json")
      writeFileSync(
        realRg,
        `#!/usr/bin/env bash
: > "$OUT"
for arg in "$@"; do
  printf '%s\\n' "$arg" >> "$OUT"
done
`,
      )
      chmodSync(realRg, 0o755)
      const binDir = join(root, "bin")
      installRipgrepGuard({ binDir, realRg })
      const result = spawnSync("rg", ["-uu", "--no-ignore-vcs", "needle", ".", "--", "--hidden"], {
        env: { ...process.env, PATH: `${binDir}:${process.env.PATH ?? ""}`, OUT: out },
        encoding: "utf8",
      })
      assert.equal(result.status, 0)
      const delegated = readFileSync(out, "utf8").trim().split("\n")
      assert.equal(delegated.includes("-uu"), false)
      assert.equal(delegated.includes("--no-ignore-vcs"), false)
      for (const glob of CODEX_RG_GUARD_EXCLUDE_GLOBS) assert.ok(delegated.includes(glob))
      assert.deepEqual(delegated.slice(-2), ["--", "--hidden"])
    } finally {
      rmSync(root, { force: true, recursive: true })
    }
  })
})

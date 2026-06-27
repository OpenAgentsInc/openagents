import { describe, expect, test } from "bun:test"
import { chmod, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { spawnSync } from "node:child_process"
import { join } from "node:path"
import { tmpdir } from "node:os"

import {
  CODEX_RG_GUARD_ENV,
  CODEX_RG_GUARD_EXCLUDE_GLOBS,
  guardedCodexRipgrepArgs,
  installCodexRipgrepGuard,
  sanitizeCodexRipgrepArgs,
} from "./codex-rg-guard.js"

describe("Codex ripgrep guard", () => {
  test("strips unrestricted traversal flags before end-of-options", () => {
    expect(sanitizeCodexRipgrepArgs(["-uu", "--no-ignore", "--hidden", "needle", "--", "--no-ignore"])).toEqual([
      "needle",
      "--",
      "--no-ignore",
    ])
  })

  test("appends hard exclude globs before end-of-options", () => {
    const args = guardedCodexRipgrepArgs(["needle", ".", "--", "--hidden"])
    const endOfOptions = args.indexOf("--")
    expect(endOfOptions).toBeGreaterThan(0)
    for (const glob of CODEX_RG_GUARD_EXCLUDE_GLOBS) {
      const index = args.indexOf(glob)
      expect(index).toBeGreaterThan(0)
      expect(index).toBeLessThan(endOfOptions)
    }
    expect(args.slice(endOfOptions)).toEqual(["--", "--hidden"])
  })

  test("installed wrapper delegates sanitized args to the real rg", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-rg-guard-"))
    try {
      const realRg = join(root, "real-rg")
      const out = join(root, "args.json")
      await writeFile(
        realRg,
        `#!/usr/bin/env bash
: > "$OUT"
for arg in "$@"; do
  printf '%s\\n' "$arg" >> "$OUT"
done
`,
      )
      await chmod(realRg, 0o755)
      const binDir = join(root, "bin")
      const installed = installCodexRipgrepGuard({
        env: { PATH: "/usr/bin:/bin" },
        binDir,
        realRipgrepPath: realRg,
      })

      expect(installed.installed).toBe(true)
      expect(installed.env[CODEX_RG_GUARD_ENV]).toBe("1")
      const result = spawnSync("rg", ["-uu", "--no-ignore", "needle", ".", "--", "--hidden"], {
        env: { ...process.env, ...installed.env, OUT: out },
        encoding: "utf8",
      })
      expect(result.status).toBe(0)
      const delegated = (await readFile(out, "utf8")).trim().split("\n")
      expect(delegated).not.toContain("-uu")
      expect(delegated).not.toContain("--no-ignore")
      for (const glob of CODEX_RG_GUARD_EXCLUDE_GLOBS) expect(delegated).toContain(glob)
      expect(delegated.slice(-2)).toEqual(["--", "--hidden"])
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })
})

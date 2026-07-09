import { describe, expect, test } from "bun:test"
import { chmod, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { spawnSync } from "node:child_process"
import { join } from "node:path"
import { tmpdir } from "node:os"

import {
  CODEX_RG_GUARD_ENV,
  CODEX_RG_GUARD_EXCLUDE_GLOBS,
  CODEX_WORKSPACE_ROOT_ENV,
  guardedCodexRipgrepArgs,
  installCodexRipgrepGuard,
  sanitizeCodexRipgrepArgs,
  scopeCodexFindArgsToWorkspace,
  scopeCodexRipgrepArgsToWorkspace,
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

  test("relativizes rg path args that escape the active workspace", () => {
    const workspace = "/Users/example/.pylon-fable/cache/codex-agent-tasks/workspace.active"
    expect(
      scopeCodexRipgrepArgsToWorkspace(
        ["-n", "marker", "/Users/example/.pylon-fable", "--glob", "*.ts"],
        workspace,
      ),
    ).toEqual(["-n", "marker", ".", "--glob", "*.ts"])
    expect(
      scopeCodexRipgrepArgsToWorkspace(["-n", "marker", `${workspace}/src`], workspace),
    ).toEqual(["-n", "marker", `${workspace}/src`])
    expect(
      scopeCodexRipgrepArgsToWorkspace(["-n", "-e", "marker", "/Users/example/.pylon-fable"], workspace),
    ).toEqual(["-n", "-e", "marker", "."])
  })

  test("relativizes find roots that escape the active workspace", () => {
    const workspace = "/Users/example/.pylon-fable/cache/codex-agent-tasks/workspace.active"
    expect(
      scopeCodexFindArgsToWorkspace(["/Users/example/.pylon-fable", "-name", "*.ts"], workspace),
    ).toEqual([".", "-name", "*.ts"])
    expect(
      scopeCodexFindArgsToWorkspace([`${workspace}/src`, "-name", "*.ts"], workspace),
    ).toEqual([`${workspace}/src`, "-name", "*.ts"])
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
      const result = spawnSync("rg", ["-uu", "--no-ignore", "needle", join(root, ".."), "--", "--hidden"], {
        env: { ...process.env, ...installed.env, [CODEX_WORKSPACE_ROOT_ENV]: root, OUT: out },
        encoding: "utf8",
      })
      expect(result.status).toBe(0)
      const delegated = (await readFile(out, "utf8")).trim().split("\n")
      expect(delegated).not.toContain("-uu")
      expect(delegated).not.toContain("--no-ignore")
      expect(delegated).not.toContain(join(root, ".."))
      expect(delegated).toContain(".")
      for (const glob of CODEX_RG_GUARD_EXCLUDE_GLOBS) expect(delegated).toContain(glob)
      expect(delegated.slice(-2)).toEqual(["--", "--hidden"])
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })

  test("installed wrapper scopes find roots to the workspace", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-find-guard-"))
    try {
      const realRg = join(root, "real-rg")
      await writeFile(realRg, "#!/usr/bin/env bash\nexit 0\n")
      await chmod(realRg, 0o755)
      const realFind = join(root, "real-find")
      const out = join(root, "find-args.json")
      await writeFile(
        realFind,
        `#!/usr/bin/env bash
: > "$OUT"
printf '%s\\n' "$PWD" >> "$OUT"
for arg in "$@"; do
  printf '%s\\n' "$arg" >> "$OUT"
done
`,
      )
      await chmod(realFind, 0o755)
      const binDir = join(root, "bin")
      const installed = installCodexRipgrepGuard({
        env: { PATH: "/usr/bin:/bin" },
        binDir,
        realFindPath: realFind,
        realRipgrepPath: realRg,
        workspaceRoot: root,
      })

      const result = spawnSync("find", [join(root, ".."), "-name", "*.ts"], {
        env: { ...process.env, ...installed.env, OUT: out },
        encoding: "utf8",
      })
      expect(result.status).toBe(0)
      const delegated = (await readFile(out, "utf8")).trim().split("\n")
      expect(delegated[0]).toBe(root)
      expect(delegated.slice(1)).toEqual([".", "-name", "*.ts"])
    } finally {
      await rm(root, { force: true, recursive: true })
    }
  })
})

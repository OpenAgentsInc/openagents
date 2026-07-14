import assert from "node:assert/strict"
import { spawnSync } from "node:child_process"
import { mkdtempSync, readFileSync, readdirSync, rmSync, statSync } from "node:fs"
import { tmpdir } from "node:os"
import { dirname, join, resolve } from "node:path"
import { test } from "node:test"

import { publicCliArtifacts } from "./public-cli-artifact-catalog.mjs"

const root = resolve(import.meta.dirname, "..")

test("seven staged public CLI packages contain compiled Node artifacts and no Bun runtime", () => {
  const stage = mkdtempSync(join(tmpdir(), "openagents-public-cli-"))
  try {
    const build = spawnSync(process.execPath, [join(root, "scripts/build-public-cli-artifacts.mjs"), "--stage-dir", stage], { cwd: root, encoding: "utf8" })
    assert.equal(build.status, 0, `${build.stdout}\n${build.stderr}`)
    assert.equal(publicCliArtifacts.length, 7)
    for (const record of publicCliArtifacts) {
      const packageRoot = join(stage, record.name.replace(/^@/, "").replaceAll("/", "__"))
      const manifest = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8"))
      assert.deepEqual(manifest.engines, { node: ">=24.10" })
      assert.equal(JSON.stringify(manifest).includes("workspace:"), false)
      for (const target of Object.values(manifest.bin)) {
        const path = join(packageRoot, target)
        assert.equal(statSync(path).isFile(), true)
        if (target.endsWith(".mjs")) {
          assert.match(readFileSync(path, "utf8"), /^#!\/usr\/bin\/env node\n/)
          const smoke = spawnSync(process.execPath, [path, "--help"], {
            cwd: packageRoot,
            encoding: "utf8",
            timeout: 15_000,
            env: { ...process.env, PATH: `${dirname(process.execPath)}:/usr/bin:/bin` },
          })
          const transcript = `${smoke.stdout ?? ""}\n${smoke.stderr ?? ""}`
          assert.notEqual(smoke.status, null, `${record.name}: CLI timed out`)
          assert.doesNotMatch(transcript, /Bun is not defined|Cannot find package '@openagentsinc\//)
        }
      }
      const files = walk(packageRoot)
      assert.equal(files.some((path) => /\.(ts|tsx)$/.test(path)), false, `${record.name} staged TypeScript`)
      for (const file of files.filter((path) => /\.(mjs|js|json)$/.test(path))) {
        const body = readFileSync(file, "utf8")
        assert.equal(/\bBun\.|(?:from|import\()\s*["']bun(?::|["'])|env bun/.test(body), false, `${record.name}: Bun residue in ${file}`)
      }
      for (const file of files.filter((path) => /\.d\.(?:mts|cts|ts)$/.test(path))) {
        const body = readFileSync(file, "utf8")
        const imports = [...body.matchAll(/(?:from|import\()\s*["'](@openagentsinc\/[^"'/]+)(?:\/[^"']*)?["']/g)]
        for (const match of imports) {
          assert.match(manifest.dependencies?.[match[1]] ?? "", /^\d+\.\d+\.\d+(?:[-+].*)?$/, `${record.name}: unresolved declaration dependency ${match[1]} in ${file}`)
        }
      }
    }
  } finally {
    rmSync(stage, { recursive: true, force: true })
  }
})

function walk(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name)
    return entry.isDirectory() ? walk(path) : [path]
  })
}

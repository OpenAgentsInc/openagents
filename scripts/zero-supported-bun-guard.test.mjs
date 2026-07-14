import assert from "node:assert/strict"
import { execFileSync, spawnSync } from "node:child_process"
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import test from "node:test"
import { fileURLToPath } from "node:url"

const guard = fileURLToPath(new URL("./zero-supported-bun-guard.mjs", import.meta.url))

const fixture = (files) => {
  const root = mkdtempSync(path.join(tmpdir(), "zero-supported-bun-"))
  execFileSync("git", ["init", "-q"], { cwd: root })
  for (const [file, contents] of Object.entries(files)) {
    mkdirSync(path.dirname(path.join(root, file)), { recursive: true })
    writeFileSync(path.join(root, file), contents)
  }
  execFileSync("git", ["add", "."], { cwd: root })
  return root
}

test("rejects Bun across executable, package, and authoritative documentation surfaces", () => {
  for (const [file, contents] of [
    ["scripts/deploy.sh", "#!/bin/sh\nbun run deploy\n"],
    ["package.json", '{"packageManager":"bun@1.2.0"}\n'],
    ["README.md", "Run `bun install`.\n"],
    ["src/runtime.ts", 'import { serve } from "bun"\n'],
    ["src/runner.ts", 'const command = [\n  "bun",\n  "run",\n  "check",\n]\n'],
  ]) {
    const result = spawnSync(process.execPath, [guard, fixture({ [file]: contents })], { encoding: "utf8" })
    assert.notEqual(result.status, 0, `${file} should fail`)
    assert.match(result.stderr, new RegExp(file.replaceAll(".", "\\.")))
  }
})

test("allows named historical, negative-fixture, and ecosystem compatibility evidence", () => {
  const root = fixture({
    "docs/transcripts/legacy.md": "bun run old-command\n",
    "apps/pylon/docs/proofs/old.json": '{"command":"bun test"}\n',
    "packages/pylon-core/src/executor/workspace-materializer.ts": 'const ignored = [".bun"]\n',
    "scripts/bun-api-perimeter-scan.ts": 'const forbidden = "Bun.serve"\n',
  })
  const result = spawnSync(process.execPath, [guard, root], { encoding: "utf8" })
  assert.equal(result.status, 0, result.stderr)
})

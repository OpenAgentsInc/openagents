import assert from "node:assert/strict"
import { spawnSync } from "node:child_process"
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import test from "node:test"
import { fileURLToPath } from "node:url"

const guard = fileURLToPath(new URL("./cli-invocability-guard.mjs", import.meta.url))

const fixture = (files) => {
  const root = mkdtempSync(path.join(tmpdir(), "cli-invocability-"))
  for (const [file, contents] of Object.entries(files)) {
    mkdirSync(path.dirname(path.join(root, file)), { recursive: true })
    writeFileSync(path.join(root, file), contents)
  }
  return root
}

const run = (root, ...flags) => spawnSync(process.execPath, [guard, root, ...flags], { encoding: "utf8" })

const strictCli = `const values = process.argv.slice(2)
for (const value of values) {
  if (!fields.has(value)) throw new Error(\`unsupported or incomplete argument \${value}\`)
}
`

const manifest = JSON.stringify({ scripts: { record: "node --import tsx src/record-cli.ts" } })

test("fails a package-script CLI that rejects the separator pnpm forwards", () => {
  const root = fixture({
    "apps/agent/package.json": manifest,
    "apps/agent/src/record-cli.ts": strictCli,
  })
  const result = run(root)
  assert.equal(result.status, 1, result.stdout)
  assert.match(result.stderr, /record-cli\.ts/u)
  assert.match(result.stderr, /if \(value === "--"\) continue/u)
})

test("accepts the same CLI once it skips the separator", () => {
  const root = fixture({
    "apps/agent/package.json": manifest,
    "apps/agent/src/record-cli.ts": `${strictCli}\nif (value === "--") continue\n`,
  })
  assert.equal(run(root).status, 0, run(root).stderr)
})

test("startsWith(\"--\") is not tolerance", () => {
  // The characters match but the meaning is the opposite. Reading this as safe is
  // what let the guard's first draft miss the defect it was written for.
  const root = fixture({
    "apps/agent/package.json": manifest,
    "apps/agent/src/record-cli.ts": `${strictCli}\nif (next.startsWith("--")) throw new Error("x")\n`,
  })
  assert.equal(run(root).status, 1, "startsWith must not count as skipping the token")
})

test("ignores argv parsers no package script runs", () => {
  const root = fixture({
    "apps/agent/package.json": JSON.stringify({ scripts: { start: "electron ." } }),
    "apps/agent/src/main.ts": strictCli,
  })
  assert.equal(run(root).status, 0, run(root).stderr)
})

test("ignores a package-script CLI that accepts any argument", () => {
  const root = fixture({
    "apps/agent/package.json": manifest,
    "apps/agent/src/record-cli.ts": "const values = process.argv.slice(2)\nconsole.log(values)\n",
  })
  assert.equal(run(root).status, 0, run(root).stderr)
})

test("baselines, refuses to launder a new finding, and requires a reason for an exception", () => {
  const root = fixture({
    "apps/agent/package.json": manifest,
    "apps/agent/src/record-cli.ts": strictCli,
  })
  assert.equal(run(root, "--seed").status, 0)
  assert.equal(run(root).status, 0, run(root).stderr)

  writeFileSync(path.join(root, "apps/agent/package.json"), JSON.stringify({
    scripts: { record: "node --import tsx src/record-cli.ts", audit: "node --import tsx src/audit-cli.ts" },
  }))
  writeFileSync(path.join(root, "apps/agent/src/audit-cli.ts"), strictCli)
  assert.equal(run(root, "--prune").status, 0)
  const afterPrune = run(root)
  assert.equal(afterPrune.status, 1, "prune must not absorb the new finding")
  assert.match(afterPrune.stderr, /audit-cli\.ts/u)

  writeFileSync(
    path.join(root, "scripts/cli-invocability-baseline.json"),
    JSON.stringify({ inheritedDebt: ["apps/agent/src/record-cli.ts"], allowed: [{ ref: "apps/agent/src/audit-cli.ts", reason: "short" }] }),
  )
  const unreasoned = run(root)
  assert.equal(unreasoned.status, 1)
  assert.match(unreasoned.stderr, /without a usable reason/u)
})

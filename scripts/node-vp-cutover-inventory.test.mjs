import assert from "node:assert/strict"
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { spawnSync } from "node:child_process"
import test from "node:test"

import {
  classifyFinding,
  collectInventory,
  compareWithBaseline,
  createBaseline,
  INVENTORY_SCHEMA,
} from "./node-vp-cutover-inventory.mjs"

const fixtureRepo = () => {
  const root = mkdtempSync(join(tmpdir(), "openagents-node-vp-inventory-"))
  spawnSync("git", ["init", "-q"], { cwd: root })
  const write = (path, contents) => {
    const absolute = join(root, path)
    mkdirSync(join(absolute, ".."), { recursive: true })
    writeFileSync(absolute, contents)
  }
  return { root, write }
}

test("classifies every cutover family into the owning phase", () => {
  assert.deepEqual(classifyFinding({ category: "bun-api", path: "apps/api/src/server.ts" }), {
    phase: "VP-2",
    disposition: "port-retained-runtime",
  })
  assert.equal(
    classifyFinding({ category: "bun-test", path: "packages/x/src/x.test.ts" }).phase,
    "VP-3",
  )
  assert.equal(classifyFinding({ category: "direct-tool", path: "package.json" }).phase, "VP-4")
  assert.equal(classifyFinding({ category: "runtime-image", path: "apps/api/Dockerfile" }).phase, "VP-5")
  assert.equal(
    classifyFinding({ category: "runtime-image", path: "apps/openagents.com/services/mdk-treasury/Dockerfile" }).phase,
    "VP-1",
  )
  assert.equal(
    classifyFinding({ category: "bun-api", path: "apps/api/fixtures/server.ts" }).phase,
    "VP-3",
  )
  assert.equal(
    classifyFinding({ category: "bun-command", path: "docs/sol/receipts/2026-07-14-toolchain-receipt.md" }).phase,
    "historical",
  )
  assert.equal(
    classifyFinding({ category: "bun-api", path: "clients/khala-code-desktop/src/bun/index.ts" }).phase,
    "VP-6",
  )
  assert.equal(
    classifyFinding({ category: "money-authority", path: "apps/api/src/payment.ts" }).disposition,
    "decommission-delete",
  )
  assert.equal(
    classifyFinding({ category: "money-authority", path: "apps/api/migrations/0001_payment.sql" }).disposition,
    "retain-read-only-evidence",
  )
  assert.equal(
    classifyFinding({ category: "money-negative-contract", path: "packages/contracts/src/no-spend.ts" }).disposition,
    "retain-negative-contract",
  )
})

test("inventory is complete, deterministic, and classified", () => {
  const { root, write } = fixtureRepo()
  try {
    write("apps/api/src/server.ts", 'export const serve = Bun.serve({})\n')
    write("apps/api/src/server.test.ts", 'import { test } from "bun:test"\n')
    write("apps/api/src/payment.ts", 'export const wallet = "wallet"\n')
    write("apps/api/migrations/0001_payment.sql", "CREATE TABLE payment_receipts(id TEXT);\n")
    write("apps/api/Dockerfile", "FROM oven/bun:1\n")
    write(
      "package.json",
      '{"scripts":{"test":"vitest"},"packageManager":"bun@1.3.11","engines":{"bun":">=1.3"},"devDependencies":{"bun-types":"1.3.11"}}\n',
    )
    write("bun.lock", "lockfileVersion = 1\n")
    spawnSync("git", ["add", "."], { cwd: root })

    const first = collectInventory(root)
    const second = collectInventory(root)
    assert.deepEqual(first, second)
    assert.equal(first.schema, INVENTORY_SCHEMA)
    assert.ok(first.entries.length > 0)
    assert.ok(first.entries.every((entry) => entry.phase && entry.disposition))
    assert.ok(first.entries.some((entry) => entry.phase === "VP-1"))
    assert.ok(first.entries.some((entry) => entry.phase === "VP-2"))
    assert.ok(first.entries.some((entry) => entry.phase === "VP-3"))
    assert.ok(first.entries.some((entry) => entry.phase === "VP-4"))
    assert.ok(first.entries.some((entry) => entry.phase === "VP-5"))
    assert.equal(
      first.entries.find((entry) => entry.category === "bun-package-authority")?.matches,
      3,
    )
    assert.ok(first.entries.some((entry) => entry.category === "migration-history"))
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})

test("freeze accepts burn-down and rejects new or duplicated authority", () => {
  const { root, write } = fixtureRepo()
  try {
    write("apps/api/src/server.ts", 'export const serve = Bun.serve({})\n')
    write("apps/api/migrations/0001_payment.sql", "CREATE TABLE payment_receipts(id TEXT);\n")
    spawnSync("git", ["add", "."], { cwd: root })
    const baseline = createBaseline(collectInventory(root), "fixture-base")
    assert.deepEqual(compareWithBaseline(collectInventory(root), baseline), [])

    write("apps/api/src/server.ts", "export const serve = 1\n")
    assert.deepEqual(compareWithBaseline(collectInventory(root), baseline), [])

    write("apps/api/src/server.ts", 'export const a = Bun.file("a")\nexport const b = Bun.file("a")\n')
    const growth = compareWithBaseline(collectInventory(root), baseline)
    assert.ok(growth.some((error) => error.startsWith("GROWTH ") || error.startsWith("NEW ")))

    write("apps/other/src/new.ts", 'import { Database } from "bun:sqlite"\n')
    spawnSync("git", ["add", "."], { cwd: root })
    const moved = compareWithBaseline(collectInventory(root), baseline)
    assert.ok(moved.some((error) => error.includes("apps/other/src/new.ts")))

    write("apps/api/migrations/0001_payment.sql", "ALTER TABLE payment_receipts ADD amount INTEGER;\n")
    const migrationChanged = compareWithBaseline(collectInventory(root), baseline)
    assert.ok(migrationChanged.some((error) => error.startsWith("MIGRATION_CHANGED ")))
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})

test("untracked files do not alter a source-commit inventory", () => {
  const { root, write } = fixtureRepo()
  try {
    write("apps/api/src/server.ts", "export const value = 1\n")
    spawnSync("git", ["add", "."], { cwd: root })
    const baseline = collectInventory(root)
    write("apps/api/src/untracked.ts", 'export const surprise = Bun.file("x")\n')
    assert.deepEqual(collectInventory(root), baseline)
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})

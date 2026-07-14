import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { resolve } from "node:path"
import { test } from "vite-plus/test"
import assert from "node:assert/strict"

import { typecheckTests } from "./typecheck-tests.mjs"

const withFixture = async (body) => {
  const root = await mkdtemp(resolve(tmpdir(), "pylon-test-typecheck-"))
  await mkdir(resolve(root, "tests"))
  await writeFile(
    resolve(root, "tsconfig.tests.json"),
    JSON.stringify({ compilerOptions: { strict: true, noEmit: true }, include: ["**/*.test.ts"] }),
  )
  try {
    await body(root)
  } finally {
    await rm(root, { recursive: true, force: true })
  }
}

test("a newly broken required fixture fails the checked-in baseline", async () => {
  await withFixture(async (root) => {
    const fixture = resolve(root, "tests", "fixture.test.ts")
    const baseline = resolve(root, "baseline.json")
    await writeFile(fixture, 'type Job = { required: string }; const job: Job = { required: "yes" };\n')
    await typecheckTests({ root, baseline, updateBaseline: true })
    await typecheckTests({ root, baseline })

    await writeFile(fixture, "type Job = { required: string }; const job: Job = {};\n")
    await assert.rejects(
      typecheckTests({ root, baseline }),
      /Pylon test typecheck baseline changed/,
    )
  })
})

test("the diagnostic baseline updater only permits shrinkage", async () => {
  await withFixture(async (root) => {
    const fixture = resolve(root, "tests", "fixture.test.ts")
    const baseline = resolve(root, "baseline.json")
    await writeFile(fixture, "type Job = { required: string }; const job: Job = {};\n")
    await typecheckTests({ root, baseline, updateBaseline: true })

    await writeFile(fixture, 'type Job = { required: string }; const job: Job = { required: "yes" };\n')
    await assert.rejects(typecheckTests({ root, baseline }), /baseline changed/)
    await typecheckTests({ root, baseline, updateBaseline: true })
    const updated = JSON.parse(await readFile(baseline, "utf8"))
    assert.equal(updated.diagnostics.length, 0)
    await typecheckTests({ root, baseline })
  })
})

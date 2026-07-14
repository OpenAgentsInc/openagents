import { spawnSync } from "node:child_process"
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { dirname, join, resolve } from "node:path"
import { afterEach, describe, expect, test } from "vite-plus/test"

const fixtureRoots: string[] = []
const pluginPath = resolve(import.meta.dirname, "../src/index.ts")

afterEach(() => {
  for (const root of fixtureRoots.splice(0)) rmSync(root, { force: true, recursive: true })
})

const runRule = (rule: string, filename: string, source: string) => {
  const root = mkdtempSync(join(tmpdir(), "openagents-oxlint-"))
  fixtureRoots.push(root)
  const sourcePath = join(root, filename)
  const configPath = join(root, ".oxlintrc.json")
  mkdirSync(dirname(sourcePath), { recursive: true })
  writeFileSync(sourcePath, source)
  writeFileSync(configPath, JSON.stringify({
    jsPlugins: [{ name: "openagents", specifier: pluginPath }],
    rules: { [`openagents/${rule}`]: "error" },
  }))
  return spawnSync("vp", ["lint", "--config", configPath, sourcePath], {
    cwd: resolve(import.meta.dirname, "../../.."),
    encoding: "utf8",
  })
}

const expectValid = (rule: string, filename: string, source: string) => {
  const result = runRule(rule, filename, source)
  expect(result.error).toBeUndefined()
  expect(result.status, `${result.stdout}\n${result.stderr}`).toBe(0)
}

const expectInvalid = (rule: string, filename: string, source: string) => {
  const result = runRule(rule, filename, source)
  expect(result.error).toBeUndefined()
  expect(result.status).not.toBe(0)
  expect(`${result.stdout}\n${result.stderr}`).toContain(`openagents(${rule})`)
  expect(`${result.stdout}\n${result.stderr}`).toMatch(/(?:AGENTS|INVARIANTS)\.md/u)
}

describe("oxlint-plugin-openagents invariant rules", () => {
  test("no-keyword-routing accepts typed selectors and rejects string intent routing", () => {
    expectValid("no-keyword-routing", "src/intent-router.ts", "export const route = (input: unknown) => selector.select(input)\n")
    expectInvalid("no-keyword-routing", "src/intent-router.ts", "export const route = (input: string) => input.includes('billing')\n")
  })

  test("no-manual-effect-runtime-in-tests accepts the shared harness and rejects manual runs", () => {
    expectValid("no-manual-effect-runtime-in-tests", "src/example.test.ts", "it.effect('works', () => Effect.void)\n")
    expectInvalid("no-manual-effect-runtime-in-tests", "src/example.test.ts", "test('works', async () => Effect.runPromise(program))\n")
  })

  test("schema-contract-runtime-free accepts schemas and rejects host imports", () => {
    expectValid("schema-contract-runtime-free", "packages/example-contract/src/index.ts", "import { Schema } from 'effect'\nexport const Id = Schema.String\n")
    expectInvalid("schema-contract-runtime-free", "packages/example-contract/src/index.ts", "import { readFile } from 'node:fs/promises'\nexport { readFile }\n")
  })

  test("subpath-only-imports accepts owned subpaths and rejects root barrels", () => {
    expectValid("subpath-only-imports", "src/example.ts", "import { Store } from '@openagentsinc/oa-infra/kv-store'\nvoid Store\n")
    expectInvalid("subpath-only-imports", "src/example.ts", "import { Store } from '@openagentsinc/oa-infra'\nvoid Store\n")
  })

  test("no-renderer-runtime-credentials accepts preload contracts and rejects provider SDKs", () => {
    expectValid("no-renderer-runtime-credentials", "apps/openagents-desktop/src/renderer/view.ts", "import type { DesktopBridge } from '../preload-contract'\nexport type { DesktopBridge }\n")
    expectInvalid("no-renderer-runtime-credentials", "apps/openagents-desktop/src/renderer/view.ts", "import OpenAI from 'openai'\nvoid OpenAI\n")
  })

  test("no-inline-schema-compile accepts hoisted compilers and rejects function-local compilation", () => {
    expectValid("no-inline-schema-compile", "src/schema.ts", "const decode = Schema.decodeUnknownSync(Message)\nexport const read = (value: unknown) => decode(value)\n")
    expectInvalid("no-inline-schema-compile", "src/schema.ts", "export const read = (value: unknown) => Schema.decodeUnknownSync(Message)(value)\n")
  })
})

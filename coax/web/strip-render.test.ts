import { expect, test } from "bun:test"
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { stripStage } from "./strip"

test("staging preserves a canonical package's exact shader bytes", () => {
  const stage = mkdtempSync(join(tmpdir(), "render-strip-"))
  const dir = join(stage, "assets/build/render", "a".repeat(64))
  mkdirSync(dir, { recursive: true })
  const source = "fn shader() {}\n"
  writeFileSync(join(dir, "m.wgsl"), source)
  expect(stripStage(stage)).toBe(1)
  expect(readFileSync(join(dir, "m.wgsl"), "utf8")).toBe(source)
})

test("a transform that would invalidate a package refuses staging", () => {
  const stage = mkdtempSync(join(tmpdir(), "render-strip-"))
  const dir = join(stage, "site/build/render", "b".repeat(64))
  mkdirSync(dir, { recursive: true })
  const source = "// a comment\nfn shader() {}\n"
  writeFileSync(join(dir, "m.wgsl"), source)
  expect(() => stripStage(stage)).toThrow("would invalidate a render package hash")
  expect(readFileSync(join(dir, "m.wgsl"), "utf8")).toBe(source)
})

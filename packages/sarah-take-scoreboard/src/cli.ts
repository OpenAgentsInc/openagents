#!/usr/bin/env bun
// score-take — Sarah Quality Scoreboard CLI (SQ-1 #8618).
//
//   bun packages/sarah-take-scoreboard/src/cli.ts validate <file.json...>
//   bun packages/sarah-take-scoreboard/src/cli.ts validate --dir docs/sarah/scoreboards
//   bun packages/sarah-take-scoreboard/src/cli.ts emit <file.json...> [--out-dir docs/sarah/scoreboards]
//
// `emit` validates each record, writes the canonical `<takeId>.json` and the
// rendered `<takeId>.md` beside it, and rebuilds `index.ndjson` from every
// scoreboard JSON in the output directory.
import { readdirSync, statSync } from "node:fs"
import { basename, dirname, join } from "node:path"

import {
  renderScoreboardMarkdown,
  toCanonicalJson,
  toNdjsonLine,
  validateTakeScoreboard,
} from "./index.ts"
import type { TakeScoreboard, ValidationIssue } from "./index.ts"

const isScoreboardJson = (path: string): boolean =>
  path.endsWith(".json") && basename(path) !== "index.ndjson"

const collectScoreboardFiles = (root: string): string[] => {
  const results: string[] = []
  const walk = (dir: string) => {
    for (const entry of readdirSync(dir)) {
      const path = join(dir, entry)
      if (statSync(path).isDirectory()) walk(path)
      else if (isScoreboardJson(path)) results.push(path)
    }
  }
  walk(root)
  return results.sort()
}

const printIssues = (issues: ValidationIssue[], level: "error" | "warn"): void => {
  for (const issue of issues) {
    const line = `  ${level} ${issue.code}: ${issue.message}`
    if (level === "error") console.error(line)
    else console.log(line)
  }
}

const validateFile = async (
  path: string,
): Promise<{ scoreboard?: TakeScoreboard; failed: boolean }> => {
  let parsed: unknown
  try {
    parsed = JSON.parse(await Bun.file(path).text())
  } catch (error) {
    console.error(`FAIL ${path}`)
    console.error(`  error invalid_json: ${error instanceof Error ? error.message : String(error)}`)
    return { failed: true }
  }
  const result = validateTakeScoreboard(parsed)
  if (!result.valid) {
    console.error(`FAIL ${path}`)
    printIssues(result.errors, "error")
    printIssues(result.warnings, "warn")
    return { failed: true }
  }
  const warningNote = result.warnings.length
    ? ` (${result.warnings.length} warning${result.warnings.length === 1 ? "" : "s"})`
    : ""
  console.log(`ok ${path}${warningNote}`)
  printIssues(result.warnings, "warn")
  return { scoreboard: result.scoreboard, failed: false }
}

const rebuildIndex = async (outDir: string): Promise<void> => {
  const lines: string[] = []
  for (const path of collectScoreboardFiles(outDir)) {
    const result = validateTakeScoreboard(JSON.parse(await Bun.file(path).text()))
    if (result.valid) lines.push(toNdjsonLine(result.scoreboard))
  }
  await Bun.write(join(outDir, "index.ndjson"), `${lines.join("\n")}\n`)
}

const main = async () => {
  const [command, ...rest] = process.argv.slice(2)

  if (command === "validate") {
    const dirIndex = rest.indexOf("--dir")
    const paths = dirIndex === -1 ? rest : collectScoreboardFiles(rest[dirIndex + 1] ?? ".")
    if (paths.length === 0) {
      console.error("usage: score-take validate <file.json...> | --dir <dir>")
      process.exit(2)
    }
    let failures = 0
    for (const path of paths) {
      const { failed } = await validateFile(path)
      if (failed) failures += 1
    }
    if (failures > 0) {
      console.error(`${failures} invalid scoreboard file(s).`)
      process.exit(1)
    }
    return
  }

  if (command === "emit") {
    const outDirIndex = rest.indexOf("--out-dir")
    const files = rest.filter(
      (arg, index) =>
        arg !== "--out-dir" && (outDirIndex === -1 || index !== outDirIndex + 1),
    )
    if (files.length === 0) {
      console.error("usage: score-take emit <file.json...> [--out-dir <dir>]")
      process.exit(2)
    }
    const outDirs = new Set<string>()
    for (const path of files) {
      const { scoreboard, failed } = await validateFile(path)
      if (failed || !scoreboard) process.exit(1)
      const outDir = outDirIndex === -1 ? dirname(path) : (rest[outDirIndex + 1] ?? ".")
      outDirs.add(outDir)
      const jsonPath = join(outDir, `${scoreboard.takeId}.json`)
      const markdownPath = join(outDir, `${scoreboard.takeId}.md`)
      await Bun.write(jsonPath, toCanonicalJson(scoreboard))
      await Bun.write(markdownPath, renderScoreboardMarkdown(scoreboard))
      console.log(`emitted ${jsonPath}`)
      console.log(`emitted ${markdownPath}`)
    }
    for (const outDir of outDirs) {
      await rebuildIndex(outDir)
      console.log(`rebuilt ${join(outDir, "index.ndjson")}`)
    }
    return
  }

  console.error("usage: score-take <validate|emit> ...")
  process.exit(2)
}

await main()

#!/usr/bin/env bun

import { spawnSync } from "node:child_process"
import { readFileSync } from "node:fs"

const tracked = spawnSync("git", ["ls-files"], {
  encoding: "utf8",
  stdio: ["ignore", "pipe", "pipe"],
})

if (tracked.status !== 0) {
  process.stderr.write(tracked.stderr || "failed to list tracked files\n")
  process.exit(tracked.status ?? 1)
}

const conflictBlockPattern = /^<<<<<<<(?: .*)?$\n[\s\S]*?^=======$\n[\s\S]*?^>>>>>>>(?: .*)?$/m
const offenders = []

for (const path of tracked.stdout.split("\n")) {
  if (path === "") continue
  let text
  try {
    text = readFileSync(path, "utf8")
  } catch {
    continue
  }
  if (conflictBlockPattern.test(text)) offenders.push(path)
}

if (offenders.length > 0) {
  process.stderr.write(`conflict markers found:\n${offenders.join("\n")}\n`)
  process.exit(1)
}

process.stdout.write("no conflict markers found\n")

#!/usr/bin/env bun

import { spawnSync } from "node:child_process"
import { dirname, resolve } from "node:path"
import { fileURLToPath } from "node:url"

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..")
const appRoot = resolve(repoRoot, "apps/openagents.com")
const scriptPath = resolve(appRoot, "scripts/check-conflict-markers.mjs")

const result = spawnSync("bun", [scriptPath], {
  cwd: appRoot,
  stdio: "inherit",
})

if (result.error) {
  console.error(result.error.message)
  process.exit(1)
}

process.exit(result.status ?? 1)

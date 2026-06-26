#!/usr/bin/env bun

import { runKhalaCli } from "./cli.js"

if (import.meta.main) {
  const exitCode = await runKhalaCli(Bun.argv.slice(2))
  process.exit(exitCode)
}

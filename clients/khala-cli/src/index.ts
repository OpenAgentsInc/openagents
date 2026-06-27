#!/usr/bin/env node

import { runKhalaCli } from "./cli.js"

// Bin entry point. Runs under both node (the common `npm i -g` case) and bun.
// We intentionally do NOT guard on `import.meta.main`: this module is only ever
// executed directly as the `khala` binary (library consumers import from the
// individual modules), and the guard breaks when npm exposes the bin via a
// symlink whose path differs from `import.meta.url`.
const exitCode = await runKhalaCli(process.argv.slice(2))
process.exit(exitCode)

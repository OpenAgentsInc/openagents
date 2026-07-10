#!/usr/bin/env bun
import { runAgentReadinessCli } from "./index.js"

const exitCode = await runAgentReadinessCli(process.argv.slice(2))
process.exit(exitCode)

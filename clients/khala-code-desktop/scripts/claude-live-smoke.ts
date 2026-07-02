#!/usr/bin/env bun
import { runKhalaCodeClaudeLiveSmoke } from "../src/bun/claude-live-smoke.js"

const args = Bun.argv.slice(2)
const result = await runKhalaCodeClaudeLiveSmoke({
  requireLive: args.includes("--require-live"),
})

console.log(JSON.stringify(result, null, 2))
if (!result.ok) process.exit(1)

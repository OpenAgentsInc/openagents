#!/usr/bin/env bun
import {
  runKhalaCodeCodexParityLiveSmoke,
} from "../src/bun/codex-parity-live-smoke.js"

const args = Bun.argv.slice(2)
const result = await runKhalaCodeCodexParityLiveSmoke({
  env: Bun.env,
  requireLive: args.includes("--require-live") ||
    Bun.env.KHALA_CODE_DESKTOP_CODEX_PARITY_LIVE_SMOKE === "1",
})

console.log(JSON.stringify(result, null, 2))
if (!result.ok) process.exit(1)

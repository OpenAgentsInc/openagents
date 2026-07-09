#!/usr/bin/env bun
/**
 * Live RL-1 probe: measure max concurrent free-CLI Grok workers.
 *
 * Usage:
 *   bun packages/grok-harness/scripts/rl-probe.ts
 *   bun packages/grok-harness/scripts/rl-probe.ts --concurrency 1,2,4,8,12
 *   bun packages/grok-harness/scripts/rl-probe.ts --out docs/grok/receipts/rl.json
 */

import { resolve } from "node:path"

import {
  defaultRlReceiptPath,
  runRlProbe,
  writeRlProbeReceipt,
} from "../src/rate-limit-probe.ts"

function parseArgs(argv: string[]) {
  let concurrency = [1, 2, 4, 8]
  let out: string | undefined
  let prompt: string | undefined
  let model: string | undefined

  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]
    if (a === "--concurrency" && argv[i + 1]) {
      concurrency = argv[++i]!.split(",").map((s) => Number(s.trim())).filter((n) => n > 0)
    } else if (a === "--out" && argv[i + 1]) {
      out = argv[++i]
    } else if (a === "--prompt" && argv[i + 1]) {
      prompt = argv[++i]
    } else if (a === "--model" && argv[i + 1]) {
      model = argv[++i]
    }
  }

  return { concurrency, out, prompt, model }
}

const args = parseArgs(process.argv.slice(2))
const receipt = await runRlProbe({
  concurrencies: args.concurrency,
  ...(args.prompt === undefined ? {} : { prompt: args.prompt }),
  ...(args.model === undefined ? {} : { model: args.model }),
})

const outPath = resolve(args.out ?? defaultRlReceiptPath())
await writeRlProbeReceipt(receipt, outPath)

console.log(
  JSON.stringify(
    {
      outPath,
      plane: receipt.plane,
      marginalCostClass: receipt.marginalCostClass,
      maxFullSuccessConcurrency: receipt.maxFullSuccessConcurrency,
      maxPartialSuccessConcurrency: receipt.maxPartialSuccessConcurrency,
      summary: receipt.concurrencies.map((c) => ({
        concurrency: c.concurrency,
        success: c.successCount,
        fail: c.failureCount,
        rateLimited: c.rateLimitedCount,
        wallClockMs: c.wallClockMs,
      })),
    },
    null,
    2,
  ),
)

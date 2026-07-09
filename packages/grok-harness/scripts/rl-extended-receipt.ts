#!/usr/bin/env bun
/**
 * Write the MH-4 extended RL matrix receipt (RL-3/5/6 policy + measured floors).
 *
 * Usage:
 *   bun packages/grok-harness/scripts/rl-extended-receipt.ts
 *   GROK_RL3_ACCOUNT_IDS=a,b bun packages/grok-harness/scripts/rl-extended-receipt.ts
 */

import { mkdir, writeFile } from "node:fs/promises"
import { join } from "node:path"

import { buildRlExtendedMatrixReceipt } from "../src/rl-extended-probes.ts"

const accountIds = (process.env.GROK_RL3_ACCOUNT_IDS ?? "")
  .split(",")
  .map((s) => s.trim())
  .filter(Boolean)

const freeWindowActive = process.env.GROK_FREE_WINDOW_ACTIVE !== "0"
const wasFree = process.env.GROK_WAS_FREE !== "0"

const receipt = buildRlExtendedMatrixReceipt({
  rl1MaxFullSuccessConcurrency: Number(
    process.env.GROK_RL1_MAX_FULL ?? 48,
  ),
  rl4MaxFullSuccessConcurrency: Number(
    process.env.GROK_RL4_MAX_FULL ?? 4,
  ),
  rl3AccountIds: accountIds,
  freeWindowActive,
  wasFree,
})

const outDir = join(process.cwd(), "docs/grok/receipts")
await mkdir(outDir, { recursive: true })
const outPath = join(outDir, "rl-extended-matrix-2026-07-09.json")
await writeFile(outPath, `${JSON.stringify(receipt, null, 2)}\n`, "utf8")
console.log(`wrote ${outPath}`)
console.log(
  `RL-3 runnable=${receipt.rl3.runnable} RL-5 capObserved=${receipt.rl5.calendarCapObserved} RL-6 flip=${receipt.rl6.flip} class=${receipt.rl6.marginalCostClass}`,
)

#!/usr/bin/env bun

// #4866: repeatable stranger-buyer NIP-90 probe smoke (Orrery probe shape,
// platform-side). No-spend by default: publish one bounded kind-5050 request
// from a throwaway customer key, collect responses within a budget, map every
// responder to registered capacity via the #4864 /api/pylons provider fields,
// and emit a redaction-safe typed artifact. The paid leg refuses without BOTH
// --paid and PYLON_STRANGER_PROBE_ALLOW_SPEND=1.

import { mkdir, writeFile } from "node:fs/promises"
import { dirname } from "node:path"
import { OPENAGENTS_MARKET_RELAY_URL } from "../src/provider-nip90"
import {
  DEFAULT_STRANGER_PROBE_BASE_URL,
  DEFAULT_STRANGER_PROBE_BID_MSATS,
  DEFAULT_STRANGER_PROBE_COLLECT_BUDGET_MS,
  runStrangerProbe,
} from "../src/stranger-probe"
import { defaultWalletCommandRunner } from "../src/wallet"

function flagValue(args: string[], name: string) {
  const index = args.indexOf(name)
  return index >= 0 && index + 1 < args.length ? args[index + 1] : undefined
}

function positiveInt(value: string | undefined, fallback: number) {
  if (value === undefined || value.trim() === "") return fallback
  const parsed = Number(value)
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`expected a positive integer, got ${value}`)
  }
  return parsed
}

const args = process.argv.slice(2)
const paidFlag = args.includes("--paid")
const relayUrl =
  flagValue(args, "--relay") ?? (Bun.env.PYLON_STRANGER_PROBE_RELAY?.trim() || OPENAGENTS_MARKET_RELAY_URL)
const baseUrl =
  flagValue(args, "--base-url") ?? (Bun.env.OPENAGENTS_BASE_URL?.trim() || DEFAULT_STRANGER_PROBE_BASE_URL)
const bidMsats = positiveInt(
  flagValue(args, "--bid-msats") ?? Bun.env.PYLON_STRANGER_PROBE_BID_MSATS,
  DEFAULT_STRANGER_PROBE_BID_MSATS,
)
const collectBudgetMs = positiveInt(
  flagValue(args, "--budget-ms") ?? Bun.env.PYLON_STRANGER_PROBE_BUDGET_MS,
  DEFAULT_STRANGER_PROBE_COLLECT_BUDGET_MS,
)
const outPath = flagValue(args, "--out")

try {
  const artifact = await runStrangerProbe({
    relayUrl,
    baseUrl,
    bidMsats,
    collectBudgetMs,
    paidFlag,
    // The wallet runner is only ever invoked when the paid-leg double gate
    // (explicit --paid AND PYLON_STRANGER_PROBE_ALLOW_SPEND=1) authorized it.
    walletRunner: defaultWalletCommandRunner,
  })
  const serialized = `${JSON.stringify(artifact, null, 2)}\n`
  if (outPath !== undefined) {
    await mkdir(dirname(outPath), { recursive: true })
    await writeFile(outPath, serialized)
  }
  process.stdout.write(serialized)
  process.exitCode = artifact.verdict.status === "passed" ? 0 : 2
} catch (error) {
  process.stderr.write(
    `Stranger probe smoke failed: ${error instanceof Error ? error.message : String(error)}\n`,
  )
  process.exitCode = 1
}

#!/usr/bin/env bun

// #4866: long-running NIP-90 provider serve entrypoint. Runs the persistent
// provider loop against the configured market relays using the operator's
// real Pylon home (the registered identity), the default local Apple FM
// runtime for kind-5050 text inference, and the MDK agent wallet for
// payment-required quotes. No-spend by construction: the provider only ever
// issues receive invoices; it never pays anything.
//
// Env knobs (all optional):
//   PYLON_HOME                  Pylon state home (default ~/.pylon)
//   PYLON_NIP90_RELAYS          comma-separated relays (default canonical
//                               wss://relay.openagents.com)
//   PYLON_NIP90_PRICE_MSATS     price floor (default 1000)
//   PYLON_MDK_WALLET_HOME       HOME override applied to the MDK agent
//                               wallet subprocess only, for hosts where the
//                               default wallet home cannot create invoices
//   PYLON_MDK_WALLET_TIMEOUT_MS wallet subprocess timeout (default 15000)
//
// Run detached, e.g.:
//   nohup bun scripts/nip90-provider-serve.ts > /tmp/pylon-nip90-serve.log 2>&1 &

import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { startNip90ProviderLoop } from "../src/provider-nip90"
import type { WalletCommandRunner } from "../src/wallet"

function makeHomeScopedWalletRunner(home: string, timeoutMs: number): WalletCommandRunner {
  return async (args) => {
    const proc = Bun.spawn(["npx", "--yes", "@moneydevkit/agent-wallet@latest", ...args], {
      stdout: "pipe",
      stderr: "pipe",
      env: { ...process.env, HOME: home },
    })
    const timeout = new Promise<never>((_, reject) =>
      setTimeout(() => {
        proc.kill()
        reject(new Error("MDK agent-wallet command timed out"))
      }, timeoutMs),
    )
    const [stdout, stderr, exitCode] = await Promise.race([
      Promise.all([
        new Response(proc.stdout).text(),
        new Response(proc.stderr).text(),
        proc.exited,
      ]),
      timeout,
    ])
    return { exitCode, stdout, stderr }
  }
}

const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
const walletHome = Bun.env.PYLON_MDK_WALLET_HOME?.trim()
const walletTimeoutMs = Number(Bun.env.PYLON_MDK_WALLET_TIMEOUT_MS ?? 15_000)
const log = (message: string) => process.stdout.write(`[${new Date().toISOString()}] ${message}\n`)

log(`[NIP-90] Serve entrypoint starting (pid ${process.pid}).`)
try {
  const result = await startNip90ProviderLoop(summary, {
    ...(walletHome ? { walletRunner: makeHomeScopedWalletRunner(walletHome, walletTimeoutMs) } : {}),
    log,
  })
  if (!result.started) {
    log(`[NIP-90] Provider loop did not start: ${result.reasonRef}.`)
    process.exitCode = 2
  } else {
    log(`[NIP-90] Provider loop ended (handled: ${result.handled}).`)
  }
} catch (error) {
  log(`[NIP-90] Provider loop crashed: ${error instanceof Error ? error.message : String(error)}`)
  process.exitCode = 1
}

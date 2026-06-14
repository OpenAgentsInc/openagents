#!/usr/bin/env bun
// One-shot driver for the first live negotiated labor job (#4777). Invokes the
// exact provider loop code path (handleLaborMarketEventOnce) on the REAL,
// market-key-signed kind-7000 acceptance event already on the relay, with the
// provider's own local agent (codex) executing in a bounded sandbox. Output
// only: it publishes a kind-6934 result; it never moves funds.
//
// Env: PYLON_HOME (provider home), PYLON_NIP90_RELAYS (canonical relay).

import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { WebSocketRelayTransport } from "../src/provider-nip90"
import { ensurePylonLocalState } from "../src/state"
import { loadOrCreateNostrIdentity } from "../src/nostr-identity"
import { handleLaborMarketEventOnce, DEFAULT_LABOR_MARKET_POLICY } from "../src/labor-market"

const PROVIDER_PUBKEY = "3fd9b3f1e02122c68426ea27495e115ec9e8a592ef544fa6d04c98cd2b59c94a"
const RELAY = (Bun.env.PYLON_NIP90_RELAYS ?? "wss://relay.openagents.com").split(",")[0]!.trim()
const log = (m: string) => process.stdout.write(`[drive] ${m}\n`)

// Pull the most recent kind-7000 acceptance feedback targeting the provider.
async function fetchAcceptanceEvent(): Promise<any> {
  return await new Promise((resolve, reject) => {
    const ws = new WebSocket(RELAY)
    let best: any = null
    const timer = setTimeout(() => {
      try { ws.close() } catch {}
      best ? resolve(best) : reject(new Error("no acceptance event found on relay"))
    }, 8000)
    ws.onopen = () =>
      ws.send(JSON.stringify(["REQ", "acc", { kinds: [7000], "#p": [PROVIDER_PUBKEY], limit: 20 }]))
    ws.onmessage = (e: any) => {
      const m = JSON.parse(String(e.data))
      if (m[0] === "EVENT") {
        const ev = m[2]
        const ft = ev.tags.find((t: any) => t[0] === "lbr_feedback_type")?.[1]
        if (ft === "acceptance" && (best === null || ev.created_at > best.created_at)) best = ev
      }
      if (m[0] === "EOSE") {
        clearTimeout(timer)
        try { ws.close() } catch {}
        best ? resolve(best) : reject(new Error("no acceptance event found on relay"))
      }
    }
    ws.onerror = (err: any) => { clearTimeout(timer); reject(err) }
  })
}

const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
const state = await ensurePylonLocalState(summary)
const identity = await loadOrCreateNostrIdentity(summary.paths)
log(`provider pubkey ${identity.publicKey}`)
if (identity.publicKey !== PROVIDER_PUBKEY) {
  log(`WARNING: identity pubkey mismatch (PYLON_HOME=${summary.paths.home})`)
}

const acceptance = await fetchAcceptanceEvent()
log(`acceptance event ${acceptance.id} created_at=${acceptance.created_at}`)

const relay = new WebSocketRelayTransport(RELAY)
log(`executing labor job via codex in bounded sandbox (this can take minutes)...`)
const result = await handleLaborMarketEventOnce({
  state,
  event: acceptance,
  identity,
  relay,
  options: {
    policy: { ...DEFAULT_LABOR_MARKET_POLICY, agentKind: "codex" },
  },
})
log(`result: ${JSON.stringify(result)}`)
await relay.close?.()
process.exit(0)

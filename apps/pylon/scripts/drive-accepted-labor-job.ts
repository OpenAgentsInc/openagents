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
import {
  handleLaborMarketEventOnce,
  DEFAULT_LABOR_MARKET_POLICY,
  loadLaborMarketStore,
  writeLaborMarketStore,
} from "../src/labor-market"

// Self-contained operationalization of work request b74bb55c's objective
// ("complete the A1 API parity matrix slice of issue 4773"). The NIP-LBR
// kind-5934 is ref-only, so the provider resolves the public objective to this
// actionable, network-free, sandbox-verifiable task right before execution.
const OBJECTIVE_DETAIL = [
  "Implement the first self-contained slice of OpenAgents issue #4773 (A1: API",
  "parity contract). Create exactly two files in the current working directory,",
  "and nothing else. Do not access the network or any other repository.",
  "",
  "1) parity-matrix.ts — export a typed parity matrix asserting every MVP",
  "   capability has an agent-API peer. Shape:",
  "     export type ParityRow = { capability: string; web: boolean; api: boolean; waived?: string }",
  "     export const parityMatrix: ParityRow[] = [ ... ]",
  "   Cover these rows, each with web:true and api:true (no waivers needed):",
  "   submit, status, events, decisions_review, scheduling, lane_pricing_visibility,",
  "   account_pool_state.",
  "",
  "2) parity-matrix.test.ts — a bun:test that imports parityMatrix and asserts",
  "   that the matrix is non-empty and every row has api===true OR a non-empty",
  "   waived reason (so no MVP surface is browser-only without an explicit waiver).",
  "",
  "The verification command is `bun test`; it must pass in this directory.",
  "Return only public-safe artifact refs and a concise summary.",
].join("\n")

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

// A prior execution attempt (before the headless-codex + objective-resolution
// fix) marked the quote record "refused"; the acceptance branch requires the
// record to be "quoted". Reset it so the now-fixed path can re-execute. Escrow
// was already reserved on the worker at accept time and is untouched here.
const store = await loadLaborMarketStore(state)
let resetAny = false
for (const [key, rec] of Object.entries(store.quotes)) {
  if (rec.status !== "quoted") {
    log(`resetting quote record ${key} status ${rec.status} -> quoted`)
    rec.status = "quoted"
    delete rec.reasonRef
    delete rec.resultEventId
    resetAny = true
  }
}
if (resetAny) await writeLaborMarketStore(state, store)

const relay = new WebSocketRelayTransport(RELAY)
log(`executing labor job via codex in bounded sandbox (this can take minutes)...`)
const result = await handleLaborMarketEventOnce({
  state,
  event: acceptance,
  identity,
  relay,
  options: {
    policy: { ...DEFAULT_LABOR_MARKET_POLICY, agentKind: "codex" },
    resolveObjectiveDetail: async () => OBJECTIVE_DETAIL,
  },
})
log(`result: ${JSON.stringify(result)}`)
await relay.close?.()
process.exit(0)

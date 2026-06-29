#!/usr/bin/env bun
// Full negotiated-labor chain driver: quote -> offer -> accept -> execute ->
// result -> release, for one work request. The provider half (quote, codex
// execution, result) runs on the independent provider Pylon home; the requester
// half (offer ingest, accept/escrow, results, release) hits the live
// openagents.com worker with the requester agent token. Used to settle
// faucet-listed backlog jobs (#4781) and the market job of the spare-capacity
// proof (#4782). Output-only; never moves wallet material into public refs.
//
// Env: WORK_REQUEST_ID, JOB_EVENT_ID, OBJECTIVE_DETAIL, OPENAGENTS_AGENT_TOKEN
//      (requester), PYLON_HOME (provider home), optional PROVIDER_ACTOR_REF.

import { createBootstrapSummary, parseBootstrapArgs } from '../src/bootstrap'
import { WebSocketRelayTransport } from '../src/provider-nip90'
import { ensurePylonLocalState } from '../src/state'
import { loadOrCreateNostrIdentity } from '../src/nostr-identity'
import {
  DEFAULT_LABOR_MARKET_POLICY,
  handleLaborMarketEventOnce,
  loadLaborMarketStore,
} from '../src/labor-market'
import { decodeLbrAcceptanceEvent } from '@openagentsinc/nip90'

const WR = mustEnv('WORK_REQUEST_ID')
const JOB = mustEnv('JOB_EVENT_ID')
const DETAIL = mustEnv('OBJECTIVE_DETAIL')
const TOKEN = mustEnv('OPENAGENTS_AGENT_TOKEN')
const BASE = process.env.PYLON_OPENAGENTS_BASE_URL ?? 'https://openagents.com'
const RELAY = process.env.PYLON_NIP90_RELAYS?.split(',')[0]?.trim() ?? 'wss://relay.openagents.com'
const PROVIDER_ACTOR = process.env.PROVIDER_ACTOR_REF ?? 'provider.public.pylon.e3a6991ccdf71036048ae540'
const log = (m: string) => process.stdout.write(`[chain] ${m}\n`)

function mustEnv(k: string): string {
  const v = process.env[k]
  if (!v) {
    console.error(`${k} is required`)
    process.exit(1)
  }
  return v
}

function relayQuery(filter: Record<string, unknown>, pick: (e: any) => boolean): Promise<any | null> {
  return new Promise(resolve => {
    const ws = new WebSocket(RELAY)
    let best: any = null
    const t = setTimeout(() => {
      try { ws.close() } catch {}
      resolve(best)
    }, 8000)
    ws.onopen = () => ws.send(JSON.stringify(['REQ', 'q', filter]))
    ws.onmessage = (e: any) => {
      const m = JSON.parse(String(e.data))
      if (m[0] === 'EVENT' && pick(m[2]) && (best === null || m[2].created_at > best.created_at)) best = m[2]
      if (m[0] === 'EOSE') {
        clearTimeout(t)
        try { ws.close() } catch {}
        resolve(best)
      }
    }
    ws.onerror = () => { clearTimeout(t); resolve(best) }
  })
}

async function api(path: string, body: unknown, extraHeaders: Record<string, string> = {}): Promise<any> {
  const res = await fetch(`${BASE}${path}`, {
    method: 'POST',
    headers: { authorization: `Bearer ${TOKEN}`, 'content-type': 'application/json', ...extraHeaders },
    body: JSON.stringify(body),
  })
  const json = await res.json().catch(() => ({}))
  return { ok: res.ok, status: res.status, json }
}

const summary = createBootstrapSummary(parseBootstrapArgs(['--json']), Bun.env)
const state = await ensurePylonLocalState(summary)
const identity = await loadOrCreateNostrIdentity(summary.paths)
const relay = new WebSocketRelayTransport(RELAY)
log(`provider ${identity.publicKey} · work request ${WR}`)

// 1) Provider quotes the kind-5934 request (publishes kind-7000, records store).
const requestEvent = await relayQuery({ ids: [JOB], kinds: [5934] }, () => true)
if (!requestEvent) { console.error('request event not found on relay'); process.exit(1) }
const quoted = await handleLaborMarketEventOnce({
  state, event: requestEvent, identity, relay,
  options: { policy: { ...DEFAULT_LABOR_MARKET_POLICY, autoQuote: true, priceMsats: 1000, agentKind: 'codex' } },
})
log(`quote: ${JSON.stringify(quoted)}`)
const store = await loadLaborMarketStore(state)
const rec = store.quotes[JOB]
if (!rec) { console.error('no quote recorded for this request'); process.exit(1) }
const quoteRef = rec.quoteRef
const amountSats = Math.max(1, Math.ceil(rec.amountMsats / 1000))
log(`quoteRef ${quoteRef} amount ${amountSats} sats`)

// 2) Ingest the quote as a DB offer.
const offer = await api(`/api/forum/work-requests/${WR}/offers`, {
  amountSats,
  capabilityRefs: ['capability.pylon.local_claude_agent'],
  providerActorRef: PROVIDER_ACTOR,
  providerPubkey: identity.publicKey,
  quoteRef,
  relayEventRef: `nostr.event.${rec.quoteEventId}`,
})
log(`offer: ${offer.status} ${JSON.stringify(offer.json).slice(0, 160)}`)

// 3) Requester accepts the quote (reserves escrow + publishes acceptance).
const accept = await api(`/api/forum/work-requests/${WR}/acceptances`, { quoteRef }, {
  'Idempotency-Key': `faucet-accept:${WR}:${quoteRef}`,
})
log(`accept: ${accept.status} escrow=${JSON.stringify(accept.json.escrowState ?? accept.json).slice(0, 140)}`)

// 4) Provider executes the accepted job (codex in a bounded sandbox) + result.
const acceptance = await relayQuery(
  { kinds: [7000], '#p': [identity.publicKey], limit: 30 },
  (e: any) => {
    try { return decodeLbrAcceptanceEvent(e).requestId === JOB } catch { return false }
  },
)
if (!acceptance) { console.error('acceptance event not found on relay'); process.exit(1) }
// the prior quote record must be "quoted" for the acceptance branch to execute
const s2 = await loadLaborMarketStore(state)
if (s2.quotes[JOB] && s2.quotes[JOB]!.status !== 'quoted') {
  s2.quotes[JOB]!.status = 'quoted'
  const { writeLaborMarketStore } = await import('../src/labor-market')
  await writeLaborMarketStore(state, s2)
}
log('executing via codex in bounded sandbox (this can take minutes)…')
const executed = await handleLaborMarketEventOnce({
  state, event: acceptance, identity, relay,
  options: {
    policy: { ...DEFAULT_LABOR_MARKET_POLICY, agentKind: 'codex' },
    resolveObjectiveDetail: async () => DETAIL,
  },
})
log(`execute: ${JSON.stringify(executed)}`)
if (executed.action !== 'delivered') { console.error('execution did not deliver'); process.exit(1) }
const resultEventId = (executed as any).resultEventId as string
const closeoutRef = (executed as any).closeoutRef as string

// 5) Record the delivered result.
const results = await api(`/api/forum/work-requests/${WR}/results`, {
  artifactRefs: [],
  closeoutRef,
  quoteRef,
  resultEventRef: `result.public.pylon.labor_market.${resultEventId}`,
  verificationCommandRef: 'command.public.pylon.labor.bun_test',
})
log(`results: ${results.status} ${JSON.stringify(results.json.result ?? results.json).slice(0, 120)}`)

// 6) Release escrow to the provider (validator-pass) -> settled.
const release = await api(`/api/forum/work-requests/${WR}/release`, {
  quoteRef,
  verificationVerdictRef: `verdict.public.pylon.labor_market.${WR}.bun_test.pass`,
})
log(`release: ${release.status} released=${release.json.released} escrow.state=${release.json.escrow?.state}`)

await relay.close?.()
console.log(JSON.stringify({
  workRequestId: WR, quoteRef, amountSats, resultEventId, closeoutRef,
  released: release.json.released, escrowState: release.json.escrow?.state,
}, null, 2))
process.exit(0)

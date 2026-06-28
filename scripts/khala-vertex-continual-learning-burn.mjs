#!/usr/bin/env node
// Khala-ROUTED VERTEX continual-learning burn loop (spend-capped).
//
// PURPOSE. Burn PAID Vertex Gemini capacity (project openagentsgemini) on
// GENUINELY VALUABLE continual-learning work, but route every call THROUGH KHALA
// so the spend (a) counts on the public "Khala Tokens Served" counter, (b) writes
// an exact `token_usage_events` row, (c) writes an owner-private `agent_traces`
// row, and (d) produces a real continual-learning artifact (distillation dataset
// records + GEPA/mutalisk-shaped optimization candidates). The owner's hard rule:
// "no point burning unless Khala improves" -- so this is NOT a direct Vertex call
// (a direct Vertex call moves the Khala counter by +0 and records nothing in our
// DB). It is a Khala `/api/v1/chat/completions` call that DISPATCHES to Vertex.
//
// HOW KHALA -> VERTEX. Every request uses model `openagents/khala` with the
// HONEST internal continual-learning attribution headers:
//   x-openagents-demand-kind:   internal           (NOT internal_stress)
//   x-openagents-demand-source: cl_vertex_burn     (NOT glm-saturation)
//   x-oa-emit-trace:            on                 (opt this request into trace capture)
// We deliberately do NOT send the GLM-SATURATION pin
// (`internal_stress`+`glm-saturation`) the GLM burn uses, and the request is not
// tool-bearing, so the gateway's special branches (glm-saturation GLM-only,
// gym_mirrorcode strong-coding) DO NOT fire. The request falls to the DEFAULT
// `openagents/khala` lane plan, which dispatches to Vertex Gemini
// (`gemini-3.5-flash`) -- directly when GLM is down (fail-open of own-capacity to
// the paid conversational backing), and as the conversational backing generally.
// See chat-completions-routes.ts (basePlannedIds / glmSaturationStressKhalaRequest)
// and vertex-gemini-adapter.ts (DEFAULT_GEMINI_MODEL_ID = 'gemini-3.5-flash').
//
// WHY IT COUNTS + RECORDS (verified facts, not assumptions):
//   - COUNTER + token_usage_events: the Worker's served-tokens recorder inserts a
//     `token_usage_events` row for EVERY served Khala completion and publishes a
//     public-counter delta whenever `servedTokensRowIsPublicCountable(...)` is
//     true -- which it is unconditionally (served-tokens-recorder.ts). The
//     `demand_kind`/`demand_source` we send are persisted on the row, so the
//     public demand-mix endpoint segments this burn by `cl_vertex_burn`.
//   - agent_traces: the Khala chat-trace emitter persists an ATIF `agent_traces`
//     row when the Worker master flag `KHALA_CHAT_TRACE_EMIT_ENABLED` is on AND
//     this request opted in. We send `x-oa-emit-trace: on` to opt in, with the
//     same demand attribution stamped on the trace (khala-chat-trace-emitter.ts).
//     If the deployment master flag is OFF, the completion + token row still land
//     (counter still moves); only the trace is a no-op. The runner reports the
//     observed trace-capture outcome so the operator can see which it was.
//
// SPEND BOUND (hard cap; real money). This burn is PAID Vertex spend on
// project openagentsgemini. Cumulative estimated spend is computed from the
// provider-reported usage tokens and the gemini-3.5-flash COST basis
// (pricing.ts VERTEX_GEMINI_COST: input $0.075 / output $0.30 per 1M tokens).
// When cumulative est spend >= CL_VERTEX_TRANCHE_USD (default $50) the loop STOPS
// immediately -- no further requests are launched. The cap is checked before every
// request AND between cycles, so concurrency cannot meaningfully overrun it.
//
// THE VALUABLE WORKLOAD (continual-learning value). Identical lanes to the GLM
// burn so Khala still IMPROVES from the spend:
//   1. CORPUS lane: distill a staged technical corpus (default the Inference
//      Engineering book) into structured SFT/eval records under rotating lenses.
//   2. TRACE lane (optional, admin token): pull the live operator trace-review
//      report and produce failure-mode analyses + mutalisk-shaped optimization
//      candidates {signature, base_module, optimized_module, metric,
//      trace_provenance}. Candidates are EVIDENCE only; the Effect product
//      surface remains the acceptance/promotion authority.
//
// OUTPUT. Owner-private JSONL dataset + per-batch public-safe receipt under
// $CL_OUT_DIR (default ~/work/.khala-continual-learning-vertex, gitignored). The
// dataset is derived training data and is NEVER committed; receipts carry only
// counts, digests, route, observed usage, and the spend estimate -- no raw corpus
// text, prompts, keys, or PII.
//
// SECRET-SAFE. Bearer key(s) read from a gitignored secrets file; never printed.
//
// Usage:
//   node scripts/khala-vertex-continual-learning-burn.mjs [--once] [--cycles N]
// Env (all optional, sane defaults):
//   KHALA_BASE_URL            default https://openagents.com/api/v1
//   KHALA_BURN_ENV            secrets file (default ~/work/.secrets/khala-heartbeat.env)
//   CL_BURN_KEYS              comma keys (else KHALA_HEARTBEAT_KEYS from the env file)
//   CL_VERTEX_TRANCHE_USD     hard per-tranche spend cap in USD (default 50)
//   CL_VERTEX_IN_USD_PER_MTOK gemini-3.5-flash input cost  (default 0.075)
//   CL_VERTEX_OUT_USD_PER_MTOK gemini-3.5-flash output cost (default 0.30)
//   CL_DEMAND_KIND            demand kind header (default internal)
//   CL_DEMAND_SOURCE          demand source header (default cl_vertex_burn)
//   CL_EMIT_TRACE             '1' to send x-oa-emit-trace (default 1)
//   CL_CONCURRENCY            parallel in-flight calls (default 4)
//   CL_MAX_TOKENS             max_tokens per call (default 1024)
//   CL_CORPUS                 corpus path (default ~/work/inference-engineering-fulltext.txt)
//   CL_OUT_DIR                output dir (default ~/work/.khala-continual-learning-vertex)
//   CL_CHUNK_CHARS            corpus chunk size (default 3200)
//   CL_CHUNK_OVERLAP          chunk overlap (default 400)
//   CL_TRACE_EVERY            run the trace lane every N cycles (default 5; 0=off)
//   OPENAGENTS_ADMIN_API_TOKEN  admin token for the trace lane (else trace lane skipped)
//   CL_EMPTY_RETRIES          retries on empty completion (default 2)

import fs from 'node:fs'
import path from 'node:path'
import os from 'node:os'
import crypto from 'node:crypto'

const HOME = os.homedir()
const WORK = path.join(HOME, 'work')
const env = process.env

const BASE = env.KHALA_BASE_URL || 'https://openagents.com/api/v1'
const PUBLIC_BASE = env.KHALA_PUBLIC_BASE || 'https://openagents.com'
const BURN_ENV =
  env.KHALA_BURN_ENV || path.join(WORK, '.secrets', 'khala-heartbeat.env')
const CONCURRENCY = Math.max(1, parseInt(env.CL_CONCURRENCY || '4', 10))
const MAX_TOKENS = Math.max(64, parseInt(env.CL_MAX_TOKENS || '1024', 10))
const CORPUS =
  env.CL_CORPUS || path.join(WORK, 'inference-engineering-fulltext.txt')
const OUT_DIR =
  env.CL_OUT_DIR || path.join(WORK, '.khala-continual-learning-vertex')
const CHUNK_CHARS = Math.max(800, parseInt(env.CL_CHUNK_CHARS || '3200', 10))
const CHUNK_OVERLAP = Math.max(0, parseInt(env.CL_CHUNK_OVERLAP || '400', 10))
const TRACE_EVERY = Math.max(0, parseInt(env.CL_TRACE_EVERY || '5', 10))
const EMPTY_RETRIES = Math.max(0, parseInt(env.CL_EMPTY_RETRIES || '2', 10))
const ADMIN_TOKEN = (env.OPENAGENTS_ADMIN_API_TOKEN || '').trim()

// --- spend cap (real Vertex money) ------------------------------------------
const TRANCHE_USD = Math.max(0.01, parseFloat(env.CL_VERTEX_TRANCHE_USD || '50'))
// gemini-3.5-flash COST basis, per 1M tokens (pricing.ts VERTEX_GEMINI_COST).
const IN_USD_PER_MTOK = Math.max(
  0,
  parseFloat(env.CL_VERTEX_IN_USD_PER_MTOK || '0.075'),
)
const OUT_USD_PER_MTOK = Math.max(
  0,
  parseFloat(env.CL_VERTEX_OUT_USD_PER_MTOK || '0.30'),
)

// --- honest internal CL attribution (selects Vertex, never GLM) -------------
const DEMAND_KIND = (env.CL_DEMAND_KIND || 'internal').trim()
const DEMAND_SOURCE = (env.CL_DEMAND_SOURCE || 'cl_vertex_burn').trim()
const EMIT_TRACE = (env.CL_EMIT_TRACE ?? '1').trim() !== '0'

const argv = process.argv.slice(2)
const ONCE = argv.includes('--once')
const CYCLES_FLAG = (() => {
  const i = argv.indexOf('--cycles')
  return i >= 0 && argv[i + 1] ? Math.max(1, parseInt(argv[i + 1], 10)) : 0
})()

// --- secrets / keys ---------------------------------------------------------
function loadEnvFile(p) {
  try {
    const txt = fs.readFileSync(p, 'utf8')
    for (const line of txt.split('\n')) {
      const m = line.match(/^\s*([A-Za-z_][A-Za-z0-9_]*)=(.*)$/)
      if (m && env[m[1]] === undefined) {
        let v = m[2].trim()
        if (
          (v.startsWith('"') && v.endsWith('"')) ||
          (v.startsWith("'") && v.endsWith("'"))
        )
          v = v.slice(1, -1)
        env[m[1]] = v
      }
    }
  } catch {
    /* optional */
  }
}
loadEnvFile(BURN_ENV)
const KEYS = (env.CL_BURN_KEYS || env.KHALA_BURN_KEYS || env.KHALA_HEARTBEAT_KEYS || '')
  .split(',')
  .map(s => s.trim())
  .filter(Boolean)
if (KEYS.length === 0) {
  console.error(
    `[fatal] no burn keys. Set KHALA_HEARTBEAT_KEYS in ${BURN_ENV} (or CL_BURN_KEYS).`,
  )
  process.exit(1)
}
let keyIdx = 0
const nextKey = () => KEYS[keyIdx++ % KEYS.length]

fs.mkdirSync(OUT_DIR, { recursive: true })
const day = new Date().toISOString().slice(0, 10)
const DATASET = path.join(OUT_DIR, `corpus-dataset-${day}.jsonl`)
const CANDIDATES = path.join(OUT_DIR, `remediation-candidates-${day}.jsonl`)
const RECEIPT = path.join(OUT_DIR, `receipt-${day}.json`)
const LOG = path.join(OUT_DIR, `burn-${day}.jsonl`)

const sha = s => crypto.createHash('sha256').update(s).digest('hex')

// --- stats ------------------------------------------------------------------
const stats = {
  startedAt: new Date().toISOString(),
  route: `khala->vertex (demand_kind=${DEMAND_KIND}, demand_source=${DEMAND_SOURCE})`,
  servedModel: 'openagents/khala',
  emitTrace: EMIT_TRACE,
  capUsd: TRANCHE_USD,
  cycles: 0,
  calls: 0,
  ok: 0,
  empty: 0,
  errors: 0,
  http000: 0,
  promptTokens: 0,
  completionTokens: 0,
  totalTokens: 0,
  corpusRecords: 0,
  remediationCandidates: 0,
  // public-safe observed served lanes from the response `openagents.supply_lane`
  // (gemini = Vertex Gemini). Proves Khala dispatched to Vertex.
  supplyLanes: {},
}

// --- spend estimate ---------------------------------------------------------
// Estimated USD Vertex cost for the tokens burned so far, on the
// gemini-3.5-flash cost basis. Cap is checked against this number.
function estSpendUsd() {
  return (
    (stats.promptTokens * IN_USD_PER_MTOK +
      stats.completionTokens * OUT_USD_PER_MTOK) /
    1_000_000
  )
}
let capExceeded = false
function capReached() {
  if (capExceeded) return true
  if (estSpendUsd() >= TRANCHE_USD) {
    capExceeded = true
    logLine({ event: 'spend_cap_reached', estSpendUsd: estSpendUsd(), capUsd: TRANCHE_USD })
  }
  return capExceeded
}

function writeReceipt(corpusDigestPrefix) {
  const receipt = {
    schema: 'openagents.khala.continual_learning_vertex_burn_receipt.v0_1',
    generatedAt: new Date().toISOString(),
    startedAt: stats.startedAt,
    route: stats.route,
    servedModelRequested: stats.servedModel,
    demandKind: DEMAND_KIND,
    demandSource: DEMAND_SOURCE,
    emitTrace: EMIT_TRACE,
    paidVertexSpend: true,
    corpusDigestPrefix,
    spend: {
      capUsd: TRANCHE_USD,
      estSpendUsd: Math.round(estSpendUsd() * 1_000_000) / 1_000_000,
      remainingUsd:
        Math.round(Math.max(0, TRANCHE_USD - estSpendUsd()) * 1_000_000) /
        1_000_000,
      capReached: capExceeded,
      costBasis: {
        model: 'gemini-3.5-flash',
        inputUsdPerMtok: IN_USD_PER_MTOK,
        outputUsdPerMtok: OUT_USD_PER_MTOK,
      },
    },
    counts: {
      cycles: stats.cycles,
      calls: stats.calls,
      ok: stats.ok,
      emptyCompletions: stats.empty,
      errors: stats.errors,
      http000: stats.http000,
      corpusRecords: stats.corpusRecords,
      remediationCandidates: stats.remediationCandidates,
    },
    tokensBurned: {
      prompt: stats.promptTokens,
      completion: stats.completionTokens,
      total: stats.totalTokens,
    },
    observedSupplyLanes: stats.supplyLanes,
    note:
      'Khala-routed PAID Vertex Gemini (gemini-3.5-flash) continual-learning burn. ' +
      'Requests use model openagents/khala with honest internal CL attribution (NOT the ' +
      'glm-saturation pin), so the gateway dispatches to Vertex; every completion counts ' +
      'on the public Khala counter, writes an exact token_usage_events row, and (with the ' +
      'Worker trace flag on + per-request opt-in) writes an owner-private agent_traces row. ' +
      'Dataset is owner-private derived training data and is not exported here.',
  }
  fs.writeFileSync(RECEIPT, JSON.stringify(receipt, null, 2))
  return receipt
}
function logLine(obj) {
  fs.appendFileSync(LOG, JSON.stringify({ ts: new Date().toISOString(), ...obj }) + '\n')
}

// --- the Khala->Vertex call (the ONLY network call shape) -------------------
async function khalaCall(messages, { maxTokens = MAX_TOKENS, temperature = 0.5 } = {}) {
  const key = nextKey()
  const body = JSON.stringify({
    model: 'openagents/khala',
    max_tokens: maxTokens,
    temperature,
    messages,
  })
  const headers = {
    authorization: `Bearer ${key}`,
    'content-type': 'application/json',
    // HONEST internal continual-learning attribution. This is NOT the
    // glm-saturation pin, so the gateway routes openagents/khala to its DEFAULT
    // lane plan (Vertex Gemini when GLM is down / as conversational backing).
    'x-openagents-demand-kind': DEMAND_KIND,
    'x-openagents-demand-source': DEMAND_SOURCE,
  }
  // Opt this request into ATIF trace capture so an agent_traces row is written
  // (gated additionally by the Worker master flag KHALA_CHAT_TRACE_EMIT_ENABLED).
  if (EMIT_TRACE) headers['x-oa-emit-trace'] = 'on'
  const ctrl = new AbortController()
  const to = setTimeout(() => ctrl.abort(), 180_000)
  try {
    const res = await fetch(`${BASE}/chat/completions`, {
      method: 'POST',
      signal: ctrl.signal,
      headers,
      body,
    })
    stats.calls++
    const status = res.status
    if (status !== 200) {
      stats.errors++
      const txt = await res.text().catch(() => '')
      return { ok: false, status, error: txt.slice(0, 200) }
    }
    const json = await res.json()
    const content = json?.choices?.[0]?.message?.content || ''
    const usage = json?.usage || {}
    const pt = usage.prompt_tokens || 0
    const ct = usage.completion_tokens || 0
    stats.promptTokens += pt
    stats.completionTokens += ct
    stats.totalTokens += usage.total_tokens || pt + ct
    // Record the disclosed served lane when present (public-safe).
    const lane = json?.openagents?.supply_lane
    if (typeof lane === 'string') {
      stats.supplyLanes[lane] = (stats.supplyLanes[lane] || 0) + 1
    }
    const responseId = typeof json?.id === 'string' ? json.id : undefined
    if (!content.trim()) {
      stats.empty++
      return { ok: false, status: 200, empty: true, usage, responseId, lane }
    }
    stats.ok++
    return { ok: true, status: 200, content, usage, responseId, lane }
  } catch (e) {
    stats.calls++
    stats.errors++
    stats.http000++
    return { ok: false, status: 0, error: String(e?.name || e).slice(0, 120) }
  } finally {
    clearTimeout(to)
  }
}

async function khalaCallRetryEmpty(messages, opts) {
  let last
  for (let i = 0; i <= EMPTY_RETRIES; i++) {
    if (capReached()) return { ok: false, status: 0, capped: true }
    last = await khalaCall(messages, { ...opts, temperature: 0.4 + i * 0.15 })
    if (last.ok) return last
    if (last.status === 0) {
      await sleep(1500 + i * 1500)
    }
  }
  return last
}

const sleep = ms => new Promise(r => setTimeout(r, ms))

// --- corpus chunking + lenses ----------------------------------------------
function readCorpusChunks() {
  let text
  try {
    text = fs.readFileSync(CORPUS, 'utf8')
  } catch {
    return { chunks: [], digestPrefix: 'no_corpus' }
  }
  const digestPrefix = sha(text).slice(0, 16)
  const chunks = []
  const step = CHUNK_CHARS - CHUNK_OVERLAP
  for (let i = 0; i < text.length; i += step) {
    const c = text.slice(i, i + CHUNK_CHARS).trim()
    if (c.length > 400) chunks.push({ index: chunks.length, text: c })
  }
  return { chunks, digestPrefix }
}

// Rotating distillation lenses (genuinely different, valuable SFT/eval data).
const LENSES = [
  {
    id: 'exam_qa_gold_reasoning',
    sys:
      'You are an expert ML inference-systems engineer building a high-quality training dataset. ' +
      'From the passage, produce ONE challenging exam-style question that tests real understanding ' +
      '(not trivia), then a rigorous step-by-step GOLD reasoning answer grounded ONLY in the passage. ' +
      'Output strict JSON: {"question": "...", "gold_reasoning": "...", "final_answer": "...", "difficulty": "easy|medium|hard"}.',
  },
  {
    id: 'worked_derivation',
    sys:
      'You are writing worked-example training data for ML inference engineering. ' +
      'From the passage, identify a quantitative or systems relationship and produce ONE worked example: ' +
      'a concrete scenario, the derivation/calculation steps, and the result. ' +
      'Output strict JSON: {"scenario": "...", "steps": ["..."], "result": "...", "concept": "..."}.',
  },
  {
    id: 'misconception_critique',
    sys:
      'You are building a self-critique / preference dataset for ML inference engineering. ' +
      'From the passage, state ONE common misconception a practitioner might hold, why it is wrong per the passage, ' +
      'and the correct understanding. ' +
      'Output strict JSON: {"misconception": "...", "why_wrong": "...", "correct": "...", "concept": "..."}.',
  },
  {
    id: 'first_principles_explainer',
    sys:
      'You are writing concept-explanation training data for ML inference engineering. ' +
      'From the passage, pick the single most important concept and explain it from first principles, ' +
      'including when it matters in production serving. ' +
      'Output strict JSON: {"concept": "...", "explanation": "...", "when_it_matters": "...", "pitfalls": ["..."]}.',
  },
]

function tryParseJson(s) {
  const fence = s.match(/```(?:json)?\s*([\s\S]*?)```/)
  const cand = fence ? fence[1] : s
  const start = cand.indexOf('{')
  const end = cand.lastIndexOf('}')
  if (start < 0 || end <= start) return null
  try {
    return JSON.parse(cand.slice(start, end + 1))
  } catch {
    return null
  }
}

// --- worker pool over a task list ------------------------------------------
async function runPool(tasks, worker) {
  let i = 0
  const runners = Array.from({ length: CONCURRENCY }, async () => {
    while (i < tasks.length) {
      if (capReached()) return
      const myIndex = i++
      await worker(tasks[myIndex], myIndex)
    }
  })
  await Promise.all(runners)
}

// --- corpus lane ------------------------------------------------------------
async function corpusCycle(cycle, corpus) {
  const lens = LENSES[cycle % LENSES.length]
  const fh = fs.openSync(DATASET, 'a')
  let produced = 0
  await runPool(corpus.chunks, async chunk => {
    const messages = [
      { role: 'system', content: lens.sys },
      {
        role: 'user',
        content: `Source passage (Inference Engineering corpus):\n"""\n${chunk.text}\n"""\n\nReturn ONLY the strict JSON object.`,
      },
    ]
    const r = await khalaCallRetryEmpty(messages)
    if (!r.ok) return
    const parsed = tryParseJson(r.content)
    const record = {
      ts: new Date().toISOString(),
      lane: 'corpus',
      lens: lens.id,
      corpusDigestPrefix: corpus.digestPrefix,
      chunkIndex: chunk.index,
      chunkDigestPrefix: sha(chunk.text).slice(0, 12),
      servedModelRequested: 'openagents/khala',
      route: stats.route,
      supplyLane: r.lane,
      responseId: r.responseId,
      usage: r.usage,
      valid_json: !!parsed,
      record: parsed || { raw: r.content },
    }
    fs.writeSync(fh, JSON.stringify(record) + '\n')
    produced++
    stats.corpusRecords++
  })
  fs.closeSync(fh)
  logLine({ event: 'corpus_cycle', cycle, lens: lens.id, produced })
  return produced
}

// --- trace lane (real fleet failure modes -> mutalisk-shaped candidates) ----
async function traceCycle(cycle) {
  if (!ADMIN_TOKEN) {
    logLine({ event: 'trace_cycle_skipped', reason: 'no_admin_token' })
    return 0
  }
  if (capReached()) return 0
  let report
  try {
    const res = await fetch(
      `${PUBLIC_BASE}/api/operator/khala/trace-review?limit=8`,
      { headers: { authorization: `Bearer ${ADMIN_TOKEN}` } },
    )
    if (res.status !== 200) {
      logLine({ event: 'trace_cycle_skipped', reason: `http_${res.status}` })
      return 0
    }
    report = await res.json()
  } catch (e) {
    logLine({ event: 'trace_cycle_skipped', reason: String(e).slice(0, 80) })
    return 0
  }
  const failureModes = (report.failureModes || []).filter(f => f && f.label)
  if (failureModes.length === 0) return 0
  const fh = fs.openSync(CANDIDATES, 'a')
  let produced = 0
  await runPool(failureModes, async fm => {
    const facts = {
      failure_label: fm.label,
      severity: fm.severity,
      count_24h: fm.count,
      failure_ref: fm.failureRef,
      window: report.window,
      demand_sources: (report.demandSources || []).map(d => ({
        label: d.label,
        count: d.count,
      })),
    }
    const messages = [
      {
        role: 'system',
        content:
          'You are a senior reliability + ML-serving engineer producing a continual-learning improvement candidate ' +
          'from REAL production fleet telemetry. Given a failure mode aggregate, produce: a likely root-cause analysis, ' +
          'a concrete remediation (routing/prompt/policy/code), and a mutalisk-shaped optimization candidate. ' +
          'Output strict JSON: {"failure_ref":"...","root_cause":"...","remediation":"...","verification":"...",' +
          '"candidate":{"signature":"...","base_module":"...","optimized_module":"...","metric":"...","trace_provenance":"..."}}. ' +
          'Be specific and actionable. This is candidate evidence only; the product surface gates/admits it.',
      },
      {
        role: 'user',
        content: `Production failure-mode telemetry (public-safe aggregate):\n${JSON.stringify(facts, null, 2)}\n\nReturn ONLY the strict JSON object.`,
      },
    ]
    const r = await khalaCallRetryEmpty(messages, { maxTokens: 1200 })
    if (!r.ok) return
    const parsed = tryParseJson(r.content)
    const rec = {
      ts: new Date().toISOString(),
      lane: 'trace',
      source: 'operator_khala_trace_review',
      window: report.window,
      failureRef: fm.failureRef,
      servedModelRequested: 'openagents/khala',
      route: stats.route,
      supplyLane: r.lane,
      responseId: r.responseId,
      usage: r.usage,
      valid_json: !!parsed,
      candidate: parsed || { raw: r.content },
    }
    fs.writeSync(fh, JSON.stringify(rec) + '\n')
    produced++
    stats.remediationCandidates++
  })
  fs.closeSync(fh)
  logLine({ event: 'trace_cycle', cycle, produced })
  return produced
}

// --- main loop --------------------------------------------------------------
function fmt(n) {
  return n.toLocaleString('en-US')
}
function usd(n) {
  return `$${(Math.round(n * 10000) / 10000).toFixed(4)}`
}
async function main() {
  const corpus = readCorpusChunks()
  console.log(
    `[start] Khala-routed VERTEX continual-learning burn (spend-capped)\n` +
      `  route=khala->vertex demand_kind=${DEMAND_KIND} demand_source=${DEMAND_SOURCE} emit_trace=${EMIT_TRACE}\n` +
      `  SPEND CAP=${usd(TRANCHE_USD)} (gemini-3.5-flash cost: in ${IN_USD_PER_MTOK}/Mtok out ${OUT_USD_PER_MTOK}/Mtok)\n` +
      `  keys=${KEYS.length} concurrency=${CONCURRENCY} max_tokens=${MAX_TOKENS}\n` +
      `  corpus=${CORPUS} chunks=${corpus.chunks.length} digest=${corpus.digestPrefix}\n` +
      `  trace_lane=${ADMIN_TOKEN ? `every ${TRACE_EVERY} cycles` : 'OFF (no admin token)'}\n` +
      `  out=${OUT_DIR}`,
  )
  if (corpus.chunks.length === 0) {
    console.error(`[fatal] no corpus chunks at ${CORPUS}`)
    process.exit(1)
  }
  const maxCycles = CYCLES_FLAG || (ONCE ? 1 : Infinity)
  const t0 = Date.now()
  for (let cycle = 0; cycle < maxCycles; cycle++) {
    if (capReached()) {
      console.log(`[stop] spend cap ${usd(TRANCHE_USD)} reached (est ${usd(estSpendUsd())}); stopping before cycle ${cycle}.`)
      break
    }
    stats.cycles = cycle + 1
    const cp = await corpusCycle(cycle, corpus)
    let tp = 0
    if (TRACE_EVERY > 0 && cycle % TRACE_EVERY === 0) tp = await traceCycle(cycle)
    const mins = (Date.now() - t0) / 60000
    const tpm = mins > 0 ? Math.round(stats.totalTokens / mins) : 0
    writeReceipt(corpus.digestPrefix)
    console.log(
      `[cycle ${cycle}] lens=${LENSES[cycle % LENSES.length].id} corpus_records=${cp} trace_candidates=${tp} | ` +
        `tokens=${fmt(stats.totalTokens)} (~${fmt(tpm)}/min) est_spend=${usd(estSpendUsd())}/${usd(TRANCHE_USD)} ` +
        `ok=${stats.ok} empty=${stats.empty} err=${stats.errors} lanes=${JSON.stringify(stats.supplyLanes)}`,
    )
    if (capReached()) {
      console.log(`[stop] spend cap ${usd(TRANCHE_USD)} reached (est ${usd(estSpendUsd())}); stopping.`)
      break
    }
    if (cycle + 1 >= maxCycles) break
    await sleep(1000)
  }
  const receipt = writeReceipt(corpus.digestPrefix)
  console.log(`[done] receipt: ${RECEIPT}`)
  console.log(
    JSON.stringify({
      tokensBurned: receipt.tokensBurned,
      spend: receipt.spend,
      observedSupplyLanes: receipt.observedSupplyLanes,
    }),
  )
}

main().catch(e => {
  console.error('[fatal]', e)
  process.exit(1)
})

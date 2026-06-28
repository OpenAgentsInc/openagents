#!/usr/bin/env node
// Khala GLM own-capacity CONTINUAL-LEARNING burn loop.
//
// PURPOSE. Drive our self-hosted GLM-5.2-REAP-504B fleet at full tilt with
// GENUINELY VALUABLE work at $0 marginal cost, producing a real continual-
// learning distillation dataset (not toy problems, not synthetic make-work).
//
// THE $0 OWN-CAPACITY GUARANTEE (verified live 2026-06-28). Every request uses
// model `openagents/khala` with the GLM-SATURATION attribution headers:
//   x-openagents-demand-kind:   internal_stress
//   x-openagents-demand-source: glm-saturation
// The gateway (chat-completions-routes.ts) routes that exact combination to the
// GLM lane ONLY: `[HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID]` with NO Vertex /
// Fireworks / OpenRouter paid fallback. So a 200 is ALWAYS served by our own
// GLM boxes (G4 spot, billed hourly => $0 marginal per token); when GLM is
// unavailable the request FAILS CLOSED (no paid spill). This script NEVER sends
// any other model/route, so it can never incur external paid spend. It is also
// internal_stress => preemptible: real external Khala users always win, this
// burn yields. (See docs/inference/2026-06-25-khala-glm-52-reap-backing-lane.md
// and the GLM-saturation branch in chat-completions-routes.ts.)
//
// THE VALUABLE WORKLOAD. Two real continual-learning lanes, both distilled by
// GLM itself (self-distillation / on-policy synthetic data):
//   1. CORPUS lane: distill a staged technical corpus (default the Inference
//      Engineering book) into structured supervised/eval records under rotating
//      "lenses" (exam Q&A with gold reasoning, worked derivations, common-
//      misconception critiques, first-principles explanations). Diverse, high-
//      token, genuinely useful SFT/eval data for our own models' inference-
//      engineering knowledge.
//   2. TRACE lane (optional, admin token): pull the LIVE operator trace-review
//      report (real fleet failure modes / triage signal) and have GLM produce
//      failure-mode analyses + concrete remediation + a mutalisk-shaped
//      optimization candidate {signature, base_module, optimized_module,
//      metric, trace_provenance}. This is the "learn from our real agent
//      traces" lane; the Effect side remains the acceptance/promotion authority
//      (mutalisk candidate contract — candidates are evidence, never writes).
//
// OUTPUT. Owner-private JSONL dataset + per-batch public-safe receipt under
// $OUT_DIR (default ~/work/.khala-continual-learning, gitignored). The dataset
// is derived training data and is NEVER committed; receipts carry only counts,
// digests, route, and observed usage — no raw corpus text, prompts, keys, or PII.
//
// SECRET-SAFE. Bearer key(s) read from a gitignored secrets file; never printed.
//
// Usage:
//   node scripts/khala-glm-continual-learning-burn.mjs [--once] [--cycles N]
// Env (all optional, sane defaults):
//   KHALA_BASE_URL            default https://openagents.com/api/v1
//   KHALA_BURN_ENV            secrets file (default ~/work/.secrets/khala-heartbeat.env)
//   KHALA_BURN_KEYS           comma keys (else KHALA_HEARTBEAT_KEYS from the env file)
//   CL_CONCURRENCY            parallel in-flight GLM calls (default 4)
//   CL_MAX_TOKENS             max_tokens per call (default 1024)
//   CL_CORPUS                 corpus path (default ~/work/inference-engineering-fulltext.txt)
//   CL_OUT_DIR                output dir (default ~/work/.khala-continual-learning)
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
const OUT_DIR = env.CL_OUT_DIR || path.join(WORK, '.khala-continual-learning')
const CHUNK_CHARS = Math.max(800, parseInt(env.CL_CHUNK_CHARS || '3200', 10))
const CHUNK_OVERLAP = Math.max(0, parseInt(env.CL_CHUNK_OVERLAP || '400', 10))
const TRACE_EVERY = Math.max(0, parseInt(env.CL_TRACE_EVERY || '5', 10))
const EMPTY_RETRIES = Math.max(0, parseInt(env.CL_EMPTY_RETRIES || '2', 10))
const ADMIN_TOKEN = (env.OPENAGENTS_ADMIN_API_TOKEN || '').trim()

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
  route: 'glm-saturation:internal_stress (GLM-only, no paid fallback)',
  servedModel: 'openagents/khala',
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
}
function writeReceipt(corpusDigestPrefix) {
  const receipt = {
    schema: 'openagents.khala.continual_learning_burn_receipt.v0_1',
    generatedAt: new Date().toISOString(),
    startedAt: stats.startedAt,
    route: stats.route,
    servedModelRequested: stats.servedModel,
    ownCapacity: true,
    paidFallback: false,
    corpusDigestPrefix,
    counts: {
      cycles: stats.cycles,
      calls: stats.calls,
      ok: stats.ok,
      emptyCompletions: stats.empty,
      errors: stats.errors,
      http000_glm_unavailable: stats.http000,
      corpusRecords: stats.corpusRecords,
      remediationCandidates: stats.remediationCandidates,
    },
    tokensBurned: {
      prompt: stats.promptTokens,
      completion: stats.completionTokens,
      total: stats.totalTokens,
    },
    note:
      'All tokens served by self-hosted GLM-5.2-REAP-504B (own-capacity, $0 marginal). ' +
      'Requests use the GLM-only glm-saturation route with no paid fallback; a 200 is always own-GLM. ' +
      'Dataset is owner-private derived training data and is not exported here.',
  }
  fs.writeFileSync(RECEIPT, JSON.stringify(receipt, null, 2))
  return receipt
}
function logLine(obj) {
  fs.appendFileSync(LOG, JSON.stringify({ ts: new Date().toISOString(), ...obj }) + '\n')
}

// --- the $0 own-capacity GLM call (the ONLY network call shape) -------------
async function glmCall(messages, { maxTokens = MAX_TOKENS, temperature = 0.5 } = {}) {
  const key = nextKey()
  const body = JSON.stringify({
    model: 'openagents/khala',
    max_tokens: maxTokens,
    temperature,
    messages,
  })
  const ctrl = new AbortController()
  const to = setTimeout(() => ctrl.abort(), 180_000)
  try {
    const res = await fetch(`${BASE}/chat/completions`, {
      method: 'POST',
      signal: ctrl.signal,
      headers: {
        authorization: `Bearer ${key}`,
        'content-type': 'application/json',
        // HARD-CODED $0 own-capacity route. Do not change: this is what pins
        // dispatch to GLM-only with no paid fallback.
        'x-openagents-demand-kind': 'internal_stress',
        'x-openagents-demand-source': 'glm-saturation',
      },
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
    if (!content.trim()) {
      stats.empty++
      return { ok: false, status: 200, empty: true, usage }
    }
    stats.ok++
    return { ok: true, status: 200, content, usage }
  } catch (e) {
    stats.calls++
    stats.errors++
    stats.http000++
    return { ok: false, status: 0, error: String(e?.name || e).slice(0, 120) }
  } finally {
    clearTimeout(to)
  }
}

async function glmCallRetryEmpty(messages, opts) {
  let last
  for (let i = 0; i <= EMPTY_RETRIES; i++) {
    last = await glmCall(messages, { ...opts, temperature: 0.4 + i * 0.15 })
    if (last.ok) return last
    if (last.status === 0) {
      // GLM unavailable: short backoff, then retry (never falls to paid).
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

// Rotating distillation lenses. Each produces genuinely different, valuable
// supervised/eval data from the same source passage.
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
  // Models sometimes wrap JSON in prose / code fences. Extract the largest brace span.
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
    const r = await glmCallRetryEmpty(messages)
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
      route: 'glm-saturation',
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
    const r = await glmCallRetryEmpty(messages, { maxTokens: 1200 })
    if (!r.ok) return
    const parsed = tryParseJson(r.content)
    const rec = {
      ts: new Date().toISOString(),
      lane: 'trace',
      source: 'operator_khala_trace_review',
      window: report.window,
      failureRef: fm.failureRef,
      servedModelRequested: 'openagents/khala',
      route: 'glm-saturation',
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
async function main() {
  const corpus = readCorpusChunks()
  console.log(
    `[start] GLM own-capacity continual-learning burn\n` +
      `  route=GLM-only(glm-saturation,internal_stress) no-paid-fallback\n` +
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
    stats.cycles = cycle + 1
    const cp = await corpusCycle(cycle, corpus)
    let tp = 0
    if (TRACE_EVERY > 0 && cycle % TRACE_EVERY === 0) tp = await traceCycle(cycle)
    const mins = (Date.now() - t0) / 60000
    const tpm = mins > 0 ? Math.round(stats.totalTokens / mins) : 0
    writeReceipt(corpus.digestPrefix)
    console.log(
      `[cycle ${cycle}] lens=${LENSES[cycle % LENSES.length].id} corpus_records=${cp} trace_candidates=${tp} | ` +
        `tokens=${fmt(stats.totalTokens)} (~${fmt(tpm)}/min) ok=${stats.ok} empty=${stats.empty} glm_down=${stats.http000} err=${stats.errors}`,
    )
    if (cycle + 1 >= maxCycles) break
    // brief pause between cycles to let receipts flush; GLM stays warm.
    await sleep(1000)
  }
  const receipt = writeReceipt(corpus.digestPrefix)
  console.log(`[done] receipt: ${RECEIPT}`)
  console.log(JSON.stringify(receipt.tokensBurned))
}

main().catch(e => {
  console.error('[fatal]', e)
  process.exit(1)
})

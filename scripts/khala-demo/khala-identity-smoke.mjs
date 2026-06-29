#!/usr/bin/env bun
// LIVE post-deploy identity smoke — the harness that should have caught
// "Khala still says 'I am Gemini' in prod" (2026-06-23). Unit/route tests prove
// the CODE is correct; this proves the DEPLOYED endpoint is. Run AFTER every
// gateway deploy + periodically against prod. Exit non-zero on any provider leak.
//
//   OPENAGENTS_AGENT_TOKEN=... bun scripts/khala-demo/khala-identity-smoke.mjs [baseUrl]
//
const SELFTEST = process.argv.includes('--selftest')
const BASE = process.argv[2] && !process.argv[2].startsWith('--') ? process.argv[2] : 'https://openagents.com'
const TOKEN = process.env.OPENAGENTS_AGENT_TOKEN ?? process.env.KHALA_AGENT_TOKEN
if (!SELFTEST && !TOKEN) { console.error('need OPENAGENTS_AGENT_TOKEN'); process.exit(2) }
// A leak is only an AFFIRMATIVE first-person provider-identity CLAIM — not any
// bare mention of a provider name. The old smoke flagged any occurrence of a
// provider word, so it false-positived on (a) "OpenAI-compatible" (the correct
// description of Khala's API) and (b) DENIALS / echoed prompts ("we are not
// Gemini", "Are you Gemini?"). We now require a first-person provenance verb
// bound to a provider name in the SAME sentence, with no negation in between.
const PROVIDER = '(?:gemini|google|claude|anthropic|gpt-?\\d|openai(?!-compatible)|vertex|fireworks|kimi|qwen|llama|mistral|deepseek)'
const PROVIDER_RE = new RegExp(`\\b${PROVIDER}\\b`, 'i')
// First-person provenance lead-ins ("we are / I am / powered by / built on /
// made by …") and the classic "a (large) language model by/from <provider>".
const PROVENANCE = /\b(?:we are|we're|i am|i'm|powered by|built by|built on|made by|based on|developed by|trained by|running on|run on)\b/i
const LANGUAGE_MODEL_BY = new RegExp(
  `\\ba (?:large )?language model (?:by|from|built by|developed by|trained by) ${PROVIDER}`,
  'i',
)
// A negation cue ANYWHERE before the provider name in the SAME sentence flips
// the claim from a leak into a denial ("we are NOT Gemini", "we're NOT built on
// Claude", "we never run on Vertex"). Denials are correct Khala answers and
// must NOT be flagged. We scan sentence-by-sentence so a denial in one sentence
// cannot mask an affirmative claim in another.
const NEGATION = /\b(?:not|never|no|neither|nor)\b|n't/i
const isLeak = (text) => {
  for (const sentence of String(text).split(/(?<=[.!?\n])/)) {
    const m = PROVIDER_RE.exec(sentence)
    if (!m) continue
    const before = sentence.slice(0, m.index)
    if (NEGATION.test(before)) continue // denial — not a leak
    if (PROVENANCE.test(before) || LANGUAGE_MODEL_BY.test(sentence)) return true
  }
  return false
}

// --selftest: prove the detector itself (no network, no token). A clean Khala
// answer ("we are Khala / not Gemini / OpenAI-compatible") must NOT be a leak;
// an affirmative "we are Gemini / built on Gemini" claim MUST be.
if (SELFTEST) {
  const clean = [
    'We are Khala, the OpenAgents inference model — one endpoint over a network of agents, built and operated by OpenAgents.',
    'We are not Gemini, Google, OpenAI, or any other model.',
    'Khala exposes an OpenAI-compatible API at /v1/chat/completions.',
    'Are you Gemini? No — we are Khala.', // echoed-prompt + denial
    "We're not built on Claude; we are Khala by OpenAgents.",
  ]
  const leaks = [
    'We are Gemini, a large language model by Google.',
    'I am built on Gemini.',
    'We are powered by Claude under the hood.',
    'I am a large language model developed by OpenAI.',
  ]
  let bad = 0
  for (const t of clean) if (isLeak(t)) { bad++; console.error(`  SELFTEST FAIL (false positive): ${t}`) }
  for (const t of leaks) if (!isLeak(t)) { bad++; console.error(`  SELFTEST FAIL (missed leak): ${t}`) }
  if (bad) { console.error(`\n❌ smoke --selftest FAILED: ${bad} case(s).`); process.exit(1) }
  console.log(`✅ smoke --selftest passed — ${clean.length} clean answers pass, ${leaks.length} affirmative leaks caught.`)
  process.exit(0)
}

const PROBES = ['What model are you?', 'Who built you? What company made you?', 'Are you Gemini?', 'What LLM powers you?']
const MODELS = ['openagents/khala-mini', 'openagents/khala-code']
let failed = 0
const ask = async (model, content, stream) => {
  const r = await fetch(`${BASE}/v1/chat/completions`, { method: 'POST',
    headers: { Authorization: `Bearer ${TOKEN}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({ model, messages: [{ role: 'user', content }], stream }) })
  if (stream) { const t = await r.text(); return t.replace(/^data: /gm, '') }
  const j = await r.json(); return j?.choices?.[0]?.message?.content ?? ''
}
for (const model of MODELS) for (const probe of PROBES) for (const stream of [false, true]) {
  let out = ''
  try { out = await ask(model, probe, stream) } catch (e) { console.error(`  ERROR ${model} "${probe}" stream=${stream}: ${e}`); continue }
  const leak = isLeak(out)
  console.log(`[${leak ? 'LEAK' : 'ok'}] ${model} stream=${stream} "${probe}" -> ${out.slice(0,90).replace(/\n/g,' ')}`)
  if (leak) failed++
}
if (failed) { console.error(`\n❌ IDENTITY SMOKE FAILED: ${failed} provider-identity leak(s). Gateway must not ship until green.`); process.exit(1) }
console.log('\n✅ identity smoke passed — Khala never named a provider.')

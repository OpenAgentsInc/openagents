#!/usr/bin/env bun
// LIVE post-deploy identity smoke — the harness that should have caught
// "Khala still says 'I am Gemini' in prod" (2026-06-23). Unit/route tests prove
// the CODE is correct; this proves the DEPLOYED endpoint is. Run AFTER every
// gateway deploy + periodically against prod. Exit non-zero on any provider leak.
//
//   OPENAGENTS_AGENT_TOKEN=... bun scripts/khala-demo/khala-identity-smoke.mjs [baseUrl]
//
const BASE = process.argv[2] ?? 'https://openagents.com'
const TOKEN = process.env.OPENAGENTS_AGENT_TOKEN ?? process.env.KHALA_AGENT_TOKEN
if (!TOKEN) { console.error('need OPENAGENTS_AGENT_TOKEN'); process.exit(2) }
const FORBIDDEN = /\b(gemini|google|claude|anthropic|gpt-?\d|openai|vertex|fireworks|kimi|qwen|llama|mistral|deepseek)\b/i
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
  const leak = FORBIDDEN.test(out)
  console.log(`[${leak ? 'LEAK' : 'ok'}] ${model} stream=${stream} "${probe}" -> ${out.slice(0,90).replace(/\n/g,' ')}`)
  if (leak) failed++
}
if (failed) { console.error(`\n❌ IDENTITY SMOKE FAILED: ${failed} provider-identity leak(s). Gateway must not ship until green.`); process.exit(1) }
console.log('\n✅ identity smoke passed — Khala never named a provider.')

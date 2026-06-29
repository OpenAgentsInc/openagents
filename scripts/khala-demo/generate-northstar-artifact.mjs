#!/usr/bin/env bun
// Submit the crossy-road north-star prompt to openagents/khala-code via the STREAMING
// path (the 524 fix), save the produced single-file HTML artifact, and print the
// receipt (tokens / cost_msat) so a caller can run it through the executed acceptance
// runner (M8 #6016, EPIC #6017).
//
// This is a BOUNDED-SPEND driver: ONE generation per invocation. It NEVER loops; the
// caller enforces the hard cap of attempts. It reuses the existing streaming transport
// from `run-head-to-head.mjs` (no new auth path) and writes NO secrets to disk.
//
// Auth/base/model come from env (same names the head-to-head runner reads):
//   KHALA_BASE_URL    e.g. https://openagents.com/v1
//   KHALA_AGENT_TOKEN bearer token (funded prod-credit agent token)
//   KHALA_MODEL       default openagents/khala-code
//
// Usage:
//   KHALA_BASE_URL=... KHALA_AGENT_TOKEN=... \
//   bun scripts/khala-demo/generate-northstar-artifact.mjs --out <artifact.html> \
//     [--contract] [--prompt-file <path>]
//
//   --contract        append the executed-acceptance STATE CONTRACT to the prompt so
//                     the model knows which window hooks the headless runner reads.
//   --out <path>      where to write the extracted HTML artifact (required).
//   --receipt <path>  where to write the receipt JSON (tokens/cost_msat). Optional.

import { writeFileSync } from 'node:fs'
import { liveStreamTransport, CROSSY_ROAD_PROMPT } from './run-head-to-head.mjs'

// The EXACT state contract the headless acceptance runner reads (see
// apps/openagents.com/workers/api/src/inference/acceptance-runner/runner.ts). Appended
// only when --contract is passed: a fair "can it build a WORKING game" test needs the
// model to expose the hooks the executor drives, since the gateway does not inject them.
const STATE_CONTRACT = `

IMPORTANT — expose this exact runtime state contract on \`window\` so the game can be
driven and verified headlessly (do NOT change these names or shapes):

  window.__openagentsCrossyRoadState = () => ({
    player:  { x, y, z },            // player world-unit position
    camera:  { position: { x, y, z } }, // current camera position
    progress: <number>,             // forward tiles travelled so far
    worldRowsAhead: <number>,       // count of distinct generated rows ahead of the player
    started: <boolean>,             // true once the game loop is running
    loopTicks: <number>,            // increments by >=1 on every update-loop frame
  })
  window.__openagentsCrossyRoadStart = () => { /* hide the start overlay and start the loop */ }
  window.__openagentsCrossyRoadRestart = () => { /* reset player to start: progress 0, started true */ }

Behavioral requirements the headless suite checks:
  - The page must LOAD with ZERO console/page errors. Do NOT touch localStorage or any
    storage at module/constructor load time (it throws in a sandboxed file:// context);
    guard any storage access in a try/catch and never let it run on load.
  - There must be a clickable PLAY control (a <button> or #start-btn / #play / #play-btn
    / #start element) that, when clicked, hides the overlay and sets started=true and
    makes the update loop advance (loopTicks increases over time).
  - Pressing the ArrowUp key must hop the player forward ~1 tile (progress and the
    forward world axis advance by ~1 per press).
  - The camera must FOLLOW the player smoothly: its position must move at most a few
    world units per hop (never tens/hundreds of units per move).
  - The world must keep generating new rows ahead of the player for at least 12 forward
    moves (worldRowsAhead stays >= 12 ahead; never run out into blue sky).
  - __openagentsCrossyRoadRestart() must fully reset player x/z and progress to the start.

Single self-contained HTML file using three.js from a CDN. Return ONLY the HTML.`

const parseArgs = argv => {
  const opts = {}
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]
    if (arg === '--contract') {
      opts.contract = true
      continue
    }
    if (arg.startsWith('--')) {
      const key = arg.slice(2)
      const next = argv[i + 1]
      if (next === undefined || next.startsWith('--')) opts[key] = true
      else {
        opts[key] = next
        i += 1
      }
    }
  }
  return opts
}

// Pull the single HTML document out of the model's completion. Models often wrap the
// file in a ```html fenced block; prefer that, else take from the first <!doctype/<html
// to the last </html>. Returns the raw content untouched if no HTML markers are found
// (so the caller still saves SOMETHING and the acceptance runner fails it honestly).
const extractHtml = content => {
  const fence = content.match(/```(?:html)?\s*\n([\s\S]*?)```/i)
  const body = fence ? fence[1] : content
  const lower = body.toLowerCase()
  const start = (() => {
    const d = lower.indexOf('<!doctype')
    if (d !== -1) return d
    const h = lower.indexOf('<html')
    return h !== -1 ? h : 0
  })()
  const endIdx = lower.lastIndexOf('</html>')
  const end = endIdx !== -1 ? endIdx + '</html>'.length : body.length
  return body.slice(start, end).trim()
}

const main = async () => {
  const args = parseArgs(process.argv.slice(2))
  const baseUrl = process.env.KHALA_BASE_URL
  const token = process.env.KHALA_AGENT_TOKEN
  const model = process.env.KHALA_MODEL || 'openagents/khala-code'
  const outPath = args.out
  if (!baseUrl || !token) {
    process.stderr.write('KHALA_BASE_URL and KHALA_AGENT_TOKEN are required\n')
    process.exit(2)
  }
  if (typeof outPath !== 'string') {
    process.stderr.write('--out <artifact.html> is required\n')
    process.exit(2)
  }

  const prompt = args.contract ? CROSSY_ROAD_PROMPT + STATE_CONTRACT : CROSSY_ROAD_PROMPT

  const startedAt = new Date()
  process.stderr.write(
    `submitting north-star prompt (contract=${Boolean(args.contract)}) to ${model} via stream...\n`,
  )
  let tokenCount = 0
  const response = await liveStreamTransport({
    baseUrl,
    token,
    model,
    prompt,
    onToken: () => {
      tokenCount += 1
      if (tokenCount % 500 === 0) process.stderr.write('.')
    },
  })
  const completedAt = new Date()
  process.stderr.write('\n')

  const content = response?.choices?.[0]?.message?.content ?? ''
  const html = extractHtml(content)
  writeFileSync(outPath, html + '\n')

  const oa = response?.openagents ?? {}
  const usage = response?.usage ?? {}
  const receipt = {
    model,
    startedAt: startedAt.toISOString(),
    completedAt: completedAt.toISOString(),
    wallClockMs: completedAt.getTime() - startedAt.getTime(),
    promptContract: Boolean(args.contract),
    rawContentBytes: content.length,
    extractedHtmlBytes: html.length,
    usage: {
      promptTokens: usage.prompt_tokens ?? null,
      completionTokens: usage.completion_tokens ?? null,
      totalTokens: usage.total_tokens ?? null,
    },
    receipt: {
      ref: typeof oa.receipt === 'string' ? oa.receipt : null,
      verification: typeof oa.verification === 'string' ? oa.verification : null,
      verified: oa.verified ?? null,
      scalarReward: oa.scalar_reward ?? null,
      costMsat: typeof oa.cost_msat === 'number' ? oa.cost_msat : null,
      priceMsat: typeof oa.price_msat === 'number' ? oa.price_msat : null,
    },
  }
  if (typeof args.receipt === 'string') {
    writeFileSync(args.receipt, JSON.stringify(receipt, null, 2) + '\n')
  }
  process.stderr.write(
    `wrote ${html.length} bytes HTML -> ${outPath}; ` +
      `cost_msat=${receipt.receipt.costMsat ?? 'null'} ` +
      `completion_tokens=${receipt.usage.completionTokens ?? 'null'} ` +
      `gateway_verification=${receipt.receipt.verification ?? 'null'}\n`,
  )
  process.stdout.write(JSON.stringify(receipt, null, 2) + '\n')
}

await main()

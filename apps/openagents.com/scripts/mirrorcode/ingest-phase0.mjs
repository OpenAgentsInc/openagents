#!/usr/bin/env bun
// Ingest a MirrorCode Phase-0 result JSON into the demo surface (#6378, epic
// #6376). This is the OWNER-GATED launch/record path: it POSTs a public-safe
// MirrorCode run record to `/api/gym/mirrorcode/runs`, which rebuilds it through
// the no-task-contents / no-canary public-safety boundary before storing.
//
// The runner lane (Phase 0 of
// docs/benchmarks/2026-06-27-mirrorcode-khala-gym-integration-analysis.md)
// writes a result file in the shape of `phase0-result.json` in this directory;
// this script is the thin owned ingester that records it on the demo surface.
//
// Usage (owner / operator, with the admin bearer token; never commit it):
//   OPENAGENTS_ADMIN_API_TOKEN=*** \
//   bun apps/openagents.com/scripts/mirrorcode/ingest-phase0.mjs \
//     [path/to/result.json] [--base https://openagents.com]
//
// Honesty: a `smoke` (Phase-0) run is always decisionGrade:false and is never
// shown as a published frontier measurement. Do NOT pass task source, test
// data, prompts, or canary strings — the server rejects them with a 400.

import { readFileSync } from 'node:fs'
import { resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))

const args = process.argv.slice(2)
let base = process.env.OPENAGENTS_BASE_URL ?? 'https://openagents.com'
const positional = []
for (let i = 0; i < args.length; i += 1) {
  if (args[i] === '--base') {
    base = args[i + 1] ?? base
    i += 1
  } else {
    positional.push(args[i])
  }
}

const resultPath = positional[0]
  ? resolve(process.cwd(), positional[0])
  : resolve(here, 'phase0-result.json')

const token = process.env.OPENAGENTS_ADMIN_API_TOKEN
if (!token || token.trim() === '') {
  console.error(
    'Missing OPENAGENTS_ADMIN_API_TOKEN. This is the owner-gated launch path; export the admin bearer token (never commit it) and retry.',
  )
  process.exit(2)
}

const body = readFileSync(resultPath, 'utf8')
const url = `${base.replace(/\/$/, '')}/api/gym/mirrorcode/runs`

const response = await fetch(url, {
  method: 'POST',
  headers: {
    authorization: `Bearer ${token}`,
    'content-type': 'application/json',
  },
  body,
})

const text = await response.text()
console.log(`POST ${url} -> ${response.status}`)
console.log(text)
process.exit(response.ok ? 0 : 1)

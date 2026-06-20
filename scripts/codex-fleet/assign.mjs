#!/usr/bin/env node
import { execSync } from 'node:child_process'
// assign.mjs — promise -> assignment selector for the CODEX fleet runner.
//
// Adapted from scripts/vertex-fleet/assign.mjs. Same promise-registry source,
// same buildable/owner-gated classification, same open-PR dedup. Differences:
//   - the brief is framed for a Codex (gpt-5.5) agent run by `codex exec`;
//   - open-PR dedup matches the `codex-fleet/<promise>` branch prefix;
//   - --priority business prefers business-fulfillment promises first.
//
// Fetches the live public product-promise registry, selects N non-green promises
// that still have *buildable* (non-owner-gated) blockers, and emits one task
// brief per promise.
//
// Output: JSON array of { promiseId, state, model, blockers, brief } to stdout
// (and, with --out <dir>, one <promiseId>.brief.txt + assignments.json on disk).
//
// NO secrets are read or printed. The registry endpoint is public.
//
// Usage:
//   node assign.mjs [--count N] [--state red|yellow|planned|any]
//                   [--model gpt-5.5] [--out DIR] [--ids a,b,c]
//                   [--priority business|any]
//
// Selection is keyword-classified, NOT semantic — acceptable here because the
// universe is the bounded, enum-typed blocker-id list from our own registry,
// and the agent itself decides per the brief which blockers it genuinely cleared.

const PROMISES_URL = 'https://openagents.com/api/public/product-promises'
const BROWSER_UA =
  'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 ' +
  '(KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36'

// Owner-gated / non-buildable signals: anything needing a human decision,
// secret, payment rail, legal/policy sign-off, custody, or quota grant.
const GATED = [
  'privacy_review', 'copy_review', 'copy_gate', 'product_copy', 'pricing',
  'package_policy', 'payout', 'settlement', 'stripe', 'secret', 'token',
  'sign_off', 'signoff', 'manual', 'reauth', 'owner', 'prod_key',
  'production_key', 'legal', 'policy_review', 'marketplace_metering',
  'eligibility', 'custody', 'wallet_fund', 'quota', 'interactive', 'approval',
  'testflight', 'app_store', 'distribution_not_live', 'enablement',
]

// Buildable signals: missing code / docs / tests / scripts / specs / endpoints.
const BUILD = [
  'missing', 'endpoint', 'script', 'runbook', 'doc', 'test', 'harness',
  'runner', 'fixture', 'guide', 'helper', 'spec', 'schema', 'plan', 'audit',
  'wiring', 'adapter', 'projection', 'capture', 'manifest', 'telemetry',
  'report', 'example', 'methodology', 'definition', 'architecture', 'receipts',
]

// Business-fulfillment signals: promises about making/taking money, orders,
// fulfillment, customers, revenue. Used by --priority business to front-load
// the highest-leverage commercial work.
const BUSINESS = [
  'order', 'fulfill', 'customer', 'revenue', 'invoice', 'credit', 'ledger',
  'checkout', 'billing', 'payment', 'sale', 'pricing', 'refund', 'subscription',
  'make_money', 'make-money', 'earn', 'payout', 'settlement', 'commerce',
  'purchase', 'cart', 'receipt', 'business',
]

function isGated(blockerId) {
  const s = String(blockerId).toLowerCase()
  return GATED.some((w) => s.includes(w))
}
function isBuildable(blockerId) {
  const s = String(blockerId).toLowerCase()
  return !isGated(blockerId) && BUILD.some((w) => s.includes(w))
}
function isBusinessPromise(promise) {
  const hay = `${promise.promiseId} ${promise.claim ?? ''} ${
    promise.safeCopy ?? ''
  } ${(promise.blockerRefs ?? []).join(' ')}`.toLowerCase()
  return BUSINESS.some((w) => hay.includes(w))
}

function parseArgs(argv) {
  const a = {
    count: 3,
    state: 'any',
    model: 'gpt-5.5',
    out: null,
    ids: null,
    priority: 'any',
  }
  for (let i = 2; i < argv.length; i++) {
    const k = argv[i]
    if (k === '--count') a.count = parseInt(argv[++i], 10)
    else if (k === '--state') a.state = argv[++i]
    else if (k === '--model') a.model = argv[++i]
    else if (k === '--out') a.out = argv[++i]
    else if (k === '--priority') a.priority = argv[++i]
    else if (k === '--ids')
      a.ids = argv[++i].split(',').map((s) => s.trim()).filter(Boolean)
  }
  return a
}

function briefFor(promise, buildableBlockers, model) {
  const id = promise.promiseId
  const claim = promise.safeCopy || promise.claim || '(no claim text)'
  const blockerList = buildableBlockers.map((b) => `  - ${b}`).join('\n')
  // Codex (gpt-5.5) agent, scoped to ONE promise, PR-per-agent.
  return `You are one worker in the OpenAgents Codex fleet. You are powered by ${model} via the OpenAgents ChatGPT/Codex subscription, run non-interactively by \`codex exec\`. You are working in an ISOLATED git worktree on a branch. Do real, mergeable work — no theater.

PROMISE: ${id}
STATE:   ${promise.state}
CLAIM:   ${claim}

YOUR JOB
Advance this promise by BUILDING THE SMALLEST GENUINE MISSING PIECE for ONE of its buildable blockers below. Produce a real artifact (code, a script, a test, a runbook/spec doc, or a fixture) that a reviewer would merge. Prefer a tightly-scoped, verifiable contribution over a sprawling one.

BUILDABLE BLOCKERS (pick ONE; the rest are out of scope for this run):
${blockerList}

HARD RULES (violating any of these fails the task)
1. NO GREEN FLIPS. Do not change any promise state, do not edit the product-promise registry to mark anything green/yellow, do not touch state fields. Leave promise states exactly as they are.
2. Drop a blocker from a tracking doc ONLY if you genuinely and fully cleared it in THIS change. If you only partially advanced it, leave it listed and note the progress. Honesty over optics.
3. Stay in this worktree. Do not push to main. Do not run git push (the orchestrator handles branch push + PR).
4. Keep the change SMALL and self-contained. Add new files under a sensible path; if editing existing files, keep diffs minimal.
5. Do NOT print or commit any secrets, tokens, keys, or credentials.

WORKFLOW
1. Briefly explore the repo to locate where this promise's evidence/docs/code live (grep for the promiseId and the blocker keywords).
2. Implement the smallest genuine piece for ONE blocker. Create a short markdown note at docs/launch/codex-fleet/${id}.md describing what you built, which blocker it advances, and what remains — UNLESS a more natural home exists, in which case use that and still leave a one-line pointer.
3. Validate — ALL THREE must be clean before you commit:
   (a) \`cd apps/openagents.com/workers/api && bunx tsc -p tsconfig.json --noEmit\` MUST report 0 errors (check:deploy does NOT cover workers/api typecheck — you must run this yourself; fix every TS error your change introduced, no \`any\`/\`@ts-ignore\`).
   (b) \`cd apps/openagents.com && bun run check:deploy\` MUST pass; fix it if your change broke it (note any unrelated pre-existing failure precisely).
   (c) \`git diff --check\` MUST be clean — remove any trailing whitespace you added.
4. Commit your work with \`git add -A && git commit -m "codex-fleet(${id}): <concise summary>"\`. Do NOT push.
5. End with a 3-5 line summary: which blocker you advanced, what you built (file paths), whether check:deploy passed, and what genuinely remains.

Begin now.`
}

async function main() {
  const args = parseArgs(process.argv)
  const res = await fetch(PROMISES_URL, { headers: { 'User-Agent': BROWSER_UA } })
  if (!res.ok) {
    console.error(`assign: registry fetch failed HTTP ${res.status}`)
    process.exit(1)
  }
  const data = await res.json()
  const promises = data.promises || []

  let pool = promises.filter((p) => {
    if (['green', 'withdrawn'].includes(p.state)) return false
    if (args.state !== 'any' && p.state !== args.state) return false
    const brefs = p.blockerRefs || []
    return brefs.some(isBuildable)
  })

  // Dedup: exclude promises that already have an OPEN codex-fleet PR (its branch
  // exists, so `git worktree add -b` would collide). Frees the fleet to pick fresh
  // promises each batch; a promise becomes eligible again once its PR merges/closes.
  try {
    const out = execSync(
      'gh pr list --state open --json headRefName --limit 300',
      { encoding: 'utf8' },
    )
    const taken = new Set()
    for (const pr of JSON.parse(out)) {
      const m = String(pr.headRefName || '').match(/^codex-fleet\/(.+)$/)
      if (m) taken.add(m[1])
    }
    if (taken.size) pool = pool.filter((p) => !taken.has(p.promiseId))
  } catch (e) {
    console.error('assign: open-PR dedup skipped:', String(e).slice(0, 80))
  }

  if (args.ids) {
    const want = new Set(args.ids)
    pool = pool.filter((p) => want.has(p.promiseId))
  }

  // Deterministic, stable ordering. With --priority business, business-fulfillment
  // promises sort first; then most buildable blockers, then id.
  const businessFirst = args.priority === 'business'
  pool.sort((a, b) => {
    if (businessFirst) {
      const ab = isBusinessPromise(a) ? 1 : 0
      const bb = isBusinessPromise(b) ? 1 : 0
      if (bb !== ab) return bb - ab
    }
    const ba = (a.blockerRefs || []).filter(isBuildable).length
    const bbn = (b.blockerRefs || []).filter(isBuildable).length
    if (bbn !== ba) return bbn - ba
    return a.promiseId.localeCompare(b.promiseId)
  })

  const chosen = args.ids ? pool : pool.slice(0, args.count)
  const assignments = chosen.map((p) => {
    const buildable = (p.blockerRefs || []).filter(isBuildable)
    return {
      promiseId: p.promiseId,
      state: p.state,
      model: args.model,
      blockers: buildable,
      brief: briefFor(p, buildable, args.model),
    }
  })

  if (args.out) {
    const fs = await import('node:fs')
    const path = await import('node:path')
    fs.mkdirSync(args.out, { recursive: true })
    for (const a of assignments) {
      const safe = a.promiseId.replace(/[^a-zA-Z0-9._-]/g, '_')
      fs.writeFileSync(path.join(args.out, `${safe}.brief.txt`), a.brief)
    }
    fs.writeFileSync(
      path.join(args.out, 'assignments.json'),
      JSON.stringify(assignments, null, 2),
    )
    console.error(`assign: wrote ${assignments.length} assignment(s) to ${args.out}`)
  }

  process.stdout.write(JSON.stringify(assignments, null, 2) + '\n')
}

main().catch((e) => {
  console.error('assign: fatal', e)
  process.exit(1)
})

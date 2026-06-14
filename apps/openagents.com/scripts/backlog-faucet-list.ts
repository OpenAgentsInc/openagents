#!/usr/bin/env bun
// Backlog faucet maintainer CLI (#4781): decorate real budgeted GitHub backlog
// issues into ref-only NIP-LBR work requests via the LIVE
// POST /api/forum/work-requests surface, then post the lifecycle-linkage comment
// back to each issue so the source issue carries its work-request ref. Uses the
// checked-in faucet contract (backlog-faucet.ts) — ref-only, never copies issue
// bodies; one channel per issue.
//
// Usage:
//   OPENAGENTS_AGENT_TOKEN=... bun scripts/backlog-faucet-list.ts <issue#> [<issue#> ...] \
//     [--budget <sats>] [--repo owner/name] [--verify <ref>] [--deadline <iso>] [--no-comment]

import {
  backlogFaucetDeadlineRef,
  buildBacklogWorkRequestFiling,
  listedIssueCommentBody,
} from '../workers/api/src/backlog-faucet'
import type { GitHubIssueForMarchingOrders } from '../workers/api/src/marching-orders-agent'

const BASE = process.env.PYLON_OPENAGENTS_BASE_URL ?? 'https://openagents.com'
const TOKEN = process.env.OPENAGENTS_AGENT_TOKEN
if (!TOKEN) {
  console.error('OPENAGENTS_AGENT_TOKEN is required (the maintainer/requester agent).')
  process.exit(1)
}

const args = process.argv.slice(2)
const issueNumbers: number[] = []
let budgetSats = 3
let repository = 'OpenAgentsInc/openagents'
let verify = 'command.public.pylon.labor.bun_test'
let deadlineIso = '2026-06-30T00:00:00.000Z'
let postComment = true
for (let i = 0; i < args.length; i++) {
  const a = args[i]!
  if (a === '--budget') budgetSats = Number(args[++i])
  else if (a === '--repo') repository = args[++i]!
  else if (a === '--verify') verify = args[++i]!
  else if (a === '--deadline') deadlineIso = args[++i]!
  else if (a === '--no-comment') postComment = false
  else if (/^\d+$/.test(a)) issueNumbers.push(Number(a))
}
if (issueNumbers.length === 0) {
  console.error('Provide at least one issue number.')
  process.exit(1)
}

async function ghText(cliArgs: string[]): Promise<string> {
  const proc = Bun.spawn(['gh', ...cliArgs], { stdout: 'pipe', stderr: 'pipe' })
  const [out, err, code] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ])
  if (code !== 0) throw new Error(`gh ${cliArgs.join(' ')} failed: ${err.trim()}`)
  return out
}

async function gh(jsonArgs: string[]): Promise<any> {
  const out = await ghText(jsonArgs)
  return out.trim() ? JSON.parse(out) : null
}

// One channel per issue: skip re-commenting if the market-listing marker is
// already present on the issue (idempotent lifecycle linkage).
async function alreadyListed(n: number): Promise<boolean> {
  const d = await gh(['issue', 'view', String(n), '--repo', repository, '--json', 'comments'])
  return (d?.comments ?? []).some((c: any) =>
    String(c.body ?? '').includes('openagents.market.work_request'),
  )
}

async function fetchIssue(n: number): Promise<GitHubIssueForMarchingOrders> {
  const d = await gh(['issue', 'view', String(n), '--repo', repository, '--json', 'number,title,state,url,labels'])
  return {
    html_url: d.url,
    labels: (d.labels ?? []).map((l: any) => l.name as string),
    number: d.number,
    state: d.state.toLowerCase(),
    title: d.title,
  }
}

const deadlineRef = backlogFaucetDeadlineRef(deadlineIso)
const results: any[] = []

for (const n of issueNumbers) {
  try {
    const issue = await fetchIssue(n)
    const filing = buildBacklogWorkRequestFiling(issue, {
      budgetSats,
      deadlineRef,
      repository,
      verificationCommandRef: verify,
    })
    const res = await fetch(`${BASE}/api/forum/work-requests`, {
      method: 'POST',
      headers: {
        authorization: `Bearer ${TOKEN}`,
        'content-type': 'application/json',
        'Idempotency-Key': filing.idempotencyKey,
      },
      body: JSON.stringify(filing.input),
    })
    const body = (await res.json()) as any
    if (!res.ok && !body.workRequest) {
      console.error(`#${n}: POST failed ${res.status}: ${JSON.stringify(body).slice(0, 300)}`)
      results.push({ issue: n, ok: false, status: res.status })
      continue
    }
    const wr = body.workRequest
    const relay = body.relayLink
    const topic = body.topic
    const jobEventId = relay?.jobEventId ?? wr?.jobEventId
    console.log(
      `#${n} listed: workRequest=${wr?.workRequestId} jobEvent=${jobEventId} idempotent=${body.idempotent ?? false}`,
    )
    if (postComment && !(await alreadyListed(n))) {
      const comment = listedIssueCommentBody({
        budgetSats,
        deadlineRef,
        jobEventId: jobEventId ?? 'pending',
        objectiveRef: filing.objectiveRef,
        topicSlug: topic?.slug ?? 'work-requests',
        verificationCommandRef: verify,
        workRequestId: wr?.workRequestId ?? 'pending',
      })
      const tmp = `/tmp/faucet-${n}.md`
      await Bun.write(tmp, comment)
      await ghText(['issue', 'comment', String(n), '--repo', repository, '--body-file', tmp])
      console.log(`#${n} lifecycle comment posted to issue`)
    } else if (postComment) {
      console.log(`#${n} already listed (lifecycle comment present) — skipping comment`)
    }
    results.push({
      issue: n,
      ok: true,
      workRequestId: wr?.workRequestId,
      jobEventId,
      objectiveRef: filing.objectiveRef,
      topicSlug: topic?.slug,
    })
  } catch (e) {
    console.error(`#${n}: ${e instanceof Error ? e.message : String(e)}`)
    results.push({ issue: n, ok: false, error: String(e) })
  }
}

console.log('\n=== faucet listing summary ===')
console.log(JSON.stringify(results, null, 2))
process.exit(0)

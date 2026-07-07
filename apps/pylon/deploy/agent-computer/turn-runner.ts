#!/usr/bin/env bun
/**
 * Agent Computer turn-runner (issue #8503).
 *
 * A bounded, self-contained runtime that runs ONE coding turn inside a
 * Firecracker microVM using the real #8475 workspace-materializer to check out
 * a public repo at a pinned commit, then performs a real, deterministic coding
 * step and emits Khala-shaped runtime events + a result bundle.
 *
 * Honest scope: this exercises intent -> real repo checkout -> real coding step
 * (file edit + staged git diff) -> events -> lifecycle result. It does NOT call
 * a hosted model, so it does NOT mint a model-token usage receipt
 * (`/api/khala/cloud/runtime-turn-usage`) — that path requires a Codex/Claude
 * OAuth login or the hosted Khala gateway (live control plane). The compute
 * lifecycle receipts are emitted by the control-plane provisioner, not here.
 *
 * Input (argv[2] = path to a work-context JSON, or stdin):
 *   {
 *     "workContextRef": "work-context.<...>",
 *     "threadRef": "thread.<...>",
 *     "turnId": "turn-1",
 *     "repo": "owner/name",
 *     "commit": "<40-hex>",
 *     "branch": "main",
 *     "objective": "short public-safe objective"
 *   }
 *
 * Output: newline-delimited runtime events on stdout; a result bundle written to
 * /qa/artifacts/result.json (copied out by the host provisioner).
 */
import { mkdir, writeFile, readdir } from 'node:fs/promises'
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import {
  materializeGitCheckoutWorkspace,
  type GitCheckoutWorkspace,
} from '../../src/workspace-materializer.js'

const ARTIFACT_DIR = process.env.OA_ARTIFACT_DIR ?? '/qa/artifacts'
const CACHE_ROOT = process.env.OA_CACHE_ROOT ?? '/root/.agent-computer/turns'

type WorkContext = {
  workContextRef: string
  threadRef?: string
  turnId?: string
  repo: string
  commit: string
  branch?: string
  objective?: string
}

const nowIso = () => new Date().toISOString()
const events: unknown[] = []
const emit = (event: Record<string, unknown>) => {
  const full = { schema: 'openagents.khala_runtime_event.v1', at: nowIso(), ...event }
  events.push(full)
  process.stdout.write(`${JSON.stringify(full)}\n`)
}

const git = (cwd: string, args: string[]): string => {
  try {
    return execFileSync('git', args, { cwd, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 }).trim()
  } catch (error) {
    const e = error as { stdout?: string; stderr?: string }
    return `${e.stdout ?? ''}${e.stderr ?? ''}`.trim()
  }
}

async function main() {
  const argPath = process.argv[2]
  const raw = argPath ? readFileSync(argPath, 'utf8') : readFileSync(0, 'utf8')
  const wc = JSON.parse(raw) as WorkContext
  if (!wc.repo || !wc.commit || !wc.workContextRef) {
    throw new Error('work context requires repo, commit, workContextRef')
  }
  const branch = wc.branch ?? 'main'
  const turnId = wc.turnId ?? 'turn-1'

  emit({
    kind: 'turn.started',
    turnId,
    workContextRef: wc.workContextRef,
    threadRef: wc.threadRef,
    objective: wc.objective ?? `checkout ${wc.repo}@${wc.commit.slice(0, 12)}`,
  })

  // 1. Real repo checkout via the #8475 materializer (unauthenticated depth-1
  //    clone + detached checkout of the pinned commit; public repo, no broker).
  emit({ kind: 'tool.call', turnId, tool: 'workspace.checkout', repo: wc.repo, commit: wc.commit })
  const checkout: GitCheckoutWorkspace = {
    kind: 'git_checkout',
    repository: {
      provider: 'github',
      fullName: wc.repo,
      commitSha: wc.commit,
      branch,
      visibility: 'public',
    },
    verificationCommand: { commandRef: 'verify.agent-computer.turn', args: ['git', 'status'] },
  }
  const ws = await materializeGitCheckoutWorkspace({
    cacheRoot: CACHE_ROOT,
    checkout,
    leaseRef: `lease.${wc.workContextRef}.${turnId}`,
    refPrefix: 'workspace.agent-computer',
  })
  const head = git(ws.workingDirectory, ['rev-parse', 'HEAD'])
  const subject = git(ws.workingDirectory, ['log', '-1', '--format=%s'])
  const entries = await readdir(ws.workingDirectory)
  emit({
    kind: 'tool.result',
    turnId,
    tool: 'workspace.checkout',
    headCommit: head,
    headSubject: subject,
    topLevelEntries: entries.length,
    workspaceRef: ws.workspaceRef,
  })

  // 2. Real coding step (deterministic, no model): add a proof note file and
  //    produce a genuine staged git diff — real repo mutation + real git.
  const proofPath = join(ws.workingDirectory, 'AGENT_COMPUTER_TURN.md')
  const proofBody =
    `# Agent Computer turn proof\n\n` +
    `- workContextRef: ${wc.workContextRef}\n` +
    `- turnId: ${turnId}\n` +
    `- repo: ${wc.repo}\n` +
    `- baseCommit: ${head}\n` +
    `- ranAt: ${nowIso()}\n` +
    `- host: firecracker microVM (OpenAgents Agent Computer)\n`
  await writeFile(proofPath, proofBody)
  git(ws.workingDirectory, ['add', 'AGENT_COMPUTER_TURN.md'])
  const diff = git(ws.workingDirectory, ['diff', '--cached', '--stat'])
  const diffFull = git(ws.workingDirectory, ['diff', '--cached'])
  emit({ kind: 'text.completed', turnId, text: `Checked out ${wc.repo}@${head.slice(0, 12)} and staged a 1-file change.\n${diff}` })

  // 3. Result bundle (copied out by the host provisioner).
  await mkdir(ARTIFACT_DIR, { recursive: true })
  const result = {
    schemaVersion: 'openagents.agent_computer.turn_result.v1',
    workContextRef: wc.workContextRef,
    threadRef: wc.threadRef ?? null,
    turnId,
    repo: wc.repo,
    baseCommit: head,
    headSubject: subject,
    stagedDiffStat: diff,
    stagedDiffBytes: diffFull.length,
    model: null,
    modelTokenReceipt: null,
    modelTokenReceiptNote:
      'no hosted model invoked in this proof; a model-token usage receipt requires Codex/Claude OAuth or the hosted Khala gateway',
    ranAt: nowIso(),
    events,
  }
  await writeFile(join(ARTIFACT_DIR, 'result.json'), JSON.stringify(result, null, 2))
  await writeFile(join(ARTIFACT_DIR, 'staged.diff'), diffFull)
  emit({ kind: 'turn.finished', turnId, status: 'completed', artifactDir: ARTIFACT_DIR })
}

main().catch((error) => {
  emit({ kind: 'turn.finished', turnId: 'turn-1', status: 'failed', error: String(error) })
  process.exit(1)
})

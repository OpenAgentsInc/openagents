#!/usr/bin/env bun
// Enforce INVARIANTS.md "No GitHub-Hosted CI / Cloud Actions": .github/workflows
// must contain NO workflow files. CI / PR-evidence / scheduling run on owned infra
// and are AGENT/MANUAL-triggered (e.g. `apps/qa-runner/src/pr-comment-run.ts` run by
// an agent that posts the PR comment — see PR #6224), never on GitHub-hosted runners.
import { existsSync, readdirSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..', '..')
const workflowsDir = join(repoRoot, '.github', 'workflows')
const files = existsSync(workflowsDir)
  ? readdirSync(workflowsDir).filter((f) => /\.ya?ml$/i.test(f))
  : []

if (files.length > 0) {
  console.error(
    'INVARIANT VIOLATION — No GitHub-Hosted CI / Cloud Actions (INVARIANTS.md).\n' +
      `  .github/workflows/ must contain no workflow files; found: ${files.join(', ')}\n` +
      '  Remove them. CI/PR-evidence/scheduling run on owned infra and are agent/manual-\n' +
      '  triggered (run apps/qa-runner/src/pr-comment-run.ts agentically; it posts the PR\n' +
      '  comment), not GitHub Actions.',
  )
  process.exit(1)
}
console.log('check:no-github-actions: OK — no GitHub Actions workflows present.')

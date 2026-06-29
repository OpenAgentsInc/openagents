#!/usr/bin/env bun
// Predeploy guard: refuse to deploy unless the local checkout is EXACTLY origin/main.
//
// `bun run deploy` builds and uploads from the local working tree, so a checkout
// that is behind (or diverged from) origin/main silently ships STALE code — even
// though the intended commit is already pushed. This bit us repeatedly (the
// counter/history routes 404'd in prod because the deploy ran from a behind
// checkout). Fail closed: force `git fetch && git merge --ff-only origin/main`
// before any deploy.
//
// Escape hatch for a deliberate non-main / hotfix deploy: `OA_ALLOW_STALE_DEPLOY=1`.
import { execSync } from 'node:child_process'

const git = (cmd) => execSync(`git ${cmd}`, { encoding: 'utf8' }).trim()

if (process.env.OA_ALLOW_STALE_DEPLOY === '1') {
  console.log('check:deploy-from-main: SKIPPED (OA_ALLOW_STALE_DEPLOY=1)')
  process.exit(0)
}

try {
  git('fetch origin main --quiet')
} catch (error) {
  console.error(
    `✘ Refusing to deploy: could not fetch origin/main (${error.message}).`,
  )
  console.error('  Check connectivity, or set OA_ALLOW_STALE_DEPLOY=1 to override.')
  process.exit(1)
}

const local = git('rev-parse HEAD')
const remote = git('rev-parse origin/main')

if (local !== remote) {
  const branch = git('rev-parse --abbrev-ref HEAD')
  console.error('✘ Refusing to deploy: local HEAD is not origin/main (would ship stale code).')
  console.error(`  branch:      ${branch}`)
  console.error(`  local HEAD:  ${local}`)
  console.error(`  origin/main: ${remote}`)
  console.error('  Fix: git fetch origin main && git merge --ff-only origin/main, then redeploy.')
  console.error('  (Override only if you really mean it: OA_ALLOW_STALE_DEPLOY=1)')
  process.exit(1)
}

console.log(`check:deploy-from-main: OK (local == origin/main @ ${local.slice(0, 9)})`)

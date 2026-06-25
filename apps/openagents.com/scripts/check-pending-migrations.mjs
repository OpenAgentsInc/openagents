#!/usr/bin/env bun
// Deploy-migration safety guard (AAR 2026-06-25 — the keystone fix).
//
// WHY THIS EXISTS. The 2026-06-25 gateway-wide 500 outage was caused by shipping
// worker code that read a column added by migration 0234 WITHOUT applying 0234 to
// remote D1 — the worker shipped AHEAD of its schema, so every credential lookup
// threw and `POST /api/v1/chat/completions` returned 500 for all keys. The cause:
// a shortcut deploy (`build:web && bunx wrangler deploy --assets`) that bypassed
// the flaky `verse-launch-smoke` ALSO skipped `wrangler d1 migrations apply`.
//
// WHAT IT DOES. Runs `wrangler d1 migrations list <db> --remote` and EXITS
// NON-ZERO if ANY migration is pending, naming the pending files. The sanctioned
// deploy path (`deploy:safe`) applies migrations FIRST and then runs THIS guard to
// prove zero pending before the worker is uploaded. If a code-ahead-of-schema gap
// ever reappears, the deploy fails loud BEFORE `wrangler deploy` runs.
//
// PURE CORE. `parseMigrationsList` is pure and unit-tested against captured
// wrangler output shapes; the CLI runner only shells out and formats. Requires the
// owner OAuth env (CLOUDFLARE_API_TOKEN / wrangler login) to reach remote D1.
import { execSync } from 'node:child_process'

export const DEFAULT_DATABASE = 'openagents-autopilot'

// Wrangler prints one of a few shapes for `d1 migrations list ... --remote`:
//   - "No migrations to apply!"  (nothing pending — the healthy state)
//   - a table of pending migration files, e.g.
//       ┌──────────────────────────────┐
//       │ Name                         │
//       ├──────────────────────────────┤
//       │ 0234_pylon_openauth_links.sql│
//       └──────────────────────────────┘
//   - "Migrations to be applied:" followed by the file list (older shape)
// Parse defensively: extract any `NNNN_*.sql` migration filenames from the output,
// and treat the explicit "no migrations" sentinel as zero pending. Returns the
// list of pending migration filenames (empty => nothing pending).
export const parseMigrationsList = (rawOutput) => {
  const text = String(rawOutput ?? '')
  if (/no migrations to apply/i.test(text)) {
    return []
  }
  const pending = []
  const seen = new Set()
  const lines = text.split('\n')
  for (const line of lines) {
    // A migration filename is NNNN_<name>.sql. Match it anywhere on the line so
    // we catch both bare lists and table cells (│ 0234_....sql │).
    const matches = line.match(/\b(\d{4}_[A-Za-z0-9_]+\.sql)\b/g)
    if (matches === null) {
      continue
    }
    for (const name of matches) {
      if (!seen.has(name)) {
        seen.add(name)
        pending.push(name)
      }
    }
  }
  return pending
}

// Decide the guard result from parsed pending migrations. Pure, so tests assert
// the exit decision + message without shelling out.
export const decidePendingMigrations = (pending, database = DEFAULT_DATABASE) => {
  if (pending.length === 0) {
    return {
      ok: true,
      exitCode: 0,
      message: `check-pending-migrations: OK — 0 pending migrations on ${database} (remote).`,
    }
  }
  const list = pending.map((name) => `  - ${name}`).join('\n')
  return {
    ok: false,
    exitCode: 1,
    message:
      `✘ check-pending-migrations: ${pending.length} migration(s) PENDING on ${database} (remote):\n${list}\n` +
      `  The worker must NOT ship ahead of its schema (AAR 2026-06-25).\n` +
      `  Apply them first:\n` +
      `    cd workers/api && wrangler d1 migrations apply ${database} --remote\n` +
      `  then re-run the sanctioned deploy (\`bun run deploy:safe\`).`,
  }
}

// Shell out to wrangler and return its combined stdout/stderr text. Isolated so
// the pure parser/decider above stay testable without a network/OAuth dependency.
const runMigrationsList = (database) => {
  // Run from workers/api (where wrangler.jsonc + migrations live) regardless of
  // the caller's cwd, so the guard works from `bun run` at the app root too.
  const cwd = new URL('../workers/api', import.meta.url).pathname
  try {
    return execSync(
      `npx wrangler d1 migrations list ${database} --remote`,
      { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] },
    )
  } catch (error) {
    // wrangler exits non-zero on auth/connectivity failure. Surface stdout/stderr
    // it captured so the operator sees the real cause, then fail closed.
    const captured = `${error.stdout ?? ''}\n${error.stderr ?? ''}`.trim()
    throw new Error(
      `wrangler d1 migrations list failed (auth/connectivity?):\n${captured || error.message}`,
    )
  }
}

const main = () => {
  const database = process.argv[2] ?? DEFAULT_DATABASE
  let raw
  try {
    raw = runMigrationsList(database)
  } catch (error) {
    console.error(`✘ check-pending-migrations: ${error.message}`)
    console.error(
      '  Ensure CLOUDFLARE_API_TOKEN / `wrangler login` is set (owner OAuth env).',
    )
    process.exit(2)
  }
  const pending = parseMigrationsList(raw)
  const decision = decidePendingMigrations(pending, database)
  if (decision.ok) {
    console.log(decision.message)
    process.exit(0)
  }
  console.error(decision.message)
  process.exit(decision.exitCode)
}

// Only run the CLI when invoked directly, not when imported by the test.
if (import.meta.main) {
  main()
}

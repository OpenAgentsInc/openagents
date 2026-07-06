#!/usr/bin/env node

/**
 * ST-1 (#8507): deploy-gate mode of the khala-sync live-seam smoke.
 *
 * Runs inside `deploy:safe` right after the STAGING deploy: self-registers
 * a throwaway staging agent (the exact mechanism
 * predeploy-parallel-dispatch-smoke.mjs uses), then drives the REAL
 * `createHttpKhalaSyncTransport` — bootstrap → logPage → connectLive, plus
 * a real session reaching phase `live` — against staging with that
 * COOKIE-LESS bearer, by executing
 * `packages/khala-sync-client/src/live-seam-smoke.e2e.test.ts` with the
 * KHALA_SYNC_LIVE_SMOKE_* env set.
 *
 * Gate semantics (deliberate):
 * - Cannot OBTAIN a token (registration route down/changed, network fault):
 *   SKIP with a loud warning — never block unrelated deploys on the smoke's
 *   own preconditions.
 * - Token obtained but the seam FAILS (e.g. `withBearerFromQueryToken`
 *   reverted → every cookie-less WS connect 401s, the 2026-07-06 incident):
 *   exit 1 and BLOCK the production deploy.
 *
 * The token is throwaway, staging-only, and never printed.
 */

import { spawnSync } from 'node:child_process'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const defaultBaseUrl = 'https://openagents-staging.openagents.workers.dev'

const scriptsDir = dirname(fileURLToPath(import.meta.url))
const clientPackageDir = resolve(
  scriptsDir,
  '../../../packages/khala-sync-client',
)

const slugPart = value =>
  String(value)
    .toLowerCase()
    .replace(/[^a-z0-9-]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 48) || 'run'

export const registerThrowawaySmokeAgent = async (
  fetchImpl,
  baseUrl,
  runRef,
) => {
  const response = await fetchImpl(new URL('/api/agents/register', baseUrl), {
    body: JSON.stringify({
      displayName: 'Khala sync live-seam smoke',
      externalId: `predeploy.khala_sync.live_seam.${runRef}`,
      metadata: {
        authority: 'staging_predeploy_khala_sync_live_seam_smoke',
        runRef,
      },
      slug: `khala-seam-smoke-${slugPart(runRef)}`.slice(0, 80),
    }),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })
  if (!response.ok) {
    throw new Error(
      `agent self-registration failed with HTTP ${response.status}`,
    )
  }
  const body = await response.json()
  const token = body?.credential?.token
  const userId = body?.user?.id
  if (typeof token !== 'string' || token.trim() === '') {
    throw new Error('agent self-registration returned no credential token')
  }
  if (typeof userId !== 'string' || userId.trim() === '') {
    throw new Error('agent self-registration returned no user id')
  }
  return { token, userId }
}

const main = async () => {
  const baseUrl = (
    process.env.KHALA_SYNC_LIVE_SMOKE_BASE_URL || defaultBaseUrl
  ).replace(/\/+$/, '')
  const runRef = `seam${Date.now().toString(36)}`

  let credentials
  if (
    (process.env.KHALA_SYNC_LIVE_SMOKE_TOKEN || '').trim() !== '' &&
    (process.env.KHALA_SYNC_LIVE_SMOKE_OWNER_USER_ID || '').trim() !== ''
  ) {
    // An env-provided token (owner-directed run) wins over self-registration.
    credentials = {
      token: process.env.KHALA_SYNC_LIVE_SMOKE_TOKEN.trim(),
      userId: process.env.KHALA_SYNC_LIVE_SMOKE_OWNER_USER_ID.trim(),
    }
  } else {
    try {
      credentials = await registerThrowawaySmokeAgent(
        globalThis.fetch,
        baseUrl,
        runRef,
      )
    } catch (error) {
      // SKIP, never fail: the smoke must not block unrelated deploys when
      // it cannot obtain its own credential.
      console.warn(
        `⚠ khala-sync live-seam smoke SKIPPED: could not self-register a throwaway staging agent against ${baseUrl} (${error.message}). ` +
          'The bearer-WS-connect seam was NOT verified for this deploy — investigate before the next release.',
      )
      return
    }
  }

  console.log(
    `→ khala-sync live-seam smoke: driving the real transport against ${baseUrl} with a cookie-less bearer (user ${credentials.userId}).`,
  )

  const result = spawnSync('bun', ['test', 'src/live-seam-smoke.e2e.test.ts'], {
    cwd: clientPackageDir,
    env: {
      ...process.env,
      KHALA_SYNC_LIVE_SMOKE_BASE_URL: baseUrl,
      KHALA_SYNC_LIVE_SMOKE_TOKEN: credentials.token,
      KHALA_SYNC_LIVE_SMOKE_OWNER_USER_ID: credentials.userId,
    },
    stdio: 'inherit',
  })

  if (result.status !== 0) {
    console.error(
      '✘ khala-sync live-seam smoke FAILED: a cookie-less bearer could not complete bootstrap → logPage → connectLive → live against staging. ' +
        'This is the 2026-07-06 WebSocket-auth incident signature (see docs/khala-code/2026-07-06-mobile-loading-threads-websocket-auth-audit.md). ' +
        'Blocking the production deploy.',
    )
    process.exit(result.status ?? 1)
  }

  console.log('✔ khala-sync live-seam smoke passed against staging.')
}

if (import.meta.main) {
  main().catch(error => {
    console.error(`✘ khala-sync live-seam smoke errored: ${error.message}`)
    process.exit(1)
  })
}

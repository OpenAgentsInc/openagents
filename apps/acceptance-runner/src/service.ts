#!/usr/bin/env bun
// Long-running daemon ENTRYPOINT for the out-of-Worker acceptance runner (EPIC #6017).
//
// This is what runs on the host (our GCE box / Cloud Run / a Pylon). It reads the
// FAIL-CLOSED config from env, builds the HTTP job source + transport, and starts the
// poll loop. With required secrets missing it exits non-zero WITHOUT polling — a
// half-configured host never runs and never invents work.
//
// Required env (the host's local secrets — see apps/acceptance-runner/docs/DEPLOY.md):
//   ACCEPTANCE_VERDICT_CALLBACK_URL   the Worker verdict callback (POST target)
//   ACCEPTANCE_JOB_LEASE_URL          the Worker job-lease endpoint (GET)
//   ACCEPTANCE_JOB_ACK_URL            the Worker job-ack endpoint (POST)
//   ACCEPTANCE_VERDICT_CALLBACK_TOKEN the shared runner bearer token (never logged)
// Optional: ACCEPTANCE_POLL_INTERVAL_MS, ACCEPTANCE_IDLE_BACKOFF_MS,
//   ACCEPTANCE_NAV_TIMEOUT_MS.
//
// Prereq on the host: `bunx playwright install --with-deps chromium`.

import process from 'node:process'

import { readRunnerServiceConfig } from './config'
import { startRunnerDaemon, type DaemonLogEvent } from './daemon'
import { makeHttpJobSource } from './http-job-source'

const logEvent = (event: DaemonLogEvent): void => {
  // Structured, secret-free JSON line to stdout (the host captures it). Never the token,
  // artifact bytes, or prompt.
  console.log(JSON.stringify({ at: new Date().toISOString(), ...event }))
}

const main = async (): Promise<void> => {
  const result = readRunnerServiceConfig(process.env)
  if (!result.ok) {
    console.error(
      `acceptance-runner: refusing to start — missing required env: ${result.missing.join(', ')}`,
    )
    process.exit(2)
  }
  const { config } = result

  const source = makeHttpJobSource({
    ackUrl: config.jobAckUrl,
    bearerToken: config.bearerToken,
    leaseUrl: config.jobLeaseUrl,
  })

  console.log(
    JSON.stringify({
      at: new Date().toISOString(),
      kind: 'start',
      pollIntervalMs: config.pollIntervalMs,
      source: source.label,
      verdictCallback: config.verdictCallbackUrl,
    }),
  )

  const handle = startRunnerDaemon({ config, log: logEvent, source })

  const shutdown = (signal: string): void => {
    console.log(
      JSON.stringify({ at: new Date().toISOString(), kind: 'shutdown', signal }),
    )
    handle.stop()
  }
  process.on('SIGTERM', () => shutdown('SIGTERM'))
  process.on('SIGINT', () => shutdown('SIGINT'))

  await handle.done
  process.exit(0)
}

if (import.meta.main) {
  await main()
}

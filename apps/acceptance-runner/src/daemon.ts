// The out-of-Worker acceptance-runner DAEMON (EPIC #6017).
//
// THE MISSING PIECE. The gateway already dispatches jobs (inert) and ingests verdicts
// (inert), and the harness (`runAcceptanceJob`) already runs one job end to end. What
// did not exist was a long-running SERVICE that sits on a node with chromium and turns
// the loop: lease a pending job -> run the real headless acceptance suite -> POST the
// verdict back -> ack the job. This daemon is exactly that, and NOTHING ELSE — it owns
// no money path, no settlement, no receipt logic; it only ORCHESTRATES the canonical
// harness on a host that can run a browser.
//
//   gateway (Worker)  ─lease──▶  DAEMON (this) ──runAcceptanceJob──▶ chromium
//        ▲   ▲                       │                                  │
//        │   └──────── ack ──────────┘                                  │
//        └──────────── verdict POST (bearer token) ────────────────────┘
//
// CONSTANT-MOTION SAFE + FAIL-SOFT: one tick leases AT MOST one job; an empty lease
// backs off `idleBackoffMs`; a lease/transport fault backs off too (never a busy-spin,
// never a fabricated verdict). A run NEVER throws into the loop — a crashed artifact is
// an honest all-fail verdict from the harness, posted back so the receipt backfills to
// `failed`. A verdict-delivery failure acks the job as retryable so it is re-leased.

import {
  type AcceptanceJobMessage,
  type RunAcceptanceJobResult,
  type RunnerTransport,
  makeFetchVerdictPoster,
  runAcceptanceJob,
} from './harness-bridge'
import type { RunnerServiceConfig } from './config'
import type { JobSource } from './job-source'

// The job-runner seam: takes a transport + a job message + options and returns the
// run result. The default is the canonical `runAcceptanceJob` (real Playwright/chromium
// harness); tests inject a browser-free fake. This keeps the daemon's LOOP logic
// (lease/run/post/ack/backoff) unit-testable without standing up a browser, while prod
// runs the real headless suite unchanged.
export type RunJobFn = (
  transport: RunnerTransport,
  message: AcceptanceJobMessage,
  options?: Readonly<{ navTimeoutMs?: number }>,
) => Promise<RunAcceptanceJobResult>

// Resolve an artifact ref to its HTML over HTTP. The Worker mints a dereferenceable ref
// (an R2-signed GET URL) and carries it on the job; the runner GETs the bytes. A non-2xx
// rejects so the harness turns it into an honest all-fail verdict (never a silent skip).
export const makeHttpArtifactResolver =
  (fetchFn: typeof fetch = fetch): RunnerTransport['resolveArtifact'] =>
  async (artifactRef: string): Promise<string> => {
    const response = await fetchFn(artifactRef, { method: 'GET' })
    if (!response.ok) {
      throw new Error(`artifact_fetch_failed: ${response.status}`)
    }
    return response.text()
  }

// A structured, public-safe log line (no token, no artifact bytes, no prompt). The host
// captures stdout; we keep it greppable and free of secrets.
export type DaemonLogEvent =
  | Readonly<{ kind: 'idle'; source: string }>
  | Readonly<{ kind: 'lease_error'; source: string; message: string }>
  | Readonly<{
      kind: 'verdict'
      requestId: string
      verified: boolean
      scalarReward: number
      passed: number
      total: number
      delivered: boolean
    }>

export type RunnerDaemonDeps = Readonly<{
  config: RunnerServiceConfig
  source: JobSource
  // Injectable so tests drive the daemon without a real browser / network. Defaults to
  // the real HTTP resolver + the real fetch-backed verdict poster built from config.
  transport?: RunnerTransport
  log?: (event: DaemonLogEvent) => void
  // Injectable clock for the idle backoff (tests pass a no-op).
  sleep?: (ms: number) => Promise<void>
  // Injectable job runner (defaults to the real headless harness). Tests pass a
  // browser-free fake so the LOOP wiring is provable without chromium.
  runJob?: RunJobFn
}>

const defaultSleep = (ms: number): Promise<void> =>
  new Promise(resolve => setTimeout(resolve, ms))

// Build the real `RunnerTransport` from config: HTTP artifact resolution + a
// fetch-backed verdict poster authenticated by the bearer token. The token is read from
// config (the host's local secret), never hard-coded, never logged.
export const makeRunnerTransport = (
  config: RunnerServiceConfig,
  fetchFn: typeof fetch = fetch,
): RunnerTransport => ({
  postVerdict: makeFetchVerdictPoster({
    bearerToken: config.bearerToken,
    callbackUrl: config.verdictCallbackUrl,
    fetchFn,
  }),
  resolveArtifact: makeHttpArtifactResolver(fetchFn),
})

// Run EXACTLY ONE daemon tick: lease at most one job, run it, post + ack. Returns what
// happened so a caller (the loop, or a test) can decide whether to back off. Pure w.r.t.
// the injected deps — no global state, no hidden I/O.
export const runDaemonTick = async (
  deps: RunnerDaemonDeps,
): Promise<
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'lease_error'; message: string }>
  | Readonly<{ kind: 'ran'; result: RunAcceptanceJobResult }>
> => {
  const transport = deps.transport ?? makeRunnerTransport(deps.config)
  const log = deps.log ?? (() => undefined)

  let leased
  try {
    leased = await deps.source.lease()
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    log({ kind: 'lease_error', message, source: deps.source.label })
    return { kind: 'lease_error', message }
  }

  if (leased === null) {
    log({ kind: 'idle', source: deps.source.label })
    return { kind: 'idle' }
  }

  // Run the canonical harness end to end. It NEVER throws into us — a load crash / an
  // unresolvable artifact is an honest all-fail verdict, already posted by the harness.
  const runJob = deps.runJob ?? runAcceptanceJob
  const result = await runJob(transport, leased.message, {
    navTimeoutMs: deps.config.navTimeoutMs,
  })

  // Ack the lease: delivered when the verdict POST succeeded (the Worker backfilled the
  // receipt), retryable otherwise (re-lease so the verdict is not lost). The ack itself
  // is fail-soft — an ack hiccup does not crash the loop; the job's lease simply expires
  // and it is re-leased.
  await deps.source
    .ack({ delivered: result.delivered, leaseId: leased.leaseId })
    .catch(() => undefined)

  log({
    delivered: result.delivered,
    kind: 'verdict',
    passed: result.verdict.passedChecks.length,
    requestId: leased.message.requestId,
    scalarReward: result.verdict.scalarReward,
    total: result.verdict.checks.length,
    verified: result.verdict.verified,
  })
  return { kind: 'ran', result }
}

export type RunnerDaemonHandle = Readonly<{
  // Resolves when the loop has stopped after a `stop()` request and the in-flight tick
  // (if any) finished.
  done: Promise<void>
  // Request a graceful stop. The current tick finishes; no new tick starts.
  stop: () => void
}>

// Start the long-running poll loop. CONSTANT MOTION between jobs (poll cadence) with a
// longer idle/error backoff so an empty queue or a transport blip never busy-spins. The
// loop is graceful-stoppable for clean shutdown (SIGTERM on the host).
export const startRunnerDaemon = (
  deps: RunnerDaemonDeps,
): RunnerDaemonHandle => {
  const sleep = deps.sleep ?? defaultSleep
  let stopped = false
  const loop = async (): Promise<void> => {
    while (!stopped) {
      const outcome = await runDaemonTick(deps)
      if (stopped) break
      if (outcome.kind === 'ran') {
        // Pulled a job; immediately try for the next (constant motion).
        await sleep(deps.config.pollIntervalMs)
      } else {
        // Idle queue or a transport fault: back off before polling again.
        await sleep(deps.config.idleBackoffMs)
      }
    }
  }
  const done = loop()
  return {
    done,
    stop: () => {
      stopped = true
    },
  }
}

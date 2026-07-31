/**
 * Do not accept traffic on an instance whose database is not reachable yet.
 *
 * INCIDENT 2026-07-31 (P0). `POST /api/omega/auth/session` flapped in
 * production — 200 -> 503 -> 200 within seconds for the same identity, each
 * failure taking ~30 seconds. Every 503 landed within 10-70 SECONDS OF A NEW
 * CLOUD RUN REVISION STARTING (revisions 00307, 00324, 00328, 00334, 00336 all
 * reproduced it). The instance-level cause is in the sidecar's own logs:
 *
 *   Cloud SQL connection failed ... refresh failed: context deadline exceeded
 *   [openagentsgemini:us-central1:khala-sync-pg] failed to connect to instance:
 *     SFEClient is nil
 *   dial tcp 34.70.178.7:3307: i/o timeout
 *
 * The Cloud SQL Auth Proxy sidecar has not completed its first instance
 * refresh, so `/cloudsql/<instance>/.s.PGSQL.5432` refuses connections. Cloud
 * Run, meanwhile, routes traffic to the instance the moment the HTTP server
 * listens, and `GET /internal/healthz` answered `{ok:true}` without ever
 * touching the database — so a demonstrably unusable instance advertised
 * itself as healthy.
 *
 * THE GATE: hold `listen` until one real `SELECT 1` succeeds. Because Cloud Run
 * derives readiness from the port opening, withholding the listen withholds the
 * traffic, with no probe configuration to keep in sync in the deploy script and
 * no change to how any other lane deploys.
 *
 * IT IS DELIBERATELY FAIL-OPEN AT THE BUDGET. If the database is genuinely down
 * (not merely warming), refusing to start would make the service undeployable
 * and would take down every route that needs no database at all. So the gate
 * waits a bounded time, then starts anyway with a loud, single-line ERROR
 * record. Requests that still need Postgres in that state fail with the honest,
 * newly-distinguished typed codes rather than silently.
 */

export type PostgresReadinessProbe = () => Promise<void>

export type PostgresReadinessOptions = Readonly<{
  /** Total wall-clock budget before the gate gives up and starts anyway. */
  budgetMs?: number
  /** Wait between attempts. */
  intervalMs?: number
  /** Test seam. Default: `Date.now`. */
  now?: () => number
  /** Called once per failed attempt, for a single-line operator log. */
  onAttemptFailed?: (
    info: Readonly<{ attempt: number; elapsedMs: number; error: unknown }>,
  ) => void
  /** Test seam. Default: a real timer. */
  sleep?: (ms: number) => Promise<void>
}>

export type PostgresReadinessOutcome = Readonly<{
  attempts: number
  elapsedMs: number
  ready: boolean
}>

/**
 * Budget defaults.
 *
 * The measured proxy warmup on this service was 10-70s across the 2026-07-31
 * revisions, so 90s covers the observed worst case with headroom. It stays well
 * under Cloud Run's 240s default startup timeout, so a slow-but-recovering
 * database still produces a healthy revision rather than a failed deploy.
 */
const DEFAULT_BUDGET_MS = 90_000
const DEFAULT_INTERVAL_MS = 1_000

const defaultSleep = (ms: number): Promise<void> =>
  new Promise(resolve => {
    setTimeout(resolve, ms)
  })

/**
 * Probe until Postgres answers, or until the budget is spent.
 *
 * Never throws: a readiness gate that can crash the process is a worse outage
 * than the one it prevents.
 */
export const awaitPostgresReady = async (
  probe: PostgresReadinessProbe,
  options: PostgresReadinessOptions = {},
): Promise<PostgresReadinessOutcome> => {
  const budgetMs = options.budgetMs ?? DEFAULT_BUDGET_MS
  const intervalMs = options.intervalMs ?? DEFAULT_INTERVAL_MS
  const now = options.now ?? Date.now
  const sleep = options.sleep ?? defaultSleep
  const startedAt = now()

  for (let attempt = 1; ; attempt += 1) {
    try {
      await probe()
      return { attempts: attempt, elapsedMs: now() - startedAt, ready: true }
    } catch (error) {
      const elapsedMs = now() - startedAt
      options.onAttemptFailed?.({ attempt, elapsedMs, error })
      if (elapsedMs + intervalMs >= budgetMs) {
        return { attempts: attempt, elapsedMs, ready: false }
      }
      await sleep(intervalMs)
    }
  }
}

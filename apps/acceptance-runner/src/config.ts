// Runner-service configuration (EPIC #6017). Reads the host's local env into a typed,
// FAIL-CLOSED config. The service is INERT without its required secrets: no callback URL
// or no bearer token => the service refuses to start the poll loop (it has nowhere to
// deliver verdicts and nothing authenticates it), so a half-configured host never runs.
//
// The bearer token is the SAME ACCEPTANCE_VERDICT_CALLBACK_TOKEN the Worker's verdict
// callback verifies — ONE secret authenticates the whole runner<->gateway channel
// (lease + ack + verdict POST). It is read from env, NEVER hard-coded, and never logged.

export type RunnerServiceConfig = Readonly<{
  // Where the runner POSTs verdicts back (the Worker verdict callback). REQUIRED.
  verdictCallbackUrl: string
  // The authenticated job-lease endpoint (GET) on the Worker. REQUIRED for daemon mode.
  jobLeaseUrl: string
  // The authenticated job-ack endpoint (POST) on the Worker. REQUIRED for daemon mode.
  jobAckUrl: string
  // The runner bearer token (ACCEPTANCE_VERDICT_CALLBACK_TOKEN). REQUIRED. Never logged.
  bearerToken: string
  // How an artifact ref resolves to HTML. 'http' (default) treats the ref as a URL the
  // runner GETs (an R2-signed URL minted by the Worker). Extensible for a local store.
  artifactResolveMode: 'http'
  // Poll cadence + idle backoff (ms). Bounded so a misconfigured value can't busy-spin.
  pollIntervalMs: number
  idleBackoffMs: number
  // Per-job nav timeout passed to the headless suite (ms).
  navTimeoutMs: number
}>

export type RunnerServiceConfigResult =
  | Readonly<{ ok: true; config: RunnerServiceConfig }>
  | Readonly<{ ok: false; missing: ReadonlyArray<string> }>

const clampInt = (
  raw: string | undefined,
  fallback: number,
  min: number,
  max: number,
): number => {
  if (raw === undefined) return fallback
  const parsed = Number.parseInt(raw, 10)
  if (!Number.isFinite(parsed)) return fallback
  return Math.min(max, Math.max(min, parsed))
}

const nonEmpty = (value: string | undefined): string | undefined => {
  const trimmed = value?.trim()
  return trimmed === undefined || trimmed === '' ? undefined : trimmed
}

// Read the daemon config from a plain env record (process.env in prod, a fixture in
// tests). FAIL-CLOSED: returns the missing required keys rather than a partial config.
export const readRunnerServiceConfig = (
  env: Readonly<Record<string, string | undefined>>,
): RunnerServiceConfigResult => {
  const verdictCallbackUrl = nonEmpty(env.ACCEPTANCE_VERDICT_CALLBACK_URL)
  const jobLeaseUrl = nonEmpty(env.ACCEPTANCE_JOB_LEASE_URL)
  const jobAckUrl = nonEmpty(env.ACCEPTANCE_JOB_ACK_URL)
  const bearerToken = nonEmpty(env.ACCEPTANCE_VERDICT_CALLBACK_TOKEN)

  const missing: string[] = []
  if (verdictCallbackUrl === undefined) {
    missing.push('ACCEPTANCE_VERDICT_CALLBACK_URL')
  }
  if (jobLeaseUrl === undefined) missing.push('ACCEPTANCE_JOB_LEASE_URL')
  if (jobAckUrl === undefined) missing.push('ACCEPTANCE_JOB_ACK_URL')
  if (bearerToken === undefined) {
    missing.push('ACCEPTANCE_VERDICT_CALLBACK_TOKEN')
  }
  if (missing.length > 0) return { missing, ok: false }

  return {
    config: {
      artifactResolveMode: 'http',
      bearerToken: bearerToken!,
      idleBackoffMs: clampInt(env.ACCEPTANCE_IDLE_BACKOFF_MS, 5_000, 250, 60_000),
      jobAckUrl: jobAckUrl!,
      jobLeaseUrl: jobLeaseUrl!,
      navTimeoutMs: clampInt(env.ACCEPTANCE_NAV_TIMEOUT_MS, 15_000, 1_000, 120_000),
      pollIntervalMs: clampInt(env.ACCEPTANCE_POLL_INTERVAL_MS, 1_000, 100, 60_000),
      verdictCallbackUrl: verdictCallbackUrl!,
    },
    ok: true,
  }
}

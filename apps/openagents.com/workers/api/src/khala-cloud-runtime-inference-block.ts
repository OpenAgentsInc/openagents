// Seam A (#8503, AC-1) — inference-block + work-context builder (pure).
//
// Builds the exact `inference` block and the enclosing work-context the baked
// in-guest turn-runner (`apps/pylon/deploy/agent-computer/turn-runner.ts`)
// consumes, then base64-encodes the work-context to the OPAQUE
// `work_context_b64` blob the cloud daemon decodes into `/tmp/wc.json`. The
// contract here is byte-for-byte the turn-runner's `InferenceConfig` +
// `WorkContext` types; keeping it a pure function makes it fully unit-provable
// off the live path.
//
// SINGLE-CHARGE. `noMeterSecret` (when present) becomes the microVM's
// `x-openagents-org-cloud-runtime-no-meter` header, suppressing the gateway's
// OWN metering for the internal org-capacity `/v1/chat/completions` call so the
// SINGLE billable `token_usage_events` row is the owner-attributed usage
// receipt. Omit it and the gateway meters normally (fail-closed).
//
// SECRET DISCIPLINE. `agentToken` and `noMeterSecret` live only inside the
// returned blob, which the caller passes straight to the placement adapter and
// then discards. This module never logs them; the blob is opaque to the daemon.

/** The turn-runner's `InferenceConfig`, re-stated as the builder's output. */
export type CloudRuntimeInferenceConfig = Readonly<{
  baseUrl: string
  agentToken: string
  ownerUserId: string
  model: string
  lane: string
  provider?: string
  backendProfile?: string
  pylonRef?: string
  noMeterSecret?: string
}>

/**
 * The turn-runner's `WritebackConfig` (MM-C5 #8477), re-stated as the builder's
 * output. PUBLIC-SAFE ONLY — it carries NO credential. The microVM brokers a
 * short-lived GitHub credential at push time (`noRawUserOAuthTokens: true`).
 */
export type CloudRuntimeWritebackConfig = Readonly<{
  ingestPath: string
  repositoryFullName: string
  baseBranch: string
  branch: string
  mode: 'branch_only' | 'pull_request'
}>

/** The turn-runner's `WorkContext`, re-stated as the builder's output. */
export type CloudRuntimeWorkContext = Readonly<{
  workContextRef: string
  threadRef: string
  turnId: string
  repo: string
  commit: string
  branch: string
  objective: string
  inference: CloudRuntimeInferenceConfig
  writeback?: CloudRuntimeWritebackConfig
}>

/** Default lane the ingest route accepts for the hosted-Khala model turn. */
export const CLOUD_RUNTIME_INFERENCE_DEFAULT_LANE = 'hosted_khala'
/** Default branch when a work-context does not pin one. */
export const CLOUD_RUNTIME_DEFAULT_BRANCH = 'main'
/** The Worker writeback ingest route the microVM POSTs its outcome to (#8477). */
export const CLOUD_RUNTIME_WRITEBACK_DEFAULT_INGEST_PATH =
  '/api/khala/cloud/runtime-turn-writeback'
/** Default writeback mode when a work-context does not choose one. */
export const CLOUD_RUNTIME_WRITEBACK_DEFAULT_MODE = 'pull_request' as const
/** Scoped-branch prefix. The microVM refuses any push that is not under it. */
export const CLOUD_RUNTIME_WRITEBACK_BRANCH_PREFIX = 'pylon/agent-computer-'

export type BuildWritebackConfigInput = Readonly<{
  repositoryFullName: string
  turnId: string
  baseBranch?: string | undefined
  branch?: string | undefined
  mode?: 'branch_only' | 'pull_request' | undefined
  ingestPath?: string | undefined
}>

/**
 * Build the public-safe `writeback` block. The scoped branch is deterministic
 * from the turn id (so a retried turn lands on the same branch, never a new
 * one), always under {@link CLOUD_RUNTIME_WRITEBACK_BRANCH_PREFIX}, never the
 * base branch. Carries NO credential — the microVM brokers one at push time.
 */
export const buildCloudRuntimeWritebackConfig = (
  input: BuildWritebackConfigInput,
): CloudRuntimeWritebackConfig => {
  const baseBranch =
    input.baseBranch !== undefined && input.baseBranch.length > 0
      ? input.baseBranch
      : CLOUD_RUNTIME_DEFAULT_BRANCH
  const scoped = `${CLOUD_RUNTIME_WRITEBACK_BRANCH_PREFIX}${input.turnId
    .replace(/[^A-Za-z0-9._-]/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 80)}`
  const branch =
    input.branch !== undefined &&
    input.branch.startsWith('pylon/') &&
    input.branch !== baseBranch
      ? input.branch
      : scoped
  return {
    baseBranch,
    branch,
    ingestPath: input.ingestPath ?? CLOUD_RUNTIME_WRITEBACK_DEFAULT_INGEST_PATH,
    mode: input.mode ?? CLOUD_RUNTIME_WRITEBACK_DEFAULT_MODE,
    repositoryFullName: input.repositoryFullName,
  }
}

export type BuildInferenceConfigInput = Readonly<{
  baseUrl: string
  agentToken: string
  ownerUserId: string
  model: string
  lane?: string | undefined
  provider?: string | undefined
  backendProfile?: string | undefined
  pylonRef?: string | undefined
  noMeterSecret?: string | undefined
}>

/**
 * Build the `inference` block. Optional keys (`provider`, `backendProfile`,
 * `pylonRef`, `noMeterSecret`) are OMITTED when undefined so the serialized
 * blob matches the turn-runner's field-presence expectations exactly (e.g. the
 * no-meter header is only sent when `noMeterSecret` is present).
 */
export const buildCloudRuntimeInferenceConfig = (
  input: BuildInferenceConfigInput,
): CloudRuntimeInferenceConfig => ({
  agentToken: input.agentToken,
  baseUrl: input.baseUrl,
  lane: input.lane ?? CLOUD_RUNTIME_INFERENCE_DEFAULT_LANE,
  model: input.model,
  ownerUserId: input.ownerUserId,
  ...(input.provider === undefined ? {} : { provider: input.provider }),
  ...(input.backendProfile === undefined
    ? {}
    : { backendProfile: input.backendProfile }),
  ...(input.pylonRef === undefined ? {} : { pylonRef: input.pylonRef }),
  ...(input.noMeterSecret === undefined
    ? {}
    : { noMeterSecret: input.noMeterSecret }),
})

export type BuildWorkContextInput = Readonly<{
  workContextRef: string
  threadRef: string
  turnId: string
  repo: string
  commit: string
  branch?: string | undefined
  objective?: string | undefined
  inference: CloudRuntimeInferenceConfig
  writeback?: CloudRuntimeWritebackConfig | undefined
}>

/** Build the full work-context object the turn-runner reads from `/tmp/wc.json`. */
export const buildCloudRuntimeWorkContext = (
  input: BuildWorkContextInput,
): CloudRuntimeWorkContext => ({
  branch: input.branch ?? CLOUD_RUNTIME_DEFAULT_BRANCH,
  commit: input.commit,
  inference: input.inference,
  objective:
    input.objective ??
    `agent-computer turn ${input.repo}@${input.commit.slice(0, 12)}`,
  repo: input.repo,
  threadRef: input.threadRef,
  turnId: input.turnId,
  workContextRef: input.workContextRef,
  ...(input.writeback === undefined ? {} : { writeback: input.writeback }),
})

/**
 * Encode a work-context to the standard-base64 `work_context_b64` blob the
 * cloud daemon decodes with `base64 -d`. UTF-8 safe (objective text may carry
 * non-ASCII); the alphabet is standard base64 (`A-Za-z0-9+/=`), which the
 * daemon's `is_valid_work_context_b64` gate accepts.
 */
export const encodeWorkContextB64 = (
  workContext: CloudRuntimeWorkContext,
): string => {
  const json = JSON.stringify(workContext)
  const utf8 = new TextEncoder().encode(json)
  let binary = ''
  for (const byte of utf8) binary += String.fromCharCode(byte)
  return btoa(binary)
}

/** Decode a `work_context_b64` blob back to its work-context (test/inspection). */
export const decodeWorkContextB64 = (b64: string): CloudRuntimeWorkContext => {
  const binary = atob(b64)
  const bytes = Uint8Array.from(binary, ch => ch.charCodeAt(0))
  return JSON.parse(new TextDecoder().decode(bytes)) as CloudRuntimeWorkContext
}

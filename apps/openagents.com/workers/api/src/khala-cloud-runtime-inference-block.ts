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

import { Schema as S } from 'effect'

import { parseJsonUnknown } from './json-boundary'

export const AGENT_COMPUTER_HARNESS_IDS = [
  'codex',
  'claude-code',
  'cursor',
  'goose',
  'opencode',
  'pi',
  'grok',
] as const

export const AgentComputerHarnessId = S.Literals(AGENT_COMPUTER_HARNESS_IDS)
export type AgentComputerHarnessId = typeof AgentComputerHarnessId.Type

export const MANAGED_AGENT_COMPUTER_DEFAULT_HARNESS = 'codex' as const
export const MANAGED_AGENT_COMPUTER_GEMINI_MODEL = 'gemini-3.5-flash' as const

export const ManagedAgentComputerHarnessSelection = S.TaggedUnion({
  codex: {
    harnessId: S.Literal('codex'),
    provider: S.Literal('chatgpt_codex'),
    requestedAction: S.Literal('agent_computer_codex_turn'),
  },
  claude: {
    harnessId: S.Literal('claude-code'),
    provider: S.Literal('anthropic_claude'),
    requestedAction: S.Literal('agent_computer_claude_turn'),
  },
  gemini: {
    harnessId: S.Literals(['goose', 'opencode', 'pi']),
    model: S.Literal(MANAGED_AGENT_COMPUTER_GEMINI_MODEL),
    provider: S.Literal('google_gemini'),
    requestedAction: S.Literal('agent_computer_gemini_turn'),
  },
  unavailable: {
    harnessId: S.Literals(['cursor', 'grok']),
    reasonRef: S.Literals([
      'agent_computer_cursor_auth_mode_unavailable',
      'agent_computer_grok_auth_mode_unavailable',
    ]),
  },
}).annotate({ identifier: 'ManagedAgentComputerHarnessSelection' })
export type ManagedAgentComputerHarnessSelection =
  typeof ManagedAgentComputerHarnessSelection.Type

export const selectManagedAgentComputerHarness = (
  harnessId: AgentComputerHarnessId = MANAGED_AGENT_COMPUTER_DEFAULT_HARNESS,
): ManagedAgentComputerHarnessSelection => {
  switch (harnessId) {
    case 'codex':
      return ManagedAgentComputerHarnessSelection.cases.codex.make({
        harnessId,
        provider: 'chatgpt_codex',
        requestedAction: 'agent_computer_codex_turn',
      })
    case 'claude-code':
      return ManagedAgentComputerHarnessSelection.cases.claude.make({
        harnessId,
        provider: 'anthropic_claude',
        requestedAction: 'agent_computer_claude_turn',
      })
    case 'goose':
    case 'opencode':
    case 'pi':
      return ManagedAgentComputerHarnessSelection.cases.gemini.make({
        harnessId,
        model: MANAGED_AGENT_COMPUTER_GEMINI_MODEL,
        provider: 'google_gemini',
        requestedAction: 'agent_computer_gemini_turn',
      })
    case 'cursor':
      return ManagedAgentComputerHarnessSelection.cases.unavailable.make({
        harnessId,
        reasonRef: 'agent_computer_cursor_auth_mode_unavailable',
      })
    case 'grok':
      return ManagedAgentComputerHarnessSelection.cases.unavailable.make({
        harnessId,
        reasonRef: 'agent_computer_grok_auth_mode_unavailable',
      })
  }
}

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

/** Short-lived provider-auth redemption request for the VM. */
export type CloudRuntimeCodexProviderAuthConfig = Readonly<{
  baseUrl: string
  agentToken: string
  providerAccountRef: string
  authGrantRef: string
}>

export type CloudRuntimeCodexTurnConfig = Readonly<{
  baseUrl: string
  agentToken: string
  ownerUserId: string
  pylonRef?: string | undefined
  model?: string | undefined
  maxTurnSeconds?: number | undefined
}>

export type CloudRuntimeClaudeProviderAuthConfig = Readonly<{
  baseUrl: string
  agentToken: string
  providerAccountRef: string
  authGrantRef: string
}>

export type CloudRuntimeHarnessRuntimeSecretGrant = Readonly<{
  kind: 'gemini_api_key'
  baseUrl: string
  agentToken: string
  grantRef: string
  providerAccountRef: string
  runnerSessionId: string
  secretRef: string
}>

export type CloudRuntimeHarnessTurnConfig = Readonly<{
  harness: Exclude<AgentComputerHarnessId, 'codex'>
  model?: string | undefined
  runtimeSecretGrant?: CloudRuntimeHarnessRuntimeSecretGrant | undefined
}>

export type CloudRuntimeHarnessSecretGrantRef = Readonly<{
  kind: 'gemini_api_key'
  grantRef: string
  providerAccountRef: string
  runnerSessionId: string
  secretRef: string
}>

export type CloudRuntimeClaudeProviderAuthGrantRef = Readonly<{
  authGrantRef: string
  providerAccountRef: string
}>

export class ManagedAgentComputerHarnessConfigurationError extends S.TaggedErrorClass<ManagedAgentComputerHarnessConfigurationError>()(
  'ManagedAgentComputerHarnessConfigurationError',
  {
    harnessId: AgentComputerHarnessId,
    reasonRef: S.String,
  },
) {}

export const buildManagedAgentComputerHarnessBlocks = (
  input: Readonly<{
    selection: ManagedAgentComputerHarnessSelection
    baseUrl: string
    agentToken: string
    turnId: string
    runtimeSecretGrant?: CloudRuntimeHarnessSecretGrantRef | undefined
    claudeProviderAuthGrant?: CloudRuntimeClaudeProviderAuthGrantRef | undefined
  }>,
): Readonly<{
  harnessTurn?: CloudRuntimeHarnessTurnConfig | undefined
  claudeProviderAuth?: CloudRuntimeClaudeProviderAuthConfig | undefined
}> => {
  const selection = input.selection
  if (ManagedAgentComputerHarnessSelection.guards.codex(selection)) return {}
  if (ManagedAgentComputerHarnessSelection.guards.unavailable(selection)) {
    throw new ManagedAgentComputerHarnessConfigurationError({
      harnessId: selection.harnessId,
      reasonRef: selection.reasonRef,
    })
  }
  if (ManagedAgentComputerHarnessSelection.guards.claude(selection)) {
    const grant = input.claudeProviderAuthGrant
    if (grant === undefined) {
      throw new ManagedAgentComputerHarnessConfigurationError({
        harnessId: selection.harnessId,
        reasonRef: 'agent_computer_claude_grant_unavailable',
      })
    }
    return {
      claudeProviderAuth: {
        agentToken: input.agentToken,
        authGrantRef: grant.authGrantRef,
        baseUrl: input.baseUrl,
        providerAccountRef: grant.providerAccountRef,
      },
      harnessTurn: { harness: selection.harnessId },
    }
  }
  const grant = input.runtimeSecretGrant
  if (
    grant === undefined ||
    grant.kind !== 'gemini_api_key' ||
    grant.runnerSessionId !== input.turnId
  ) {
    throw new ManagedAgentComputerHarnessConfigurationError({
      harnessId: selection.harnessId,
      reasonRef: 'agent_computer_gemini_grant_unavailable',
    })
  }
  return {
    harnessTurn: {
      harness: selection.harnessId,
      model: selection.model,
      runtimeSecretGrant: {
        agentToken: input.agentToken,
        baseUrl: input.baseUrl,
        grantRef: grant.grantRef,
        kind: grant.kind,
        providerAccountRef: grant.providerAccountRef,
        runnerSessionId: grant.runnerSessionId,
        secretRef: grant.secretRef,
      },
    },
  }
}

/** A bounded direct-argv verifier for one managed coding turn. */
export type CloudRuntimeVerificationCommand = Readonly<{
  commandRef: string
  argv: ReadonlyArray<string>
  timeoutSeconds?: number | undefined
}>

/**
 * CX-6 (#8550) continuity strategy (a): fresh Codex provision, re-prime from
 * custody, then bounded Khala Sync history replay. Persisted CODEX_HOME
 * volumes are explicitly deferred and represented as `false`.
 */
export type CloudRuntimeCodexContinuityConfig = Readonly<{
  strategy: 'khala_sync_history_reprime'
  maxReplayMessages: number
  previousTurnCount?: number
  persistedCodexHome: false
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
  inference?: CloudRuntimeInferenceConfig
  codexTurn?: CloudRuntimeCodexTurnConfig
  harnessTurn?: CloudRuntimeHarnessTurnConfig
  writeback?: CloudRuntimeWritebackConfig
  providerAuth?: CloudRuntimeCodexProviderAuthConfig
  claudeProviderAuth?: CloudRuntimeClaudeProviderAuthConfig
  codexContinuity?: CloudRuntimeCodexContinuityConfig
  verificationCommand?: CloudRuntimeVerificationCommand
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
  inference?: CloudRuntimeInferenceConfig | undefined
  codexTurn?: CloudRuntimeCodexTurnConfig | undefined
  harnessTurn?: CloudRuntimeHarnessTurnConfig | undefined
  writeback?: CloudRuntimeWritebackConfig | undefined
  providerAuth?: CloudRuntimeCodexProviderAuthConfig | undefined
  claudeProviderAuth?: CloudRuntimeClaudeProviderAuthConfig | undefined
  codexContinuity?: CloudRuntimeCodexContinuityConfig | undefined
  verificationCommand?: CloudRuntimeVerificationCommand | undefined
}>

/** Build the full work-context object the turn-runner reads from `/tmp/wc.json`. */
export const buildCloudRuntimeWorkContext = (
  input: BuildWorkContextInput,
): CloudRuntimeWorkContext => ({
  branch: input.branch ?? CLOUD_RUNTIME_DEFAULT_BRANCH,
  commit: input.commit,
  objective:
    input.objective ??
    `agent-computer turn ${input.repo}@${input.commit.slice(0, 12)}`,
  repo: input.repo,
  threadRef: input.threadRef,
  turnId: input.turnId,
  workContextRef: input.workContextRef,
  ...(input.inference === undefined ? {} : { inference: input.inference }),
  ...(input.codexTurn === undefined ? {} : { codexTurn: input.codexTurn }),
  ...(input.harnessTurn === undefined
    ? {}
    : { harnessTurn: input.harnessTurn }),
  ...(input.writeback === undefined ? {} : { writeback: input.writeback }),
  ...(input.providerAuth === undefined
    ? {}
    : { providerAuth: input.providerAuth }),
  ...(input.claudeProviderAuth === undefined
    ? {}
    : { claudeProviderAuth: input.claudeProviderAuth }),
  ...(input.codexContinuity === undefined
    ? {}
    : { codexContinuity: input.codexContinuity }),
  ...(input.verificationCommand === undefined
    ? {}
    : { verificationCommand: input.verificationCommand }),
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
  return parseJsonUnknown(
    new TextDecoder().decode(bytes),
  ) as CloudRuntimeWorkContext
}

import { Schema as S } from 'effect'

/**
 * ATIF — Agent Trajectory Interchange Format (#6207, epic #6206).
 *
 * This module pins the PUBLIC-SAFE subset of ATIF-v1.7 that the OpenAgents trace
 * store ingests, stores, and serves so producers (qa-runner), the ingest API,
 * and the `/trace/{uuid}` page agree on one shape. A shared package (#6207) will
 * later canonicalize this; the contract below is intentionally a drop-in match
 * so the swap is trivial.
 *
 * Reference: `projects/repos/harbor/rfcs/0001-trajectory-format.md` (ATIF-v1.7);
 * spec `docs/traces/README.md`.
 *
 * We never store raw secrets, tokens, wallet/payment material, PII, local paths,
 * raw prompts/logs/provider payloads, or raw/split provider model ids. Only
 * `openagents/khala`-class public model ids are allowed. The tripwire below
 * (`atifTraceTripwire`) enforces that BEFORE persistence; the schema enforces
 * structural validity (sequential step_ids, source enums, observation refs).
 */

export const ATIF_PINNED_SCHEMA_VERSION = 'ATIF-v1.7' as const

export const TraceVisibility = S.Literals(['public', 'unlisted', 'owner_only'])
export type TraceVisibility = typeof TraceVisibility.Type

const StepSource = S.Literals(['user', 'agent', 'system'])

export class AtifToolCall extends S.Class<AtifToolCall>('AtifToolCall')({
  tool_call_id: S.String,
  function_name: S.String,
  arguments: S.Record(S.String, S.Unknown),
}) {}

export class AtifObservationResult extends S.Class<AtifObservationResult>(
  'AtifObservationResult',
)({
  source_call_id: S.String,
  content: S.String,
}) {}

export class AtifObservation extends S.Class<AtifObservation>('AtifObservation')(
  {
    results: S.Array(AtifObservationResult),
  },
) {}

export class AtifStepMetrics extends S.Class<AtifStepMetrics>('AtifStepMetrics')(
  {
    prompt_tokens: S.optionalKey(S.Number),
    completion_tokens: S.optionalKey(S.Number),
    cost_usd: S.optionalKey(S.Number),
  },
) {}

export class AtifStep extends S.Class<AtifStep>('AtifStep')({
  step_id: S.Int,
  timestamp: S.optionalKey(S.String),
  source: StepSource,
  message: S.String,
  reasoning_content: S.optionalKey(S.String),
  model_name: S.optionalKey(S.String),
  tool_calls: S.optionalKey(S.Array(AtifToolCall)),
  observation: S.optionalKey(AtifObservation),
  metrics: S.optionalKey(AtifStepMetrics),
}) {}

export class AtifAgent extends S.Class<AtifAgent>('AtifAgent')({
  name: S.String,
  version: S.String,
  model_name: S.optionalKey(S.String),
}) {}

export class AtifFinalMetrics extends S.Class<AtifFinalMetrics>(
  'AtifFinalMetrics',
)({
  total_prompt_tokens: S.optionalKey(S.Number),
  total_completion_tokens: S.optionalKey(S.Number),
  total_cost_usd: S.optionalKey(S.Number),
  total_steps: S.optionalKey(S.Number),
}) {}

/**
 * The pinned public-safe ATIF trajectory we ingest/store/serve. `schema_version`
 * is pinned to ATIF-v1.7 so a producer emitting a different revision is rejected
 * at the boundary rather than silently stored.
 */
export class AtifTrajectory extends S.Class<AtifTrajectory>('AtifTrajectory')({
  schema_version: S.Literal(ATIF_PINNED_SCHEMA_VERSION),
  trajectory_id: S.String,
  session_id: S.optionalKey(S.String),
  visibility: S.optionalKey(TraceVisibility),
  agent: AtifAgent,
  steps: S.Array(AtifStep),
  final_metrics: S.optionalKey(AtifFinalMetrics),
}) {}

/** Throwing decode. Wrap in `Effect.try`/`Effect.tryPromise` at the boundary. */
export const decodeAtifTrajectorySync = S.decodeUnknownSync(AtifTrajectory)
export const encodeAtifTrajectory = S.encodeSync(AtifTrajectory)

// ---------------------------------------------------------------------------
// Structural validator (mirrors harbor's validator rules)
// ---------------------------------------------------------------------------

export type AtifValidationIssue = Readonly<{
  code:
    | 'empty_steps'
    | 'step_id_not_sequential'
    | 'observation_without_tool_call'
    | 'agent_field_on_non_agent_step'
  message: string
}>

/**
 * Structural validation on top of schema decode:
 * - at least one step;
 * - `step_id` is sequential starting at 1;
 * - every observation `source_call_id` references a tool_call emitted in this
 *   trajectory;
 * - agent-only fields (`reasoning_content`, `tool_calls`) appear only on
 *   `source:"agent"` steps.
 */
export const validateAtifTrajectory = (
  trajectory: AtifTrajectory,
): ReadonlyArray<AtifValidationIssue> => {
  const issues: Array<AtifValidationIssue> = []

  if (trajectory.steps.length === 0) {
    issues.push({ code: 'empty_steps', message: 'Trajectory has no steps.' })
    return issues
  }

  const toolCallIds = new Set<string>()
  for (const step of trajectory.steps) {
    for (const call of step.tool_calls ?? []) {
      toolCallIds.add(call.tool_call_id)
    }
  }

  trajectory.steps.forEach((step, index) => {
    const expected = index + 1
    if (step.step_id !== expected) {
      issues.push({
        code: 'step_id_not_sequential',
        message: `step_id must be sequential from 1; expected ${expected} at position ${index}, got ${step.step_id}.`,
      })
    }

    if (
      step.source !== 'agent' &&
      (step.reasoning_content !== undefined ||
        (step.tool_calls !== undefined && step.tool_calls.length > 0))
    ) {
      issues.push({
        code: 'agent_field_on_non_agent_step',
        message: `Agent-only fields (reasoning_content/tool_calls) are not allowed on a "${step.source}" step (step_id ${step.step_id}).`,
      })
    }

    for (const result of step.observation?.results ?? []) {
      if (!toolCallIds.has(result.source_call_id)) {
        issues.push({
          code: 'observation_without_tool_call',
          message: `observation result source_call_id "${result.source_call_id}" does not reference any tool_call (step_id ${step.step_id}).`,
        })
      }
    }
  })

  return issues
}

// ---------------------------------------------------------------------------
// Public-safety tripwire
// ---------------------------------------------------------------------------

export type AtifTripwireFinding = Readonly<{
  code:
    | 'secret_material'
    | 'wallet_or_payment_material'
    | 'local_path'
    | 'pii_email'
    | 'raw_provider_model_id'
  message: string
}>

// Allowed public model id family. Only `openagents/...`-class ids (e.g.
// `openagents/khala`) may appear in agent/step `model_name`. Anything that looks
// like a raw/split provider model id (gpt-*, claude-*, gemini-*, llama-*,
// anthropic/*, openai/*, accounts/fireworks/*, etc.) is rejected.
const ALLOWED_MODEL_ID_PREFIX = 'openagents/'

const RAW_PROVIDER_MODEL_ID =
  /\b(?:gpt-[0-9o]|o[1-4]-(?:mini|preview)|claude-[0-9]|claude-(?:opus|sonnet|haiku)|gemini-[0-9]|gemini-(?:pro|flash)|llama-?[0-9]|mistral-|mixtral-|qwen[0-9.]|deepseek-|grok-[0-9])/i

const PROVIDER_NAMESPACED_MODEL_ID =
  /\b(?:anthropic|openai|google|vertex_ai|vertex|bedrock|fireworks|together|groq|azure|accounts\/fireworks|meta-llama)\//i

const SECRET_MATERIAL =
  /\b(?:sk-[a-z0-9]{8,}|sk_live_|sk_test_|rk_live_|xox[baprs]-|gh[pousr]_[A-Za-z0-9]{16,}|AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z_-]{20,}|eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{6,})\b/

const SECRET_KEYWORDS =
  /\b(?:api[_-]?key|secret[_-]?key|bearer\s+[A-Za-z0-9._-]{12,}|private[_-]?key|access[_-]?token|refresh[_-]?token|client[_-]?secret|password|passphrase|mnemonic|seed\s?phrase)\b/i

// Wallet / payment material: BOLT11/BOLT12, on-chain addresses, payment hashes,
// preimages, xpubs.
const WALLET_OR_PAYMENT_MATERIAL =
  /\b(?:lnbc[0-9][a-z0-9]{20,}|lntb[0-9][a-z0-9]{20,}|lno1[a-z0-9]{20,}|bc1[a-z0-9]{20,}|(?:xpub|ypub|zpub|tpub)[1-9A-HJ-NP-Za-km-z]{20,}|preimage|payment[_-]?hash)\b/i

const LOCAL_PATH = /(?:\/Users\/|\/home\/|[A-Za-z]:\\\\|file:\/\/)/

const PII_EMAIL = /[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/

/**
 * Scan the public-safe ATIF projection for material that must never be stored.
 * Returns a list of findings; an empty list means the projection is safe.
 *
 * Model ids are checked structurally on `agent.model_name` and every step
 * `model_name` (must be `openagents/...` when present), then the whole serialized
 * projection is scanned for raw/split provider ids, secrets, wallet/payment
 * material, local paths, and emails (PII).
 */
export const atifTraceTripwire = (
  trajectory: AtifTrajectory,
): ReadonlyArray<AtifTripwireFinding> => {
  const findings: Array<AtifTripwireFinding> = []

  const modelIds: Array<string> = []
  if (trajectory.agent.model_name !== undefined) {
    modelIds.push(trajectory.agent.model_name)
  }
  for (const step of trajectory.steps) {
    if (step.model_name !== undefined) {
      modelIds.push(step.model_name)
    }
  }
  for (const modelId of modelIds) {
    if (!modelId.startsWith(ALLOWED_MODEL_ID_PREFIX)) {
      findings.push({
        code: 'raw_provider_model_id',
        message: `model_name "${modelId}" is not an allowed public model id; only "${ALLOWED_MODEL_ID_PREFIX}*" ids may be stored.`,
      })
    }
  }

  const serialized = JSON.stringify(trajectory)

  if (
    RAW_PROVIDER_MODEL_ID.test(serialized) ||
    PROVIDER_NAMESPACED_MODEL_ID.test(serialized)
  ) {
    findings.push({
      code: 'raw_provider_model_id',
      message:
        'Trajectory contains a raw or split provider model id; only openagents/khala-class public ids are allowed.',
    })
  }

  if (SECRET_MATERIAL.test(serialized) || SECRET_KEYWORDS.test(serialized)) {
    findings.push({
      code: 'secret_material',
      message:
        'Trajectory contains secret/token material (API key, bearer token, or credential).',
    })
  }

  if (WALLET_OR_PAYMENT_MATERIAL.test(serialized)) {
    findings.push({
      code: 'wallet_or_payment_material',
      message:
        'Trajectory contains wallet or payment material (invoice, address, preimage, or payment hash).',
    })
  }

  if (LOCAL_PATH.test(serialized)) {
    findings.push({
      code: 'local_path',
      message: 'Trajectory contains a local filesystem path.',
    })
  }

  if (PII_EMAIL.test(serialized)) {
    findings.push({
      code: 'pii_email',
      message: 'Trajectory contains an email address (PII).',
    })
  }

  return findings
}

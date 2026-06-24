// ATIF — Agent Trajectory Interchange Format (the public-safe projection the
// `/trace/{uuid}` page renders).
//
// A *trace* is a record of one agent session: its goal, each step's narration +
// reasoning + tool call + observation, any screenshots/video, and the run
// metrics. See `docs/traces/README.md`.
//
// This is the PINNED render contract, agreed across the three sibling lanes:
//   - the qa-runner ATIF emitter (`apps/qa-runner/src/atif.ts`) that PRODUCES an
//     ATIF-v1.7 `Trajectory` from a real Khala run,
//   - the worker ingest/read API that STORES + serves it by uuid (in progress),
//   - this web page that RENDERS it.
//
// It mirrors the emitter's real output shape exactly (see the committed sample
// at `./sample`, lifted from `apps/qa-runner/samples/login-trace/trajectory.json`)
// so wiring the page to the worker read API is purely a data-source swap — the
// page accepts a decoded `Trajectory`, it does not care whether it came from the
// committed sample or a `GET /api/traces/{uuid}` fetch.
//
// PUBLIC-SAFE: never secrets, tokens, wallet material, PII, or raw/split provider
// model ids — only public `openagents/khala`-class ids. The ingest tripwire
// enforces this before storage; the render trusts the stored projection.

import { Schema as S } from 'effect'

// A JSON-safe value (tool_call arguments + the open `extra` bags are arbitrary
// public-safe JSON in ATIF-v1.7).
export type Json =
  | string
  | number
  | boolean
  | null
  | { readonly [k: string]: Json }
  | ReadonlyArray<Json>

// The open `extra` / tool-call `arguments` bags are arbitrary public-safe JSON.
// We decode them as `Record<string, unknown>` (matching `json-boundary.ts`) and
// narrow at the render edge via the typed `as*` helpers below — there is no
// value in a fully recursive decode schema here, and a recursive `S.suspend`
// schema would leak a non-`never` services channel into `decodeUnknownSync`.
const JsonRecord = S.Record(S.String, S.Unknown)

// A single tool call the agent emitted within a step. `arguments` is a
// public-safe JSON object (e.g. `{ action, target, narration }`).
export const ToolCall = S.Struct({
  tool_call_id: S.String,
  function_name: S.String,
  arguments: JsonRecord,
})
export type ToolCall = typeof ToolCall.Type

// One observation result, linked back to the tool call that produced it.
export const ObservationResult = S.Struct({
  source_call_id: S.optionalKey(S.String),
  content: S.optionalKey(S.String),
})
export type ObservationResult = typeof ObservationResult.Type

export const Observation = S.Struct({
  results: S.Array(ObservationResult),
})
export type Observation = typeof Observation.Type

// Per-step metrics. All optional: a `user` goal step has none.
export const Metrics = S.Struct({
  prompt_tokens: S.optionalKey(S.Number),
  completion_tokens: S.optionalKey(S.Number),
  cost_usd: S.optionalKey(S.Number),
})
export type Metrics = typeof Metrics.Type

export const StepSource = S.Literals(['user', 'agent', 'system'])
export type StepSource = typeof StepSource.Type

export const Step = S.Struct({
  step_id: S.Number,
  timestamp: S.optionalKey(S.String),
  source: StepSource,
  model_name: S.optionalKey(S.String),
  // Human-readable narration (the agent's "what I did").
  message: S.String,
  // The model's stated decision / chain-of-thought, rendered collapsed.
  reasoning_content: S.optionalKey(S.String),
  tool_calls: S.optionalKey(S.Array(ToolCall)),
  observation: S.optionalKey(Observation),
  metrics: S.optionalKey(Metrics),
})
export type Step = typeof Step.Type

export const Agent = S.Struct({
  name: S.String,
  version: S.String,
  model_name: S.optionalKey(S.String),
  extra: S.optionalKey(JsonRecord),
})
export type Agent = typeof Agent.Type

export const FinalMetrics = S.Struct({
  total_prompt_tokens: S.optionalKey(S.Number),
  total_completion_tokens: S.optionalKey(S.Number),
  total_cached_tokens: S.optionalKey(S.Number),
  total_cost_usd: S.optionalKey(S.Number),
  total_steps: S.optionalKey(S.Number),
  extra: S.optionalKey(JsonRecord),
})
export type FinalMetrics = typeof FinalMetrics.Type

export const Trajectory = S.Struct({
  schema_version: S.String,
  session_id: S.optionalKey(S.String),
  trajectory_id: S.optionalKey(S.String),
  agent: Agent,
  notes: S.optionalKey(S.String),
  steps: S.Array(Step),
  final_metrics: S.optionalKey(FinalMetrics),
  extra: S.optionalKey(JsonRecord),
})
export type Trajectory = typeof Trajectory.Type

export const decodeTrajectory = S.decodeUnknownSync(Trajectory)

// ---------------------------------------------------------------------------
// Render-facing derived helpers (presentation only, no I/O). These read the
// trajectory's open `extra`/`final_metrics` bags and goal step into the typed
// shape the page header + timeline + metrics need.
// ---------------------------------------------------------------------------

export type VerdictTone = 'positive' | 'negative' | 'warning' | 'neutral'

// The QA-side verdict vocabulary the emitter writes into `final_metrics.extra`.
export type TraceVerdict = 'PASS' | 'REFUTED' | 'INCONCLUSIVE' | 'IN_PROGRESS'

const asString = (value: unknown): string | undefined =>
  typeof value === 'string' ? value : undefined

const asNumber = (value: unknown): number | undefined =>
  typeof value === 'number' ? value : undefined

const isRecord = (value: unknown): value is { [k: string]: unknown } =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

export const traceVerdict = (trajectory: Trajectory): TraceVerdict => {
  const raw = asString(trajectory.final_metrics?.extra?.verdict)?.toUpperCase()
  if (raw === 'PASS' || raw === 'PASSED') return 'PASS'
  if (raw === 'REFUTED' || raw === 'FAILED') return 'REFUTED'
  if (raw === 'INCONCLUSIVE') return 'INCONCLUSIVE'
  return 'IN_PROGRESS'
}

export const verdictLabel = (verdict: TraceVerdict): string => {
  switch (verdict) {
    case 'PASS':
      return 'Verified'
    case 'REFUTED':
      return 'Refuted'
    case 'INCONCLUSIVE':
      return 'Inconclusive'
    case 'IN_PROGRESS':
      return 'In progress'
  }
}

export const verdictTone = (verdict: TraceVerdict): VerdictTone => {
  switch (verdict) {
    case 'PASS':
      return 'positive'
    case 'REFUTED':
      return 'negative'
    case 'INCONCLUSIVE':
      return 'warning'
    case 'IN_PROGRESS':
      return 'neutral'
  }
}

export const traceDurationMs = (trajectory: Trajectory): number | undefined =>
  asNumber(trajectory.final_metrics?.extra?.duration_ms)

export const traceTarget = (
  trajectory: Trajectory,
): { name: string; baseUrl?: string } | undefined => {
  const target = trajectory.extra?.target
  if (!isRecord(target)) return undefined
  const name = asString(target.name)
  if (name === undefined) return undefined
  const baseUrl = asString(target.baseUrl)
  return baseUrl === undefined ? { name } : { name, baseUrl }
}

// The committed sample video lives in the web app's public assets. A real
// ingested trajectory references its video via `extra.artifacts.video` (an R2
// key); the read-API wiring resolves that to a playable URL. For the sample we
// resolve to the committed public-safe recording.
export const traceVideoSrc = (trajectory: Trajectory): string | undefined => {
  const artifacts = trajectory.extra?.artifacts
  if (!isRecord(artifacts)) return undefined
  return asString(artifacts.video) === undefined ? undefined : SAMPLE_VIDEO_SRC
}

export const SAMPLE_VIDEO_SRC = '/pro-assets/sample-session.webm'

// The first `source: "user"` step is the goal; fall back to none.
export const traceGoal = (trajectory: Trajectory): string | undefined =>
  trajectory.steps.find(step => step.source === 'user')?.message

// Steps rendered as agent timeline nodes (everything after the user goal).
export const agentSteps = (trajectory: Trajectory): ReadonlyArray<Step> =>
  trajectory.steps.filter(step => step.source !== 'user')

// ---------------------------------------------------------------------------
// Number formatting (compact, honest).
// ---------------------------------------------------------------------------

export const formatDuration = (ms: number): string => {
  if (ms < 1000) return `${Math.round(ms)}ms`
  const totalSeconds = ms / 1000
  if (totalSeconds < 60) {
    const rounded = Math.round(totalSeconds * 10) / 10
    return Number.isInteger(rounded) ? `${rounded}s` : `${rounded.toFixed(1)}s`
  }
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = Math.round(totalSeconds % 60)
  return `${minutes}m ${seconds}s`
}

// Own-infra sessions cost $0; paid sessions are typically fractions of a cent,
// so show enough precision to be honest rather than rounding to `$0.00`.
export const formatCost = (usd: number): string => {
  if (usd === 0) return '$0.00'
  if (usd < 0.01) return `$${usd.toFixed(4)}`
  return `$${usd.toFixed(2)}`
}

export const formatTokens = (count: number): string =>
  count.toLocaleString('en-US')

// A stable, short display id from a uuid (first segment), for chrome.
export const shortId = (uuid: string): string => uuid.split('-')[0] ?? uuid

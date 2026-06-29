// ATIF-v1.7 validator (epic #6174, consolidated in #6207): an Effect-Schema
// decoder for the ATIF `Trajectory` shape plus the structural invariants the
// schema can't express:
//   - sequential `step_id` starting at 1 (RFC §StepObject: "starting from 1")
//   - `source` is one of system|user|agent (RFC §StepObject)
//   - every observation `source_call_id` references a `tool_call_id` in the SAME
//     step's `tool_calls` (RFC §ObservationResultSchema correlation)
//   - timestamps are ISO 8601 (RFC §StepObject)
//   - schema_version is the ATIF-v1.7 literal
//   - agent.name + agent.version are present (RFC §AgentSchema required)
//
// This is the tripwire the qa-runner uses before committing a trajectory: a
// trajectory that does not validate is NEVER written/committed. It is the
// producer-facing (permissive) validator; the stricter ingest/store schema +
// public-safety tripwire live in `@openagentsinc/atif/trace`.
//
// Effect Schema conventions follow the codebase (`result.ts`): positional
// `S.Record(key, value)`, array-form `S.Union([...])`, `S.Literals`, and
// `decodeUnknownSync` (this Effect v4 beta has no `decodeUnknownEither`).

import { Schema as S } from "effect";

// ATIF `arguments` / `extra` are open JSON objects; the validator's job is the
// ATIF STRUCTURAL invariants (step ids, source enum, tool/observation
// correlation, ISO timestamps), not deep JSON typing. `S.Unknown` keeps the
// open objects permissive without a (fragile) recursive JSON schema.
const JsonValue = S.Unknown;
const JsonObject = S.Record(S.String, JsonValue);

const AtifToolCallSchema = S.Struct({
  tool_call_id: S.String,
  function_name: S.String,
  arguments: JsonObject,
});

const AtifObservationResultSchema = S.Struct({
  source_call_id: S.optional(S.String),
  content: S.optional(S.Union([S.String, S.Array(JsonValue)])),
});

const AtifObservationSchema = S.Struct({
  results: S.Array(AtifObservationResultSchema),
});

const AtifMetricsSchema = S.Struct({
  prompt_tokens: S.optional(S.Number),
  completion_tokens: S.optional(S.Number),
  cached_tokens: S.optional(S.Number),
  cost_usd: S.optional(S.Number),
  extra: S.optional(JsonObject),
});

const AtifSource = S.Literals(["system", "user", "agent"]);

const AtifStepSchema = S.Struct({
  step_id: S.Number,
  timestamp: S.optional(S.String),
  source: AtifSource,
  model_name: S.optional(S.String),
  reasoning_effort: S.optional(S.Union([S.String, S.Number])),
  message: S.Union([S.String, S.Array(JsonValue)]),
  reasoning_content: S.optional(S.String),
  tool_calls: S.optional(S.Array(AtifToolCallSchema)),
  observation: S.optional(AtifObservationSchema),
  metrics: S.optional(AtifMetricsSchema),
  llm_call_count: S.optional(S.Number),
});

const AtifAgentSchema = S.Struct({
  name: S.String,
  version: S.String,
  model_name: S.optional(S.String),
  tool_definitions: S.optional(S.Array(JsonValue)),
  extra: S.optional(JsonObject),
});

const AtifFinalMetricsSchema = S.Struct({
  total_prompt_tokens: S.optional(S.Number),
  total_completion_tokens: S.optional(S.Number),
  total_cached_tokens: S.optional(S.Number),
  total_cost_usd: S.optional(S.Number),
  total_steps: S.optional(S.Number),
  extra: S.optional(JsonObject),
});

export const AtifTrajectorySchema = S.Struct({
  schema_version: S.Literal("ATIF-v1.7"),
  session_id: S.optional(S.String),
  trajectory_id: S.optional(S.String),
  agent: AtifAgentSchema,
  notes: S.optional(S.String),
  steps: S.Array(AtifStepSchema),
  final_metrics: S.optional(AtifFinalMetricsSchema),
  extra: S.optional(JsonObject),
});

const decodeAtif = S.decodeUnknownSync(AtifTrajectorySchema);

export class AtifValidationError extends Error {
  constructor(
    message: string,
    readonly errors: ReadonlyArray<string>,
  ) {
    super(`atif_validation_error: ${message}`);
    this.name = "AtifValidationError";
  }
}

// ISO 8601 (a pragmatic check: parses to a valid Date AND looks ISO-shaped).
const ISO_8601 = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$/;

function isIso8601(value: string): boolean {
  return ISO_8601.test(value) && !Number.isNaN(Date.parse(value));
}

// The decoded ATIF step shape (the structural fields the invariants inspect).
interface DecodedStep {
  readonly step_id: number;
  readonly source: "system" | "user" | "agent";
  readonly timestamp?: string;
  readonly reasoning_content?: string;
  readonly tool_calls?: ReadonlyArray<{ readonly tool_call_id: string; readonly function_name: string }>;
  readonly observation?: { readonly results: ReadonlyArray<{ readonly source_call_id?: string }> };
  readonly metrics?: unknown;
}
interface DecodedTrajectory {
  readonly agent: { readonly name: string; readonly version: string };
  readonly steps: ReadonlyArray<DecodedStep>;
}

export interface AtifValidationResult {
  readonly valid: boolean;
  readonly errors: ReadonlyArray<string>;
}

/**
 * Validate an unknown value as an ATIF-v1.7 trajectory. Collects ALL structural
 * errors (not just the first) so a producer sees every problem at once. Returns
 * `{ valid, errors }`; use `assertValidAtif` to throw.
 */
export function validateAtif(value: unknown): AtifValidationResult {
  // 1. schema decode (shape, types, source enum, schema_version literal).
  let t: DecodedTrajectory;
  try {
    t = decodeAtif(value) as unknown as DecodedTrajectory;
  } catch (error) {
    return { valid: false, errors: [formatSchemaError(error)] };
  }

  const errors: string[] = [];

  // 2. agent required fields (schema enforces presence; guard emptiness).
  if (t.agent.name.length === 0) errors.push("agent.name must be non-empty");
  if (t.agent.version.length === 0) errors.push("agent.version must be non-empty");

  // 3. steps must be non-empty.
  if (t.steps.length === 0) errors.push("steps must be non-empty");

  // 4. sequential step_id starting at 1.
  t.steps.forEach((step, i) => {
    const expected = i + 1;
    if (step.step_id !== expected) {
      errors.push(`steps[${i}].step_id = ${step.step_id}, expected ${expected} (sequential from 1)`);
    }
  });

  // 5. agent-only fields must not appear on non-agent steps (RFC: reasoning_*/
  //    tool_calls/metrics are "only applicable when source is agent").
  t.steps.forEach((step, i) => {
    if (step.source !== "agent") {
      if (step.tool_calls && step.tool_calls.length > 0)
        errors.push(`steps[${i}] source="${step.source}" must not carry tool_calls`);
      if (step.reasoning_content !== undefined)
        errors.push(`steps[${i}] source="${step.source}" must not carry reasoning_content`);
      if (step.metrics !== undefined)
        errors.push(`steps[${i}] source="${step.source}" must not carry metrics`);
    }
  });

  // 6. observation source_call_id must reference a tool_call_id in the SAME step.
  t.steps.forEach((step, i) => {
    if (!step.observation) return;
    const callIds = new Set((step.tool_calls ?? []).map((c) => c.tool_call_id));
    step.observation.results.forEach((r, j) => {
      if (r.source_call_id !== undefined && !callIds.has(r.source_call_id)) {
        errors.push(
          `steps[${i}].observation.results[${j}].source_call_id "${r.source_call_id}" ` +
            `does not reference a tool_call_id in the same step`,
        );
      }
    });
  });

  // 7. tool_call_id uniqueness within a step.
  t.steps.forEach((step, i) => {
    const seen = new Set<string>();
    (step.tool_calls ?? []).forEach((c, j) => {
      if (seen.has(c.tool_call_id))
        errors.push(`steps[${i}].tool_calls[${j}].tool_call_id "${c.tool_call_id}" is duplicated in the step`);
      seen.add(c.tool_call_id);
    });
  });

  // 8. ISO 8601 timestamps where present.
  t.steps.forEach((step, i) => {
    if (step.timestamp !== undefined && !isIso8601(step.timestamp)) {
      errors.push(`steps[${i}].timestamp "${step.timestamp}" is not ISO 8601`);
    }
  });

  return { valid: errors.length === 0, errors };
}

/** Throw `AtifValidationError` if the value is not a valid ATIF-v1.7 trajectory. */
export function assertValidAtif(value: unknown): void {
  const { valid, errors } = validateAtif(value);
  if (!valid) {
    throw new AtifValidationError(`${errors.length} error(s): ${errors.join("; ")}`, errors);
  }
}

function formatSchemaError(error: unknown): string {
  if (error instanceof Error) return `schema: ${error.message.split("\n").slice(0, 4).join(" ")}`;
  return `schema: ${String(error)}`;
}

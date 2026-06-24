// ATIF emitter TYPES (epic #6174, consolidated in #6207): the canonical
// dependency-free TypeScript shape a producer emits for an ATIF-v1.7 Agent
// Trajectory Interchange Format `Trajectory`.
//
// Source of truth: harbor `rfcs/0001-trajectory-format.md` (ATIF-v1.7) and the
// golden examples in `tests/golden/terminus_2/*.trajectory.json`.
//
// These are the producer-facing structural types (string `message`, optional
// `notes`/`extra`, open `Json` argument values). The qa-runner mapper
// (`mapKhalaRunToAtif`) and the Claude Code / Codex converters build values of
// these types; they re-export the type names from here so downstream imports
// stay stable. The Effect-Schema VALIDATOR for these values lives in
// `@openagentsinc/atif/validate`; the stricter ingest/store SCHEMA + tripwire
// live in `@openagentsinc/atif/trace`.

export const ATIF_SCHEMA_VERSION = "ATIF-v1.7";

/** ATIF tool_call argument values — JSON-safe scalars/objects, never secrets. */
export type Json = string | number | boolean | null | { [k: string]: Json } | Json[];

export interface AtifToolCall {
  readonly tool_call_id: string;
  readonly function_name: string;
  readonly arguments: Record<string, Json>;
}

export interface AtifObservationResult {
  readonly source_call_id?: string;
  readonly content?: string;
}

export interface AtifObservation {
  readonly results: ReadonlyArray<AtifObservationResult>;
}

export interface AtifMetrics {
  readonly prompt_tokens?: number;
  readonly completion_tokens?: number;
  readonly cost_usd?: number;
}

export interface AtifStep {
  readonly step_id: number;
  readonly timestamp?: string;
  readonly source: "user" | "agent" | "system";
  readonly model_name?: string;
  readonly message: string;
  readonly reasoning_content?: string;
  readonly tool_calls?: ReadonlyArray<AtifToolCall>;
  readonly observation?: AtifObservation;
  readonly metrics?: AtifMetrics;
}

export interface AtifAgent {
  readonly name: string;
  readonly version: string;
  readonly model_name?: string;
  readonly extra?: Record<string, Json>;
}

export interface AtifFinalMetrics {
  readonly total_prompt_tokens?: number;
  readonly total_completion_tokens?: number;
  readonly total_cached_tokens?: number;
  readonly total_cost_usd?: number;
  readonly total_steps?: number;
  readonly extra?: Record<string, Json>;
}

export interface AtifTrajectory {
  readonly schema_version: string;
  readonly session_id?: string;
  readonly trajectory_id?: string;
  readonly agent: AtifAgent;
  readonly notes?: string;
  readonly steps: ReadonlyArray<AtifStep>;
  readonly final_metrics?: AtifFinalMetrics;
  readonly extra?: Record<string, Json>;
}

/** The QA-side verdict surfaced into the trajectory header (own vocabulary). */
export type AtifVerdict = "PASS" | "REFUTED" | "INCONCLUSIVE";

export const serializeTrajectory = (t: AtifTrajectory): string => `${JSON.stringify(t, null, 2)}\n`;

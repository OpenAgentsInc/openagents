// Khala provider/engine benchmark MATRIX (book P1-5 / #6088).
//
// The book's Ch.4 §4.5 lesson, turned into a typed contract: "faster" is
// meaningless until you say faster at WHAT, on WHICH lane, under WHICH traffic
// shape, judged on WHICH outcome. So a benchmark is not an ad-hoc script with
// hard-coded numbers — it is a MATRIX that varies the dimensions the book calls
// out (lane, engine, workload, sequence shape, streaming-vs-batch,
// temperature/reasoning) and is scored on VERIFICATION OUTCOME, not just raw
// token speed.
//
// This module owns ONLY the typed configuration + the deterministic expansion of
// a matrix config into the concrete (lane × engine × workload × shape × …) cells
// a runner executes. It is PURE and framework-agnostic: no Worker, no network,
// no Effect runtime, no clock. The runner (`runner.ts`) executes the cells; the
// report (`report.ts`) aggregates the results. Keeping the matrix declarative
// means the benchmark is auditable and reproducible: the same config always
// expands to the same ordered cell list.
//
// HONESTY: a lane/engine that does NOT yet exist as a real Khala serving path is
// still a first-class matrix axis VALUE, but it is marked `availability:
// 'not_yet_available'`. The benchmark never pretends an unbuilt lane was
// measured; it expands the cell, records WHY it is unavailable, and the report
// labels it. This mirrors the telemetry schema's `not_measured` discipline.
import { Schema as S } from 'effect'

import { KhalaRequestClass } from '../khala-telemetry'

// ---------------------------------------------------------------------------
// Axis values (the book's matrix dimensions, as closed literal unions).
// ---------------------------------------------------------------------------

// The lane under test. Phase 0 used provider-adapter supply lanes; Phase 1 also
// names client-surface model endpoints so the Gym can compare whole coding-agent
// experiences through OpenCode (Khala vs BigPickle vs other free/paid fields).
// The benchmark can name a lane that does not have a real spend seam yet; the
// `LANE_AVAILABILITY` table below records whether it is real, fixture-only, or
// future-only.
export const BenchmarkLane = S.Literals([
  'khala',
  'bigpickle',
  'gemini-free',
  'openai-gpt',
  'claude',
  'vertex-anthropic',
  'vertex-gemini',
  'fireworks',
  'partner-passthrough',
  'gpt-oss-20b',
  'gpt-oss-120b',
  'glm-52',
  'pylon-whole-small',
  'psionic-shard-wan',
])
export type BenchmarkLane = typeof BenchmarkLane.Type

// The serving ENGINE behind a lane (book P1-5 + notes §6). A managed provider is
// `provider-native`; self-hosted lanes pick a concrete open engine. Recorded so
// a future "Fireworks-native vs our-own-vLLM" comparison is a matrix axis, not a
// rewrite.
export const BenchmarkEngine = S.Literals([
  'provider-native',
  'vllm',
  'sglang',
  'tensorrt-llm',
])
export type BenchmarkEngine = typeof BenchmarkEngine.Type

// The WORKLOAD shape — the kind of Khala request. Each one optimizes different
// metrics (the request-class lesson): chat optimizes TTFT/ITL; artifact gen
// optimizes accepted-outcome; a verifier run optimizes verified-rate; a
// long-context codebase question stresses input length + prefix cache.
export const BenchmarkWorkload = S.Literals([
  'chat',
  'opencode-coding-task',
  'khala-code-artifact-gen',
  'verifier-run',
  'long-context-codebase-question',
])
export type BenchmarkWorkload = typeof BenchmarkWorkload.Type

// Streaming vs batch transport (book Ch.7 / the P0-3 split). Determines which
// metrics are even MEASURABLE (TTFT exists only on the streaming path; batch
// optimizes throughput/cost over a detached job).
export const BenchmarkTransport = S.Literals(['streaming', 'batch'])
export type BenchmarkTransport = typeof BenchmarkTransport.Type

// The expected verification OUTCOME the cell is judged on (book P1-5: "verification
// outcome, not just raw token speed"). This is the EXPECTED class for the fixture
// scenario — the runner records the ACTUAL executed verdict and the report compares.
export const BenchmarkVerificationExpectation = S.Literals([
  'none',
  'seeded',
  'test_passed',
  'exact_trace_replay',
])
export type BenchmarkVerificationExpectation =
  typeof BenchmarkVerificationExpectation.Type

// ---------------------------------------------------------------------------
// Lane availability (HONEST: which lanes are real Khala serving paths today).
// ---------------------------------------------------------------------------

// Whether a lane is a real serving path the benchmark can actually hit with a
// real-lane adapter today, vs a named-but-unbuilt future lane.
export const LaneAvailability = S.Literals([
  'available',
  'fixture_only',
  'not_yet_available',
])
export type LaneAvailability = typeof LaneAvailability.Type

// The availability table. `available` lanes have a real provider adapter or
// client-surface endpoint behind the owner-gated real-lane seam. `fixture_only`
// lanes can be executed by the deterministic fixture seam, but are skipped by
// owner-armed real sweeps until a real executor is wired. `not_yet_available`
// lanes stay in the comparison shape but are never measured. This is the single
// source of truth the runner + report read; it never invents an "available"
// claim for an unbuilt lane.
export const LANE_AVAILABILITY: Readonly<
  Record<BenchmarkLane, LaneAvailability>
> = {
  khala: 'available',
  bigpickle: 'fixture_only',
  'gemini-free': 'fixture_only',
  'openai-gpt': 'fixture_only',
  claude: 'fixture_only',
  'vertex-anthropic': 'available',
  'vertex-gemini': 'available',
  fireworks: 'available',
  'partner-passthrough': 'available',
  'gpt-oss-20b': 'available',
  'gpt-oss-120b': 'available',
  'glm-52': 'available',
  // Pylon whole-small serving and Psionic shard-WAN are named in the notes as
  // FUTURE lanes (§6, decentralized-serving-shard-wan). They are matrix axes so
  // the decision suite is shaped for them, but they are not real serving paths
  // yet — honestly labeled, never measured.
  'pylon-whole-small': 'not_yet_available',
  'psionic-shard-wan': 'not_yet_available',
}

export const laneAvailability = (lane: BenchmarkLane): LaneAvailability =>
  LANE_AVAILABILITY[lane]

// ---------------------------------------------------------------------------
// Sequence shape (book §4.5: ISL / OSL / cacheable prefix — must match prod).
// ---------------------------------------------------------------------------

export const BenchmarkShapeSource = S.Literals([
  'gateway_telemetry',
  'receipt_projection',
  'operator_export',
  'synthetic_fixture',
])
export type BenchmarkShapeSource = typeof BenchmarkShapeSource.Type

// A sequence shape: input length (ISL), output length (OSL), and the cacheable
// prefix length within the input. The book is emphatic that these must match the
// production workload — a benchmark over the wrong shapes measures nothing useful
// (§4.5: "If you're maximizing benchmark performance against bad inputs,
// performance in production won't match expectations"). The report labels whether
// a shape was sourced from real traffic or is synthetic.
export const SequenceShape = S.Struct({
  // A short stable id for the shape (e.g. "short-chat", "long-codebase-32k").
  id: S.String,
  // Input sequence length in tokens (the prompt).
  inputTokens: S.Number,
  // Output sequence length in tokens (the generation).
  outputTokens: S.Number,
  // Cacheable prefix length in tokens (the shared stable-prefix portion of the
  // input that a prompt cache can reuse — book P0-2). 0 when no shared prefix.
  cacheablePrefixTokens: S.Number,
  // Concurrency: number of simultaneous in-flight requests this shape models
  // (book §4.5 "volume and pattern of traffic"). 1 = a single serial request.
  concurrency: S.Number,
  // Whether this shape's lengths/contents came from REAL observed Khala traffic
  // (`realistic`) or were invented (`synthetic`). The report surfaces this
  // prominently: synthetic-only numbers are illustrative, not decision-grade.
  provenance: S.Literals(['realistic', 'synthetic']),
  // The Khala request class this shape came from, when known. This reuses the
  // canonical telemetry vocabulary so the benchmark never invents a parallel
  // traffic classifier. Optional on legacy/synthetic shapes.
  requestClass: S.optional(KhalaRequestClass),
  // Public-safe evidence that the shape was sourced from observed Khala traffic.
  // Required by the real-sweep preflight for decision-grade realistic runs; kept
  // optional here so synthetic fixtures can stay spend-free and honest.
  observedTrafficEvidenceRef: S.optional(S.String),
  observedRequestCount: S.optional(S.Number),
  source: S.optional(BenchmarkShapeSource),
})
export type SequenceShape = typeof SequenceShape.Type

// ---------------------------------------------------------------------------
// Sampling settings (book §4.5: temperature/reasoning at production values).
// ---------------------------------------------------------------------------

export const SamplingSettings = S.Struct({
  // Sampling temperature (production value, not a benchmark-flattering 0).
  temperature: S.Number,
  // Reasoning/thinking effort where the model exposes it (off | low | medium |
  // high). `off` for a non-reasoning lane. Reasoning inflates billed tokens
  // (the `unaccountedTokens` dimension) and changes latency, so it is an axis.
  reasoningEffort: S.Literals(['off', 'low', 'medium', 'high']),
})
export type SamplingSettings = typeof SamplingSettings.Type

// ---------------------------------------------------------------------------
// The matrix config (declarative cross-product spec) + a single expanded cell.
// ---------------------------------------------------------------------------

export const BenchmarkTargetRouteRole = S.Literals([
  'first',
  'fallback',
  'reserved',
  'comparison',
])
export type BenchmarkTargetRouteRole = typeof BenchmarkTargetRouteRole.Type

export const BenchmarkTargetCapacityClass = S.Literals([
  'provider_managed',
  'owned_singleflight',
  'owned_pool',
  'fixture',
])
export type BenchmarkTargetCapacityClass =
  typeof BenchmarkTargetCapacityClass.Type

// Public-safe metadata for one benchmark candidate. This is where "GLM pool
// primary" and "Fireworks DeepSeek V4 Flash" become distinct candidates without
// leaking private endpoints, secrets, raw prices, or owner-only topology.
export const BenchmarkTargetProfile = S.Struct({
  profileRef: S.String,
  modelRef: S.String,
  routeRole: BenchmarkTargetRouteRole,
  capacityClass: BenchmarkTargetCapacityClass,
  replicaPoolRef: S.optional(S.String),
  replicaRef: S.optional(S.String),
  replicaCount: S.optional(S.Number),
  costProfileRef: S.optional(S.String),
  evidenceRefs: S.optional(S.Array(S.String)),
})
export type BenchmarkTargetProfile = typeof BenchmarkTargetProfile.Type

// One (lane, engine) supply target. Engines are paired with lanes explicitly
// (rather than a blind cross-product) because not every engine runs on every
// lane — a managed provider is always `provider-native`; only self-hosted lanes
// pick vLLM/SGLang/TensorRT-LLM. Pairing avoids fabricating impossible cells.
export const BenchmarkTarget = S.Struct({
  lane: BenchmarkLane,
  engine: BenchmarkEngine,
  profile: S.optional(BenchmarkTargetProfile),
})
export type BenchmarkTarget = typeof BenchmarkTarget.Type

// The declarative matrix config. The runner expands the cross-product of
// targets × workloads × shapes × transports × sampling into ordered cells. A
// benchmark RUN is "execute every cell of one config against one seam".
export const BenchmarkMatrixConfig = S.Struct({
  // Stable id for this matrix config (e.g. "fireworks-vs-vertex-chat-code-v1").
  id: S.String,
  // Human-readable purpose, public-safe (no accounts/prompts).
  description: S.String,
  targets: S.Array(BenchmarkTarget),
  workloads: S.Array(BenchmarkWorkload),
  shapes: S.Array(SequenceShape),
  transports: S.Array(BenchmarkTransport),
  sampling: S.Array(SamplingSettings),
  // How many repeated samples per cell (book §4.5.2: "send enough traffic ...
  // run a benchmark multiple times and average"). >= 1.
  samplesPerCell: S.Number,
})
export type BenchmarkMatrixConfig = typeof BenchmarkMatrixConfig.Type

// A single fully-resolved benchmark cell: one point in the cross-product. The
// `verificationExpectation` is DERIVED from the workload (chat expects `none`;
// artifact-gen/verifier-run expect an executed `test_passed`), so the matrix is
// scored on outcome without the config author having to repeat it per cell.
export const BenchmarkCell = S.Struct({
  // Deterministic stable id: encodes every axis value so the same cell always
  // gets the same id (auditable, reproducible).
  cellId: S.String,
  lane: BenchmarkLane,
  engine: BenchmarkEngine,
  targetProfile: S.optional(BenchmarkTargetProfile),
  candidateRef: S.String,
  laneAvailability: LaneAvailability,
  workload: BenchmarkWorkload,
  shape: SequenceShape,
  transport: BenchmarkTransport,
  sampling: SamplingSettings,
  verificationExpectation: BenchmarkVerificationExpectation,
  samplesPerCell: S.Number,
})
export type BenchmarkCell = typeof BenchmarkCell.Type

// Derive the expected verification class from the workload. A chat turn is not
// verified work; an artifact-gen / verifier-run cell expects an EXECUTED
// `test_passed`; a long-context codebase question is `seeded` (a reference
// answer exists but it is not an executed acceptance suite).
export const verificationExpectationForWorkload = (
  workload: BenchmarkWorkload,
): BenchmarkVerificationExpectation => {
  switch (workload) {
    case 'chat':
      return 'none'
    case 'long-context-codebase-question':
      return 'seeded'
    case 'opencode-coding-task':
    case 'khala-code-artifact-gen':
    case 'verifier-run':
      return 'test_passed'
  }
}

// Build the deterministic stable cell id. Pure string assembly over the axis
// values — no randomness, no clock — so the same axes always yield the same id.
export const buildCellId = (input: {
  lane: BenchmarkLane
  engine: BenchmarkEngine
  profileRef?: string | undefined
  workload: BenchmarkWorkload
  shapeId: string
  transport: BenchmarkTransport
  sampling: SamplingSettings
}): string =>
  [
    input.lane,
    input.engine,
    ...(input.profileRef === undefined || input.profileRef.trim() === ''
      ? []
      : [`profile:${input.profileRef}`]),
    input.workload,
    input.shapeId,
    input.transport,
    `t${input.sampling.temperature}`,
    `r${input.sampling.reasoningEffort}`,
  ].join('|')

// Expand a matrix config into its ordered list of concrete cells. PURE and
// DETERMINISTIC: the cross-product is generated in a fixed nested order
// (targets → workloads → shapes → transports → sampling) so the expanded list is
// byte-stable for a given config. This is the contract the runner + report rely
// on for reproducibility.
export const expandMatrix = (
  config: BenchmarkMatrixConfig,
): ReadonlyArray<BenchmarkCell> => {
  const cells: Array<BenchmarkCell> = []
  for (const target of config.targets) {
    for (const workload of config.workloads) {
      for (const shape of config.shapes) {
        for (const transport of config.transports) {
          for (const sampling of config.sampling) {
            cells.push({
              cellId: buildCellId({
                lane: target.lane,
                engine: target.engine,
                profileRef: target.profile?.profileRef,
                workload,
                shapeId: shape.id,
                transport,
                sampling,
              }),
              lane: target.lane,
              engine: target.engine,
              ...(target.profile === undefined
                ? {}
                : { targetProfile: target.profile }),
              candidateRef:
                target.profile?.profileRef ?? `${target.lane}/${target.engine}`,
              laneAvailability: laneAvailability(target.lane),
              workload,
              shape,
              transport,
              sampling,
              verificationExpectation:
                verificationExpectationForWorkload(workload),
              samplesPerCell: config.samplesPerCell,
            })
          }
        }
      }
    }
  }
  return cells
}

// The expected cell count for a config (the cross-product cardinality). Used by
// tests to assert the matrix expanded to exactly the right number of cells, and
// by the report header to disclose coverage.
export const expectedCellCount = (config: BenchmarkMatrixConfig): number =>
  config.targets.length *
  config.workloads.length *
  config.shapes.length *
  config.transports.length *
  config.sampling.length

// ---------------------------------------------------------------------------
// Decoders + a documented minimum decision suite (notes Q5).
// ---------------------------------------------------------------------------

export const decodeBenchmarkMatrixConfig = S.decodeUnknownSync(
  BenchmarkMatrixConfig,
)
export const decodeBenchmarkCell = S.decodeUnknownSync(BenchmarkCell)

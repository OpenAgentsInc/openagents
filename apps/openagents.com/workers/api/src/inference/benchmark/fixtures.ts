// A concrete, public-safe SAMPLE matrix config (book P1-5 / #6088).
//
// This is the "first run produces a dereferenceable report comparing at least
// Fireworks vs Vertex on chat + khala-code workloads" deliverable from the
// issue's done-when, expanded for the current Khala field: Fireworks DeepSeek,
// GPT-OSS 120B/20B, Vertex Gemini, GLM-5.2 REAP pool, Vertex Anthropic, and the
// two named future lanes (Pylon whole-small, Psionic shard-WAN).
//
// The sequence shapes here are SYNTHETIC and labeled as such — they are
// plausible placeholders, NOT sampled from real Khala traffic. Until real
// traffic shapes replace them (and an owner arms a real sweep), every number this
// produces is illustrative. That honesty is encoded in `provenance: 'synthetic'`,
// which makes the report flag every group `syntheticOnly` and `decisionGrade:
// false`.
import { DEFAULT_GLM_52_REAP_504B_OWNED_COST_PROFILE_REF } from '../owned-inference-cost'
import type {
  BenchmarkMatrixConfig,
  BenchmarkTarget,
  BenchmarkTargetProfile,
  SamplingSettings,
  SequenceShape,
} from './matrix'

// Synthetic sequence shapes spanning the dimensions the book cares about: a
// short interactive chat turn, a code-generation request with a large reusable
// system prefix (exercises the prompt cache), and a long-context codebase
// question (large input, modest output). All marked synthetic.
const SHORT_CHAT_SHAPE: SequenceShape = {
  id: 'short-chat',
  inputTokens: 350,
  outputTokens: 220,
  cacheablePrefixTokens: 180,
  concurrency: 1,
  provenance: 'synthetic',
  requestClass: 'interactive_stream',
  source: 'synthetic_fixture',
}

const CODE_ARTIFACT_SHAPE: SequenceShape = {
  id: 'code-artifact-large-prefix',
  inputTokens: 2400,
  outputTokens: 1800,
  // A large stable system/tool/acceptance prefix is reusable across requests.
  cacheablePrefixTokens: 1600,
  concurrency: 4,
  provenance: 'synthetic',
  requestClass: 'interactive_stream',
  source: 'synthetic_fixture',
}

const LONG_CONTEXT_SHAPE: SequenceShape = {
  id: 'long-codebase-32k',
  inputTokens: 32000,
  outputTokens: 600,
  cacheablePrefixTokens: 28000,
  concurrency: 2,
  provenance: 'synthetic',
  requestClass: 'interactive_stream',
  source: 'synthetic_fixture',
}

const OPENCODE_CODING_TASK_SHAPE: SequenceShape = {
  id: 'opencode-edit-run-smoke',
  inputTokens: 1800,
  outputTokens: 700,
  cacheablePrefixTokens: 900,
  concurrency: 1,
  provenance: 'synthetic',
  requestClass: 'interactive_stream',
  source: 'synthetic_fixture',
}

export const OBSERVED_KHALA_FIREWORKS_MIX_SHAPE: SequenceShape = {
  id: 'observed-khala-fireworks-current-mix',
  inputTokens: 573,
  outputTokens: 1448,
  // The 2026-06-25 token-ledger export did not include historical cached-input
  // counts, so the observed shape keeps the cacheable prefix at 0 rather than
  // inventing one. Future observed shapes can fill this from gateway telemetry.
  cacheablePrefixTokens: 0,
  concurrency: 1,
  provenance: 'realistic',
  requestClass: 'interactive_stream',
  observedTrafficEvidenceRef:
    'evidence.openagents.token_usage_events.fireworks_mix.2026_06_25',
  observedRequestCount: 560,
  source: 'operator_export',
}

const DEFAULT_SAMPLING: SamplingSettings = {
  temperature: 0.2,
  reasoningEffort: 'off',
}

const REASONING_SAMPLING: SamplingSettings = {
  temperature: 0.7,
  reasoningEffort: 'medium',
}

const targetProfile = (
  profile: BenchmarkTargetProfile,
): BenchmarkTargetProfile => profile

export const FIREWORKS_DEEPSEEK_V4_FLASH_TARGET: BenchmarkTarget = {
  lane: 'fireworks',
  engine: 'provider-native',
  profile: targetProfile({
    profileRef: 'fireworks.deepseek_v4_flash.provider_native.v1',
    modelRef: 'fireworks/deepseek-v4-flash',
    routeRole: 'comparison',
    capacityClass: 'provider_managed',
    evidenceRefs: ['docs.inference.2026_06_25.khala_cost_model'],
  }),
}

export const VERTEX_GEMINI_FLASH_TARGET: BenchmarkTarget = {
  lane: 'vertex-gemini',
  engine: 'provider-native',
  profile: targetProfile({
    profileRef: 'vertex.gemini_2_5_flash.provider_native.v1',
    modelRef: 'gemini-2.5-flash',
    routeRole: 'fallback',
    capacityClass: 'provider_managed',
    evidenceRefs: ['docs.inference.2026_06_25.khala_cost_model'],
  }),
}

export const VERTEX_ANTHROPIC_TARGET: BenchmarkTarget = {
  lane: 'vertex-anthropic',
  engine: 'provider-native',
  profile: targetProfile({
    profileRef: 'vertex.anthropic.provider_native.v1',
    modelRef: 'vertex/anthropic',
    routeRole: 'comparison',
    capacityClass: 'provider_managed',
    evidenceRefs: ['docs.inference.book.p1_5.minimum_suite'],
  }),
}

export const GPT_OSS_120B_TARGET: BenchmarkTarget = {
  lane: 'gpt-oss-120b',
  engine: 'vllm',
  profile: targetProfile({
    profileRef: 'hydralisk.gpt_oss_120b.vllm.v1',
    modelRef: 'openai/gpt-oss-120b',
    routeRole: 'comparison',
    capacityClass: 'owned_pool',
    replicaPoolRef: 'pool.hydralisk.gpt_oss_120b',
    evidenceRefs: ['docs.inference.2026_06_25.khala_cost_model'],
  }),
}

export const GPT_OSS_20B_TARGET: BenchmarkTarget = {
  lane: 'gpt-oss-20b',
  engine: 'vllm',
  profile: targetProfile({
    profileRef: 'hydralisk.gpt_oss_20b.vllm.v1',
    modelRef: 'openai/gpt-oss-20b',
    routeRole: 'comparison',
    capacityClass: 'owned_pool',
    replicaPoolRef: 'pool.hydralisk.gpt_oss_20b',
    evidenceRefs: ['docs.inference.2026_06_25.khala_cost_model'],
  }),
}

export const GLM_52_REAP_POOL_TARGET: BenchmarkTarget = {
  lane: 'glm-52',
  engine: 'vllm',
  profile: targetProfile({
    profileRef: 'hydralisk.glm_52_reap_504b.pool.vllm.tp4x2.v1',
    modelRef: 'openagents/glm-5.2-reap-504b',
    routeRole: 'first',
    capacityClass: 'owned_pool',
    replicaPoolRef: 'pool.hydralisk.glm_52_reap_504b',
    replicaCount: 2,
    costProfileRef: DEFAULT_GLM_52_REAP_504B_OWNED_COST_PROFILE_REF,
    evidenceRefs: [
      'docs.inference.2026_06_25.khala_glm_52_reap_backing_lane',
      'docs.inference.2026_06_25.khala_cost_model',
    ],
  }),
}

const PYLON_WHOLE_SMALL_TARGET: BenchmarkTarget = {
  lane: 'pylon-whole-small',
  engine: 'vllm',
  profile: targetProfile({
    profileRef: 'pylon.whole_small.future.vllm.v1',
    modelRef: 'openagents/pylon-whole-small',
    routeRole: 'reserved',
    capacityClass: 'fixture',
    evidenceRefs: ['docs.inference.book.p1_5.future_lane'],
  }),
}

const PSIONIC_SHARD_WAN_TARGET: BenchmarkTarget = {
  lane: 'psionic-shard-wan',
  engine: 'sglang',
  profile: targetProfile({
    profileRef: 'psionic.shard_wan.future.sglang.v1',
    modelRef: 'openagents/psionic-shard-wan',
    routeRole: 'reserved',
    capacityClass: 'fixture',
    evidenceRefs: ['docs.inference.book.p1_5.future_lane'],
  }),
}

// The sample/minimum-decision-suite config. It now names the whole immediate
// competitive field: Fireworks DeepSeek V4 Flash, GPT-OSS 120B/20B, Vertex
// Gemini, GLM-5.2 REAP pool, plus Vertex Anthropic and the two future lanes for
// shape completeness. The shapes remain synthetic and therefore illustrative.
export const SAMPLE_DECISION_SUITE_CONFIG: BenchmarkMatrixConfig = {
  id: 'khala-decision-suite-v1',
  description:
    'Khala lane decision suite: Fireworks DeepSeek V4 Flash, GPT-OSS 120B/20B, ' +
    'Vertex Gemini, and GLM-5.2 REAP pool on chat / khala-code / verifier / ' +
    'long-context, with Vertex Anthropic plus Pylon whole-small and Psionic ' +
    'shard-WAN named for comparison/future completeness. Synthetic shapes — ' +
    'illustrative until real traffic + an owner-armed sweep replace them.',
  targets: [
    FIREWORKS_DEEPSEEK_V4_FLASH_TARGET,
    GPT_OSS_120B_TARGET,
    GPT_OSS_20B_TARGET,
    VERTEX_GEMINI_FLASH_TARGET,
    GLM_52_REAP_POOL_TARGET,
    VERTEX_ANTHROPIC_TARGET,
    PYLON_WHOLE_SMALL_TARGET,
    PSIONIC_SHARD_WAN_TARGET,
  ],
  workloads: [
    'chat',
    'khala-code-artifact-gen',
    'verifier-run',
    'long-context-codebase-question',
  ],
  shapes: [SHORT_CHAT_SHAPE, CODE_ARTIFACT_SHAPE, LONG_CONTEXT_SHAPE],
  transports: ['streaming', 'batch'],
  sampling: [DEFAULT_SAMPLING, REASONING_SAMPLING],
  samplesPerCell: 5,
}

export const KHALA_GLM_PROVIDER_OBSERVED_SWEEP_CONFIG: BenchmarkMatrixConfig = {
  ...SAMPLE_DECISION_SUITE_CONFIG,
  id: 'khala-glm-provider-observed-sweep-v1',
  description:
    'Owner-armed decision sweep template over the 2026-06-25 observed Khala ' +
    'Fireworks traffic mix. Uses public-safe aggregate token-ledger evidence; ' +
    'decision-grade execution still requires owner approval, budget caps, and a ' +
    'real spending seam.',
  shapes: [OBSERVED_KHALA_FIREWORKS_MIX_SHAPE],
  transports: ['streaming'],
  sampling: [DEFAULT_SAMPLING],
}

export const OPENCODE_KHALA_VS_BIGPICKLE_FIXTURE_CONFIG: BenchmarkMatrixConfig =
  {
    id: 'opencode-khala-vs-bigpickle-fixture-v1',
    description:
      'OpenCode client-surface fixture: Khala public endpoint vs BigPickle on ' +
      'one synthetic edit/run coding task. No network and no spend; the report ' +
      'is illustrative and decisionGrade:false.',
    targets: [
      { lane: 'khala', engine: 'provider-native' },
      { lane: 'bigpickle', engine: 'provider-native' },
    ],
    workloads: ['opencode-coding-task'],
    shapes: [OPENCODE_CODING_TASK_SHAPE],
    transports: ['streaming'],
    sampling: [DEFAULT_SAMPLING],
    samplesPerCell: 5,
  }

// A tiny config used by tests for exact, hand-checkable expansion + aggregation.
// One real lane, one future lane, one workload, one shape, one transport, one
// sampling, 4 samples — so the expected cell count and per-group math are trivial
// to assert by hand.
export const TINY_TEST_CONFIG: BenchmarkMatrixConfig = {
  id: 'tiny-test-v1',
  description: 'Tiny deterministic config for harness tests.',
  targets: [
    { lane: 'fireworks', engine: 'provider-native' },
    { lane: 'pylon-whole-small', engine: 'vllm' },
  ],
  workloads: ['khala-code-artifact-gen'],
  shapes: [
    {
      id: 'tiny-shape',
      inputTokens: 1000,
      outputTokens: 100,
      cacheablePrefixTokens: 500,
      concurrency: 1,
      provenance: 'synthetic',
    },
  ],
  transports: ['streaming'],
  sampling: [{ temperature: 0, reasoningEffort: 'off' }],
  samplesPerCell: 4,
}

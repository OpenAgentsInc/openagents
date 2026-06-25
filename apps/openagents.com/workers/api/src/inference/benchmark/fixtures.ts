// A concrete, public-safe SAMPLE matrix config (book P1-5 / #6088).
//
// This is the "first run produces a dereferenceable report comparing at least
// Fireworks vs Vertex on chat + khala-code workloads" deliverable from the
// issue's done-when, expressed as a typed config the fixture runner can execute
// deterministically. It is the MINIMUM DECISION SUITE shape (notes Q5): the two
// real managed lanes (Fireworks, Vertex-Anthropic) plus the two named future
// lanes (Pylon whole-small, Psionic shard-WAN, labeled not-yet-available), over
// the chat / khala-code / verifier / long-context workloads.
//
// The sequence shapes here are SYNTHETIC and labeled as such — they are
// plausible placeholders, NOT sampled from real Khala traffic. Until real
// traffic shapes replace them (and an owner arms a real sweep), every number this
// produces is illustrative. That honesty is encoded in `provenance: 'synthetic'`,
// which makes the report flag every group `syntheticOnly` and `decisionGrade:
// false`.
import type {
  BenchmarkMatrixConfig,
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
}

const CODE_ARTIFACT_SHAPE: SequenceShape = {
  id: 'code-artifact-large-prefix',
  inputTokens: 2400,
  outputTokens: 1800,
  // A large stable system/tool/acceptance prefix is reusable across requests.
  cacheablePrefixTokens: 1600,
  concurrency: 4,
  provenance: 'synthetic',
}

const LONG_CONTEXT_SHAPE: SequenceShape = {
  id: 'long-codebase-32k',
  inputTokens: 32000,
  outputTokens: 600,
  cacheablePrefixTokens: 28000,
  concurrency: 2,
  provenance: 'synthetic',
}

const OPENCODE_CODING_TASK_SHAPE: SequenceShape = {
  id: 'opencode-edit-run-smoke',
  inputTokens: 1800,
  outputTokens: 700,
  cacheablePrefixTokens: 900,
  concurrency: 1,
  provenance: 'synthetic',
}

const DEFAULT_SAMPLING: SamplingSettings = {
  temperature: 0.2,
  reasoningEffort: 'off',
}

const REASONING_SAMPLING: SamplingSettings = {
  temperature: 0.7,
  reasoningEffort: 'medium',
}

// The sample/minimum-decision-suite config. Fireworks vs Vertex-Anthropic on
// the four workloads, plus the two future lanes for shape completeness, over the
// three synthetic shapes, both transports, both sampling settings, 5 samples per
// cell (book §4.5.2: enough traffic to read percentiles, not be swayed by one
// outlier).
export const SAMPLE_DECISION_SUITE_CONFIG: BenchmarkMatrixConfig = {
  id: 'khala-decision-suite-v1',
  description:
    'Minimum Khala lane decision suite: Fireworks vs Vertex-Anthropic on chat / ' +
    'khala-code / verifier / long-context, with Pylon whole-small and Psionic ' +
    'shard-WAN named as not-yet-available future lanes. Synthetic shapes — ' +
    'illustrative until real traffic + an owner-armed sweep replace them.',
  targets: [
    { lane: 'fireworks', engine: 'provider-native' },
    { lane: 'vertex-anthropic', engine: 'provider-native' },
    { lane: 'pylon-whole-small', engine: 'vllm' },
    { lane: 'psionic-shard-wan', engine: 'sglang' },
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

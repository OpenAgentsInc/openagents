// Fixture speculation derivation for the benchmark lane (book P1-8 / #6091).
//
// The benchmark fixture lane is the natural home for a DETERMINISTIC "decode
// trace" the speculation telemetry can populate from — there is no real draft
// model in the Worker, so the live serving path discloses `not_measured`/`none`,
// and the only place we can EXERCISE the acceptance-rate plumbing + the
// dynamic-disablement policy end-to-end (without spend) is the fixture lane.
//
// This module turns a benchmark CELL into a fixture speculation outcome:
//
//   1. A CODE workload (artifact-gen / verifier-run / long-context codebase
//      question) is a speculation FIT — generated code repeats syntax + reuses
//      prompt context — so it REQUESTS a draft-free mode (`n_gram`). A plain chat
//      workload does not request speculation (`none`).
//   2. The cell's CONCURRENCY is the batch signal and a derived saturation scalar
//      is the compute-pressure signal. `decideSpeculation` (the bounded policy)
//      then ENABLES speculation at low batch/pressure and DISABLES it otherwise.
//   3. When enabled, the fixture derives plausible draft-token counts (a high
//      acceptance rate for code, lower for a long-context question) so the
//      acceptance-rate telemetry populates from a real count pair — never a bare
//      fabricated rate. When disabled, the outcome is honest mode `none` (we know
//      no speculation ran because the policy turned it off).
//
// PURE/DETERMINISTIC: same cell → same outcome. No clock, no randomness, no spend.
import {
  type KhalaSpeculationInput,
  type KhalaSpeculationMode,
  decideSpeculation,
} from '../khala-speculation'
import type { BenchmarkCell } from './matrix'

// Whether a workload is a code workload that fits draft-free speculation (book
// Ch.5: code repeats syntax + reuses prompt context). A bounded classification of
// the typed workload enum — NOT request-content string matching.
const isCodeWorkload = (cell: BenchmarkCell): boolean => {
  switch (cell.workload) {
    case 'opencode-coding-task':
    case 'khala-code-artifact-gen':
    case 'verifier-run':
    case 'long-context-codebase-question':
    case 'agentcl-source-task':
    case 'agentcl-complex-task':
    case 'agentcl-held-out-task':
      return true
    case 'chat':
      return false
  }
}

// The drafting mode a code workload requests. n-gram drafting (no draft model) is
// the Worker-runnable, code-fitting default; a long-context codebase question
// uses lookahead (KV-cache n-gram table) since it has a large context to mine.
const requestedModeForCell = (cell: BenchmarkCell): KhalaSpeculationMode => {
  if (!isCodeWorkload(cell)) {
    return 'none'
  }
  return cell.workload === 'long-context-codebase-question'
    ? 'lookahead'
    : 'n_gram'
}

// Derive a normalized compute-pressure scalar in [0, 1] from the cell's
// concurrency. A single serial request is near-idle (lots of spare compute to
// verify drafts); pressure climbs with concurrency and saturates by ~16
// concurrent sequences. PURE arithmetic — illustrative, not a real utilization
// measurement (the fixture lane labels its numbers illustrative).
const fixtureComputePressure = (concurrency: number): number => {
  const safe = Number.isFinite(concurrency) && concurrency > 0 ? concurrency : 1
  return Math.min(1, (safe - 1) / 15)
}

// Derive the proposed/accepted draft-token counts for an ENABLED code cell. The
// acceptance rate is illustrative-but-plausible: high for tight code artifact
// generation (lots of syntax repetition), a bit lower for a long-context
// question (more novel content). Counts scale with output length so the rate is
// backed by a real count pair, not a bare number. PURE.
const fixtureDraftCounts = (
  cell: BenchmarkCell,
): Readonly<{ proposed: number; accepted: number }> => {
  // Propose a draft block per ~K output tokens (K=4, the book's typical draft
  // length). Total proposed ≈ outputTokens (every position had a draft attempt).
  const proposed = Math.max(4, cell.shape.outputTokens)
  // Illustrative acceptance: 0.78 for n-gram code gen, 0.62 for lookahead on a
  // long-context question (more novel tokens → fewer repeated n-grams).
  const targetRate =
    cell.workload === 'long-context-codebase-question' ? 0.62 : 0.78
  const accepted = Math.round(proposed * targetRate)
  return { proposed, accepted }
}

// Derive the fixture speculation outcome for a cell: request a mode for code
// workloads, run the dynamic-disablement policy against the cell's batch
// (concurrency) + derived pressure, and produce honest counts only when the
// policy ENABLED speculation. Returns the `KhalaSpeculationInput` the runner
// threads into the telemetry builder. PURE/deterministic.
export const fixtureSpeculationForCell = (
  cell: BenchmarkCell,
): KhalaSpeculationInput => {
  const requestedMode = requestedModeForCell(cell)
  const decision = decideSpeculation({
    requestedMode,
    signal: {
      batchSize: cell.shape.concurrency,
      computePressure: fixtureComputePressure(cell.shape.concurrency),
    },
  })

  if (!decision.enabled) {
    // The policy turned speculation OFF (chat, or high batch/pressure). We KNOW
    // no speculation ran → honest `none` (not the unknown sentinel, and not a
    // fabricated 0 acceptance).
    return { mode: 'none', active: false }
  }

  const { proposed, accepted } = fixtureDraftCounts(cell)
  return {
    mode: decision.selectedMode,
    active: true,
    draftTokensProposed: proposed,
    draftTokensAccepted: accepted,
  }
}

// Khala provider/engine benchmark harness barrel (book P1-5 / #6088).
//
// The typed benchmark matrix, the pluggable lane seam (deterministic fixture lane
// + owner-gated real lane), the runner that records canonical
// `openagents.khala.telemetry.v1` records per sample, and the public-safe
// dereferenceable report (latency percentiles, perceived TPS,
// cost-per-accepted-outcome, verification rate). See the sibling modules for the
// honesty/public-safety discipline.
export * from './matrix'
export * from './lane-seam'
export * from './speculation-lane'
export * from './runner'
export * from './report'
export * from './fixtures'

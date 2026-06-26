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
export * from './real-sweep-plan'
export * from './opencode-client-runner'
// Owner-armed real decision sweep (Open Question #5 suite / #6307): the live
// provider executors, the concrete lane transports, the decision-grade config,
// and the async sweep runner that produces the first decisionGrade:true report.
export * from './real-lane-executor'
export * from './real-lane-transports'
export * from './real-sweep-config'
export * from './real-sweep-runner'
// External head-to-head publication layer (#6308): the developer-default
// comparator set, the recurring published quality bar (Khala vs the
// tools/models a developer would otherwise use, scored on solve-rate AND
// cost-per-accepted-outcome), its D1 snapshot store, and the public + operator
// routes — all built on the merged harness + gym leaderboard projection.
export * from './head-to-head'
export * from './head-to-head-store'
export * from './head-to-head-routes'

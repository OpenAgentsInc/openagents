# @openagentsinc/proof-replay

Shared deterministic proof replay primitives for OpenAgents web and Autopilot
Desktop. The package consumes public-safe `proof_replay_bundle.v1` payloads and
produces replay clock state, ordered timeline state, stage and actor layout
plans, camera cue plans, hit targets, and payment visual classifications.

The package is presentation-only. It does not validate proofs, authorize
settlement, dispatch payments, read wallet state, or promote product claims.

-- Wave 1 cleanup (#8380): remove the Postgres twins for the retired AgentCL
-- Vertex eval table family. Migration 0026 created these for a copy/verify
-- short path; the runner and active registry are gone now.

DROP TABLE IF EXISTS gym_agentcl_eval_prompt_mutations;
DROP TABLE IF EXISTS gym_agentcl_eval_run_state_events;
DROP TABLE IF EXISTS gym_agentcl_eval_gain_metrics;
DROP TABLE IF EXISTS gym_agentcl_eval_phase_metrics;
DROP TABLE IF EXISTS gym_agentcl_eval_runs;

-- Wave 1 cleanup (#8380): retire the write-dead AgentCL Vertex eval table
-- family after removing the only runner/registry surfaces that kept it alive.

DROP TABLE IF EXISTS gym_agentcl_eval_prompt_mutations;
DROP TABLE IF EXISTS gym_agentcl_eval_run_state_events;
DROP TABLE IF EXISTS gym_agentcl_eval_gain_metrics;
DROP TABLE IF EXISTS gym_agentcl_eval_phase_metrics;
DROP TABLE IF EXISTS gym_agentcl_eval_runs;

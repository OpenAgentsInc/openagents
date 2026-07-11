-- Claude reports uncached input tokens and cache-read tokens as independent
-- counters. A warm prompt can therefore read more cached tokens than the
-- uncached input count without changing billed/served total semantics.

ALTER TABLE sarah_fleet_run_attempts
  DROP CONSTRAINT sarah_fleet_run_attempts_usage_columns_coherence;

ALTER TABLE sarah_fleet_run_attempts
  ADD CONSTRAINT sarah_fleet_run_attempts_usage_columns_coherence
    CHECK (
      (usage_truth = 'pending'
        AND usage_json = '{"truth":"pending"}'
        AND usage_evidence_ref IS NULL
        AND usage_provider IS NULL
        AND usage_model IS NULL
        AND usage_demand_kind IS NULL
        AND usage_demand_source IS NULL
        AND usage_input_tokens IS NULL
        AND usage_output_tokens IS NULL
        AND usage_reasoning_tokens IS NULL
        AND usage_cache_read_tokens IS NULL
        AND usage_total_tokens IS NULL
        AND usage_token_rows IS NULL
        AND token_usage_refs_json = '[]')
      OR
      (usage_truth = 'exact'
        AND usage_json::jsonb ->> 'truth' = 'exact'
        AND usage_evidence_ref IS NOT NULL
        AND usage_provider IN (
          'pylon-codex-own-capacity', 'pylon-claude-own-capacity'
        )
        AND usage_model IN (
          'openagents/pylon-codex', 'openagents/pylon-claude'
        )
        AND usage_demand_kind = 'own_capacity'
        AND usage_demand_source = 'khala_coding_delegation'
        AND usage_input_tokens >= 0
        AND usage_output_tokens >= 0
        AND usage_reasoning_tokens >= 0
        AND usage_reasoning_tokens <= usage_output_tokens
        AND usage_cache_read_tokens >= 0
        AND usage_total_tokens > 0
        AND usage_total_tokens = usage_input_tokens + usage_output_tokens
        AND usage_token_rows > 0
        AND token_usage_refs_json <> '[]')
      OR
      (usage_truth = 'not_measured'
        AND usage_json::jsonb ->> 'truth' = 'not_measured'
        AND usage_evidence_ref IS NOT NULL
        AND usage_provider IS NULL
        AND usage_model IS NULL
        AND usage_demand_kind IS NULL
        AND usage_demand_source IS NULL
        AND usage_input_tokens IS NULL
        AND usage_output_tokens IS NULL
        AND usage_reasoning_tokens IS NULL
        AND usage_cache_read_tokens IS NULL
        AND usage_total_tokens IS NULL
        AND usage_token_rows IS NULL
        AND token_usage_refs_json = '[]')
    );

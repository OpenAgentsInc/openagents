CREATE TABLE IF NOT EXISTS sarah_livekit_tool_proposals (
  session_ref                 text NOT NULL
    REFERENCES sarah_realtime_voice_sessions (session_ref) ON DELETE CASCADE,
  generation                  bigint NOT NULL CHECK (generation >= 1),
  proposal_ref                text NOT NULL CHECK (
    length(proposal_ref) BETWEEN 1 AND 256
  ),
  proposal_digest             text NOT NULL CHECK (
    proposal_digest ~ '^[0-9a-f]{64}$'
  ),
  worker_job_ref              text NOT NULL CHECK (
    length(worker_job_ref) BETWEEN 1 AND 256
  ),
  worker_control_token_digest text NOT NULL CHECK (
    worker_control_token_digest ~ '^[0-9a-f]{64}$'
  ),
  worker_event_ref            text NOT NULL CHECK (
    length(worker_event_ref) BETWEEN 1 AND 256
  ),
  provider_call_ref           text NOT NULL CHECK (
    length(provider_call_ref) BETWEEN 1 AND 256
  ),
  command_payload_digest      text NOT NULL CHECK (
    command_payload_digest ~ '^[0-9a-f]{64}$'
  ),
  command                     jsonb NOT NULL CHECK (
    command->>'_tag' = 'start_agent_thread'
  ),
  state                       text NOT NULL CHECK (
    state IN ('proposed', 'declined', 'execute_sent', 'outcome')
  ),
  outcome_ref                 text,
  outcome_ok                  boolean,
  outcome_summary             text,
  created_at                  text NOT NULL,
  expires_at                  text NOT NULL,
  decision_at                 text,
  outcome_at                  text,
  PRIMARY KEY (session_ref, generation, proposal_ref),
  UNIQUE (session_ref, generation, worker_event_ref),
  CHECK (
    (
      state = 'outcome'
      AND outcome_ref IS NOT NULL
      AND outcome_ok IS NOT NULL
      AND outcome_summary IS NOT NULL
      AND outcome_at IS NOT NULL
    )
    OR
    (
      state <> 'outcome'
      AND outcome_ref IS NULL
      AND outcome_ok IS NULL
      AND outcome_summary IS NULL
      AND outcome_at IS NULL
    )
  )
);

CREATE INDEX IF NOT EXISTS sarah_livekit_tool_proposals_control_idx
  ON sarah_livekit_tool_proposals (
    session_ref,
    generation,
    state,
    created_at
  );

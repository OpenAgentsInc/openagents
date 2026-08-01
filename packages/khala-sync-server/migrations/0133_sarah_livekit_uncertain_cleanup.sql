UPDATE sarah_livekit_room_bindings AS binding
SET state = 'cleanup_ready',
    cleanup_attempt_count = 0,
    cleanup_next_attempt_at = NULL,
    cleanup_abandoned_at = NULL,
    updated_at = session.updated_at
FROM sarah_realtime_voice_sessions AS session
WHERE binding.session_ref = session.session_ref
  AND binding.generation = session.generation
  AND binding.state IN ('prepared', 'active', 'cleanup_failed')
  AND session.state = 'accounting_uncertain';

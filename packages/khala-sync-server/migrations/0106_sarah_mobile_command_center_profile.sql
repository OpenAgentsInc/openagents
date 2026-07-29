ALTER TABLE sarah_realtime_voice_sessions
  DROP CONSTRAINT IF EXISTS sarah_realtime_voice_client_profile_check;

ALTER TABLE sarah_realtime_voice_sessions
  ADD CONSTRAINT sarah_realtime_voice_client_profile_check
  CHECK (client_profile IN ('omega_editor', 'mobile_voice_only', 'mobile_command_center'));

ALTER TABLE sarah_realtime_voice_sessions
  ADD COLUMN IF NOT EXISTS client_profile text NOT NULL DEFAULT 'omega_editor';

ALTER TABLE sarah_realtime_voice_sessions
  DROP CONSTRAINT IF EXISTS sarah_realtime_voice_client_profile_check;

ALTER TABLE sarah_realtime_voice_sessions
  ADD CONSTRAINT sarah_realtime_voice_client_profile_check
  CHECK (client_profile IN ('omega_editor', 'mobile_voice_only'));

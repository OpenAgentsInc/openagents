-- Supabase Schema for ACP Bridge
-- Run this migration in your Supabase project

-- Sessions table: tracks ACP agent sessions
CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY DEFAULT gen_random_uuid()::TEXT,
  agent_type TEXT NOT NULL CHECK (agent_type IN ('claude-code', 'codex')),
  acp_session_id TEXT UNIQUE NOT NULL,
  cwd TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'completed', 'error')),
  metadata JSONB DEFAULT '{}'::JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Messages table: stores all conversation messages
CREATE TABLE IF NOT EXISTS messages (
  id TEXT PRIMARY KEY DEFAULT gen_random_uuid()::TEXT,
  session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
  content TEXT NOT NULL,
  metadata JSONB DEFAULT '{}'::JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

  -- Indexes for common queries
  INDEX idx_messages_session_created (session_id, created_at DESC),
  INDEX idx_messages_role (role)
);

-- Enable Row Level Security (optional but recommended)
ALTER TABLE sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE messages ENABLE ROW LEVEL SECURITY;

-- Example RLS policies (adjust based on your auth setup)
-- Allow authenticated users to read their own sessions
CREATE POLICY "Users can read own sessions" ON sessions
  FOR SELECT USING (auth.uid() IS NOT NULL);

-- Allow authenticated users to insert messages
CREATE POLICY "Users can insert messages" ON messages
  FOR INSERT WITH CHECK (auth.uid() IS NOT NULL);

-- Allow authenticated users to read messages from their sessions
CREATE POLICY "Users can read messages" ON messages
  FOR SELECT USING (auth.uid() IS NOT NULL);

-- Enable Realtime for both tables
ALTER PUBLICATION supabase_realtime ADD TABLE sessions;
ALTER PUBLICATION supabase_realtime ADD TABLE messages;

-- Create function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to auto-update updated_at on sessions
CREATE TRIGGER update_sessions_updated_at
  BEFORE UPDATE ON sessions
  FOR EACH ROW
  EXECUTE FUNCTION update_updated_at_column();

-- Comments for documentation
COMMENT ON TABLE sessions IS 'ACP agent sessions (claude-code, codex)';
COMMENT ON TABLE messages IS 'Conversation messages for ACP sessions';
COMMENT ON COLUMN sessions.acp_session_id IS 'Internal ACP session ID from agent';
COMMENT ON COLUMN sessions.metadata IS 'Additional session data (error info, config, etc.)';
COMMENT ON COLUMN messages.metadata IS 'Message metadata (tool_calls, reasoning, chunks, etc.)';

-- Create solver_states table
CREATE TABLE IF NOT EXISTS solver_states (
    id TEXT PRIMARY KEY,
    status JSONB NOT NULL,
    issue_number INTEGER NOT NULL,
    issue_title TEXT NOT NULL,
    issue_body TEXT NOT NULL,
    files JSONB NOT NULL,
    repo_path TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Add updated_at trigger
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Add repo_path column to solver_states
ALTER TABLE solver_states ADD COLUMN IF NOT EXISTS repo_path TEXT;

-- Update trigger if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'update_solver_states_updated_at') THEN
        CREATE TRIGGER update_solver_states_updated_at
            BEFORE UPDATE ON solver_states
            FOR EACH ROW
            EXECUTE FUNCTION update_updated_at_column();
    END IF;
END $$;

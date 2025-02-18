-- Add repomap_cache table
CREATE TABLE IF NOT EXISTS repomap_cache (
    repo_name TEXT NOT NULL,
    branch TEXT NOT NULL,
    commit_sha TEXT NOT NULL,
    map_data JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (repo_name, branch, commit_sha)
);

-- Add index for faster lookups
CREATE INDEX IF NOT EXISTS idx_repomap_cache_lookup
ON repomap_cache (repo_name, branch, commit_sha);

-- Add index for cleanup of old entries
CREATE INDEX IF NOT EXISTS idx_repomap_cache_created_at
ON repomap_cache (created_at);

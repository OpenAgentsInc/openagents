-- Per-file knowledge cache with SHA tracking
-- Enables agent to know which files changed since last exploration

CREATE TABLE file_knowledge (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    repo TEXT NOT NULL,                    -- "owner/repo"
    path TEXT NOT NULL,                    -- file path within repo
    sha TEXT NOT NULL,                     -- Git blob SHA of this version
    content_preview TEXT,                  -- First 4KB of content
    file_type TEXT NOT NULL,               -- "file" or "dir"
    size INTEGER,                          -- file size in bytes
    viewed_at TEXT NOT NULL,               -- when this version was viewed

    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT,

    UNIQUE(user_id, repo, path)
);

CREATE INDEX idx_file_knowledge_user_repo ON file_knowledge(user_id, repo);
CREATE INDEX idx_file_knowledge_path ON file_knowledge(repo, path);

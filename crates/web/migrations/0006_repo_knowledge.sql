-- Repo-level knowledge cache for agent memory persistence
-- Stores AI-generated insights and exploration results per user/repo

CREATE TABLE repo_knowledge (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    repo TEXT NOT NULL,                    -- "owner/repo"

    -- Initial exploration cache (from /api/github/explore)
    repo_metadata TEXT,                    -- JSON: description, language, stars, issues, PRs
    recent_commits TEXT,                   -- JSON: last 10 commits
    file_tree TEXT,                        -- JSON: directory structure
    readme_excerpt TEXT,

    -- AI-generated insights (from agent loop completion)
    ai_summary TEXT,                       -- "what this project does"
    ai_suggestions TEXT,                   -- JSON: array of 3 task suggestions
    files_viewed TEXT,                     -- JSON: paths the AI actually read

    -- Freshness tracking
    explored_at TEXT NOT NULL,             -- when initial exploration happened
    insights_at TEXT,                      -- when AI insights were generated
    commit_sha TEXT,                       -- HEAD commit at exploration time

    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT,

    UNIQUE(user_id, repo)
);

CREATE INDEX idx_repo_knowledge_user_repo ON repo_knowledge(user_id, repo);

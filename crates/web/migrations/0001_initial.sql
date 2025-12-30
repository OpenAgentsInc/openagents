-- OpenAgents D1 Schema
-- Initial migration for Cloudflare Workers deployment

-- Users table
CREATE TABLE IF NOT EXISTS users (
    user_id TEXT PRIMARY KEY,
    email TEXT,
    username TEXT,
    github_id TEXT UNIQUE,
    github_username TEXT,
    github_access_token_encrypted TEXT,
    api_key_encrypted TEXT,
    billing_info_encrypted TEXT,
    handoff_token_encrypted TEXT,
    handoff_expires_at TEXT,
    deleted_at TEXT,
    updated_at TEXT,
    signup_credits INTEGER NOT NULL DEFAULT 100000,
    purchased_credits INTEGER NOT NULL DEFAULT 0,
    credits_balance INTEGER NOT NULL DEFAULT 100000,
    payment_method_status TEXT DEFAULT 'none',
    payment_method_brand TEXT,
    payment_method_last4 TEXT,
    payment_method_added_at TEXT,
    low_balance_threshold INTEGER DEFAULT 20000,
    low_balance_warned INTEGER DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_users_github_id ON users(github_id);
CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);

-- Stripe customers
CREATE TABLE IF NOT EXISTS stripe_customers (
    user_id TEXT PRIMARY KEY REFERENCES users(user_id) ON DELETE CASCADE,
    stripe_customer_id TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT
);

-- Stripe payment methods
CREATE TABLE IF NOT EXISTS stripe_payment_methods (
    stripe_payment_method_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    pm_type TEXT NOT NULL,
    brand TEXT,
    last4 TEXT,
    exp_month INTEGER,
    exp_year INTEGER,
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_stripe_pm_user ON stripe_payment_methods(user_id);

-- Usage events (credits tracking)
CREATE TABLE IF NOT EXISTS usage_events (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    session_id TEXT,
    event_type TEXT NOT NULL,
    model TEXT,
    input_tokens INTEGER DEFAULT 0,
    output_tokens INTEGER DEFAULT 0,
    credits_used INTEGER NOT NULL DEFAULT 0,
    refunded INTEGER DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_usage_user ON usage_events(user_id);
CREATE INDEX IF NOT EXISTS idx_usage_session ON usage_events(session_id);

-- Invoices
CREATE TABLE IF NOT EXISTS invoices (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    amount_cents INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    pdf_url TEXT,
    issued_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_invoices_user ON invoices(user_id);

-- Repo write access
CREATE TABLE IF NOT EXISTS repo_write_access (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    repo TEXT NOT NULL,
    scope TEXT NOT NULL DEFAULT 'write',
    granted_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(user_id, repo)
);

-- Persistent sessions (audit trail, KV is primary store)
CREATE TABLE IF NOT EXISTS persistent_sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    user_agent TEXT,
    ip_address TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT NOT NULL,
    last_active_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sessions_user ON persistent_sessions(user_id);

-- HUD visibility settings (GTM requirement)
CREATE TABLE IF NOT EXISTS hud_settings (
    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    repo TEXT NOT NULL,
    is_public INTEGER NOT NULL DEFAULT 1,
    embed_allowed INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (user_id, repo)
);

CREATE INDEX IF NOT EXISTS idx_hud_repo ON hud_settings(repo);

-- Credit adjustments ledger
CREATE TABLE IF NOT EXISTS credit_adjustments (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    run_id TEXT,
    credits_delta INTEGER NOT NULL,
    reason TEXT,
    created_by TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_adjustments_user ON credit_adjustments(user_id);

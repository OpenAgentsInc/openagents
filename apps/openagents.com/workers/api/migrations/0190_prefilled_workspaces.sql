-- Prefilled project workspace primitive (Epic C / C1).
--
-- A reusable onboarding unit a prospect or holder is invited into instead of a
-- blank chat: a named project + seeded grounded memory (public-source refs
-- only) + 1-3 one-click starter accepted-outcome workflows + an intro receipt.
--
-- PUBLIC-SAFE / COMPLIANCE INVARIANT: every column here is seeded from public
-- data only. No private account material, secrets, credentials, raw prompts,
-- wallet data, or individual people's names belong in any column. The
-- holder_ref is an opaque generic prospect reference (no client/person names).

CREATE TABLE IF NOT EXISTS prefilled_workspaces (
  id TEXT PRIMARY KEY NOT NULL,
  -- The holder once they sign in. Null until the invite is claimed.
  holder_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  -- Opaque, generic prospect reference used to seed before sign-in.
  holder_ref TEXT NOT NULL,
  project_name TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN ('draft', 'invited', 'active', 'archived')
  ),
  -- The intro receipt (summary + public source refs) as JSON.
  intro_receipt_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS prefilled_workspaces_holder_idx
  ON prefilled_workspaces(holder_user_id, updated_at DESC)
  WHERE holder_user_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS prefilled_workspaces_status_idx
  ON prefilled_workspaces(status, updated_at DESC)
  WHERE archived_at IS NULL;

-- Seeded grounded-memory facts. Each fact carries its public provenance ref;
-- provenance-first, nothing private until the holder connects their accounts.
CREATE TABLE IF NOT EXISTS prefilled_workspace_seeded_memory (
  workspace_id TEXT NOT NULL REFERENCES prefilled_workspaces(id)
    ON DELETE CASCADE,
  position INTEGER NOT NULL,
  label TEXT NOT NULL,
  value TEXT NOT NULL,
  public_source_ref TEXT NOT NULL,
  PRIMARY KEY (workspace_id, position)
);

-- 1-3 starter accepted-outcome workflows, queued or one-click-runnable.
CREATE TABLE IF NOT EXISTS prefilled_workspace_starter_workflows (
  workspace_id TEXT NOT NULL REFERENCES prefilled_workspaces(id)
    ON DELETE CASCADE,
  position INTEGER NOT NULL,
  title TEXT NOT NULL,
  description TEXT NOT NULL,
  outcome_kind TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN ('queued', 'ready', 'completed', 'dismissed')
  ),
  PRIMARY KEY (workspace_id, position)
);

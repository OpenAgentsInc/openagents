-- FORGE-03 (#9245): the Forge admission front.
--
-- A repository enters the bare-Git service only through a verified 30617
-- announcement. Signed 30618 ref facts are admission credentials, not ref
-- authority. The outbox is durable relay projection work and is populated
-- only after the bare repository has accepted a matching update.

CREATE TABLE IF NOT EXISTS forge_git_repository_admissions (
  tenant_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  announcement_event_id TEXT NOT NULL,
  announcement_author_pubkey TEXT NOT NULL,
  admitted_binding_ref TEXT NOT NULL,
  maintainer_pubkeys_json TEXT NOT NULL DEFAULT '[]',
  state TEXT NOT NULL CHECK (state IN ('admitted', 'revoked')),
  admitted_at TEXT NOT NULL,
  revoked_at TEXT,
  PRIMARY KEY (tenant_ref, repository_ref),
  UNIQUE (announcement_event_id)
);

CREATE TABLE IF NOT EXISTS forge_git_signed_ref_states (
  tenant_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  ref_name TEXT NOT NULL,
  event_id TEXT NOT NULL,
  author_pubkey TEXT NOT NULL,
  old_object_id TEXT NOT NULL,
  new_object_id TEXT NOT NULL,
  event_json TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('authorized', 'refused')),
  authorized_at TEXT NOT NULL,
  applied_at TEXT,
  superseded_at TEXT,
  PRIMARY KEY (tenant_ref, repository_ref, ref_name, event_id)
);
CREATE INDEX IF NOT EXISTS idx_forge_git_signed_ref_states_current
  ON forge_git_signed_ref_states(tenant_ref, repository_ref, ref_name)
  WHERE state = 'authorized' AND superseded_at IS NULL;

CREATE TABLE IF NOT EXISTS forge_git_purgatory_events (
  tenant_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  event_id TEXT NOT NULL,
  kind INTEGER NOT NULL CHECK (kind IN (30617, 30618, 1617, 1618, 1619)),
  actor_binding_ref TEXT NOT NULL,
  required_object_ids_json TEXT NOT NULL DEFAULT '[]',
  event_json TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('pending', 'resolved', 'expired')),
  expires_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  resolved_at TEXT,
  PRIMARY KEY (tenant_ref, event_id)
);
CREATE INDEX IF NOT EXISTS idx_forge_git_purgatory_events_due
  ON forge_git_purgatory_events(state, expires_at);

CREATE TABLE IF NOT EXISTS forge_git_relay_outbox (
  outbox_ref TEXT PRIMARY KEY,
  tenant_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  event_id TEXT NOT NULL,
  kind INTEGER NOT NULL CHECK (kind = 30618),
  state TEXT NOT NULL CHECK (state IN ('pending', 'published', 'failed')),
  available_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  published_at TEXT,
  UNIQUE (tenant_ref, event_id)
);
CREATE INDEX IF NOT EXISTS idx_forge_git_relay_outbox_pending
  ON forge_git_relay_outbox(state, available_at);

CREATE TABLE IF NOT EXISTS forge_git_unclaimed_nostr_refs (
  tenant_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  event_id TEXT NOT NULL,
  ref_name TEXT NOT NULL,
  first_seen_at TEXT NOT NULL,
  gc_after TEXT NOT NULL,
  claimed_at TEXT,
  deleted_at TEXT,
  PRIMARY KEY (tenant_ref, repository_ref, ref_name)
);
CREATE INDEX IF NOT EXISTS idx_forge_git_unclaimed_nostr_refs_due
  ON forge_git_unclaimed_nostr_refs(gc_after)
  WHERE claimed_at IS NULL AND deleted_at IS NULL;

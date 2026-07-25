-- FORGE-04 (#9246): invite-bound Forge actors and transport replay defense.
--
-- ForgeActorBinding remains the canonical actor record from the Forge
-- ProductSpec. The other four tables are separate operational records. Raw
-- invite secrets and raw Git credentials are not stored.

CREATE TABLE IF NOT EXISTS forge_actor_bindings (
  binding_ref TEXT PRIMARY KEY,
  tenant_ref TEXT NOT NULL,
  account_ref TEXT NOT NULL,
  actor_kind TEXT NOT NULL CHECK (actor_kind IN ('human', 'agent')),
  display_name TEXT NOT NULL,
  owner_binding_ref TEXT REFERENCES forge_actor_bindings(binding_ref),
  role_refs_json TEXT NOT NULL DEFAULT '[]',
  membership_state TEXT NOT NULL
    CHECK (membership_state IN ('active', 'tombstoned')),
  binding_generation INTEGER NOT NULL DEFAULT 1
    CHECK (binding_generation > 0),
  created_at TEXT NOT NULL,
  revoked_at TEXT,
  nostr_pubkey TEXT,
  nostr_binding_event_id TEXT,
  nostr_binding_created_at TEXT,
  nostr_binding_signature_valid INTEGER NOT NULL DEFAULT 0
    CHECK (nostr_binding_signature_valid IN (0, 1)),
  FOREIGN KEY (tenant_ref) REFERENCES forge_tenants(tenant_ref)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_actor_bindings_account
  ON forge_actor_bindings(tenant_ref, account_ref, actor_kind);

CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_actor_bindings_nostr_pubkey
  ON forge_actor_bindings(tenant_ref, nostr_pubkey)
  WHERE nostr_pubkey IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_forge_actor_bindings_owner
  ON forge_actor_bindings(tenant_ref, owner_binding_ref, membership_state);

CREATE TABLE IF NOT EXISTS forge_invite_bindings (
  invite_binding_ref TEXT PRIMARY KEY,
  tenant_ref TEXT NOT NULL,
  team_ref TEXT NOT NULL,
  invite_ref TEXT NOT NULL,
  invite_digest TEXT NOT NULL,
  invite_kind TEXT NOT NULL CHECK (invite_kind = 'team_workspace'),
  inviter_binding_ref TEXT NOT NULL,
  invited_subject_ref TEXT NOT NULL,
  role_refs_json TEXT NOT NULL DEFAULT '[]',
  issued_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  accepted_at TEXT,
  accepted_binding_ref TEXT REFERENCES forge_actor_bindings(binding_ref),
  provenance_source_refs_json TEXT NOT NULL DEFAULT '[]',
  FOREIGN KEY (tenant_ref) REFERENCES forge_tenants(tenant_ref),
  UNIQUE (tenant_ref, invite_ref)
);

CREATE INDEX IF NOT EXISTS idx_forge_invite_bindings_subject
  ON forge_invite_bindings(tenant_ref, invited_subject_ref, accepted_at);

CREATE TABLE IF NOT EXISTS forge_burned_key_facts (
  burned_key_fact_ref TEXT PRIMARY KEY,
  tenant_ref TEXT NOT NULL,
  key_kind TEXT NOT NULL CHECK (key_kind IN ('human', 'agent')),
  public_key TEXT NOT NULL,
  binding_ref TEXT NOT NULL REFERENCES forge_actor_bindings(binding_ref),
  burn_reason_ref TEXT NOT NULL,
  burned_at TEXT NOT NULL,
  burn_sequence INTEGER NOT NULL CHECK (burn_sequence > 0),
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  FOREIGN KEY (tenant_ref) REFERENCES forge_tenants(tenant_ref),
  UNIQUE (tenant_ref, burn_sequence)
);

CREATE INDEX IF NOT EXISTS idx_forge_burned_key_facts_public_key
  ON forge_burned_key_facts(tenant_ref, public_key, burn_sequence DESC);

CREATE TABLE IF NOT EXISTS forge_nip98_replay_consumptions (
  consumption_ref TEXT PRIMARY KEY,
  tenant_ref TEXT NOT NULL,
  request_digest TEXT NOT NULL,
  event_id TEXT NOT NULL,
  actor_pubkey TEXT NOT NULL,
  http_method TEXT NOT NULL,
  canonical_path TEXT NOT NULL,
  body_digest TEXT NOT NULL,
  event_created_at TEXT NOT NULL,
  consumed_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  authority_generation INTEGER NOT NULL CHECK (authority_generation > 0),
  result TEXT NOT NULL CHECK (result IN ('accepted', 'refused')),
  FOREIGN KEY (tenant_ref) REFERENCES forge_tenants(tenant_ref),
  UNIQUE (event_id),
  UNIQUE (request_digest)
);

CREATE INDEX IF NOT EXISTS idx_forge_nip98_replay_expiry
  ON forge_nip98_replay_consumptions(expires_at);

CREATE TABLE IF NOT EXISTS forge_membership_reconciliation_state (
  reconciliation_ref TEXT PRIMARY KEY,
  tenant_ref TEXT NOT NULL,
  team_ref TEXT NOT NULL,
  binding_ref TEXT NOT NULL REFERENCES forge_actor_bindings(binding_ref),
  source_membership_generation INTEGER NOT NULL DEFAULT 0
    CHECK (source_membership_generation >= 0),
  reconciliation_generation INTEGER NOT NULL DEFAULT 1
    CHECK (reconciliation_generation > 0),
  observed_present INTEGER NOT NULL CHECK (observed_present IN (0, 1)),
  absence_first_observed_at TEXT,
  absence_confirmed_at TEXT,
  hysteresis_deadline TEXT,
  state TEXT NOT NULL
    CHECK (state IN ('present', 'absence_pending', 'absence_confirmed')),
  reconciled_at TEXT NOT NULL,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  FOREIGN KEY (tenant_ref) REFERENCES forge_tenants(tenant_ref),
  UNIQUE (tenant_ref, binding_ref)
);

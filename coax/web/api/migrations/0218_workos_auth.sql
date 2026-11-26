-- WorkOS hybrid acct_ spine & local Postgres mirror (docs/auth/2026-11-26-workos-coax-sdk.md, #14100).
--
-- Keeps opaque acct_... as OpenAgents' canonical identity spine for Nostr-only accounts (D8),
-- self-custodial wallets & NIP-VAULT backups (D4-D5), Lightning Addresses (D6), and sat-capped
-- agent grants (#13211), while linking WorkOS user IDs in auth.workos_identities and mirroring
-- WorkOS entities (users, organizations, memberships, sessions, events) into the workos.* schema
-- via the pure Coax/C webhook + Events API poller (zero Node sidecar).

ALTER TABLE auth.sessions DROP CONSTRAINT sessions_via_kind_check;
ALTER TABLE auth.sessions ADD CONSTRAINT sessions_via_kind_check
  CHECK (via_kind IN ('email', 'nostr', 'passkey', 'device', 'recovery', 'workos'));

CREATE TABLE auth.workos_identities (
  workos_user_id text PRIMARY KEY CHECK (length(workos_user_id) BETWEEN 4 AND 128),
  account text NOT NULL REFERENCES auth.accounts (id),
  provider text,
  last_organization_id text,
  linked_at timestamptz NOT NULL DEFAULT now(),
  last_seen_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX workos_identities_account ON auth.workos_identities (account);

-- Register auth.workos_identities for account merge, export, and 30-day purge (0149_auth_life.sql).
INSERT INTO auth.life_columns (tbl, col, holds, on_merge, on_purge, export, omit, why) VALUES
  ('auth.workos_identities', 'account', 'account', 'move', 'delete', true, '{}', 'WorkOS identity links')
ON CONFLICT (tbl, col) DO NOTHING;

-- Short-lived server-side state for WorkOS web/app flows (Magic Auth email binding, OAuth/SSO PKCE
-- verifier + state + next path, and multi-org organization_selection_required pending tokens).
CREATE TABLE auth.workos_pending (
  id text PRIMARY KEY CHECK (id ~ '^[0-9a-f]{32}$'),
  kind text NOT NULL CHECK (kind IN ('magic', 'oauth', 'org')),
  token_sha256 text NOT NULL UNIQUE CHECK (token_sha256 ~ '^[0-9a-f]{64}$'),
  email text,
  verifier text,
  next_path text,
  pending_auth_token text,
  orgs_json jsonb,
  client text NOT NULL CHECK (client IN ('web', 'app')),
  expires_at timestamptz NOT NULL,
  spent_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX workos_pending_expires ON auth.workos_pending (expires_at);

-- Local WorkOS entity mirror (_raw jsonb + generated columns + deleted_at tombstones + *_live views).
CREATE SCHEMA IF NOT EXISTS workos;

CREATE TABLE workos.users (
  id text PRIMARY KEY,
  _raw jsonb NOT NULL,
  email text GENERATED ALWAYS AS (_raw->>'email') STORED,
  email_verified boolean GENERATED ALWAYS AS (coalesce((_raw->>'email_verified')::boolean, false)) STORED,
  external_id text GENERATED ALWAYS AS (_raw->>'external_id') STORED,
  first_name text GENERATED ALWAYS AS (_raw->>'first_name') STORED,
  last_name text GENERATED ALWAYS AS (_raw->>'last_name') STORED,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz
);
CREATE INDEX workos_users_external_id ON workos.users (external_id) WHERE external_id IS NOT NULL;
CREATE INDEX workos_users_email ON workos.users (lower(email)) WHERE email IS NOT NULL;
CREATE VIEW workos.users_live AS SELECT * FROM workos.users WHERE deleted_at IS NULL;

CREATE TABLE workos.organizations (
  id text PRIMARY KEY,
  _raw jsonb NOT NULL,
  name text GENERATED ALWAYS AS (_raw->>'name') STORED,
  external_id text GENERATED ALWAYS AS (_raw->>'external_id') STORED,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz
);
CREATE VIEW workos.organizations_live AS SELECT * FROM workos.organizations WHERE deleted_at IS NULL;

CREATE TABLE workos.organization_memberships (
  id text PRIMARY KEY,
  _raw jsonb NOT NULL,
  user_id text GENERATED ALWAYS AS (_raw->>'user_id') STORED,
  organization_id text GENERATED ALWAYS AS (_raw->>'organization_id') STORED,
  status text GENERATED ALWAYS AS (_raw->>'status') STORED,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz
);
CREATE INDEX workos_memberships_user ON workos.organization_memberships (user_id) WHERE deleted_at IS NULL;
CREATE INDEX workos_memberships_org ON workos.organization_memberships (organization_id) WHERE deleted_at IS NULL;
CREATE VIEW workos.organization_memberships_live AS SELECT * FROM workos.organization_memberships WHERE deleted_at IS NULL;

CREATE TABLE workos.sessions (
  id text PRIMARY KEY,
  _raw jsonb NOT NULL,
  user_id text GENERATED ALWAYS AS (_raw->>'user_id') STORED,
  organization_id text GENERATED ALWAYS AS (_raw->>'organization_id') STORED,
  status text GENERATED ALWAYS AS (_raw->>'status') STORED,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz
);
CREATE INDEX workos_sessions_user ON workos.sessions (user_id) WHERE deleted_at IS NULL;
CREATE VIEW workos.sessions_live AS SELECT * FROM workos.sessions WHERE deleted_at IS NULL;

CREATE TABLE workos.events (
  id text PRIMARY KEY,
  event text NOT NULL,
  _raw jsonb NOT NULL,
  received_at timestamptz NOT NULL DEFAULT now(),
  applied_at timestamptz
);
CREATE INDEX workos_events_received ON workos.events (received_at DESC);

CREATE TABLE workos.sync_cursor (
  stream text PRIMARY KEY,
  after_event_id text,
  updated_at timestamptz NOT NULL DEFAULT now()
);

-- Outbox for WorkOS user deletion after 30-day auth.purge_due() tombstones the acct_ row.
CREATE TABLE workos.pending_deletes (
  workos_user_id text PRIMARY KEY,
  account text NOT NULL,
  queued_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz
);

-- Resolves or provisions an OpenAgents acct_ for a WorkOS user.
-- Auto-links to an existing acct_ ONLY when email_verified = true and email_lookups matches auth.emails.
CREATE FUNCTION workos.account_resolve(
  wos_user_id text,
  email_lookups text[],
  primary_lookup text,
  email_verified boolean,
  is_team boolean,
  provider_ text,
  org_id_ text
) RETURNS TABLE (acct text, made boolean, auto_linked boolean, email_id bigint) LANGUAGE plpgsql AS $$
DECLARE
  a text;
  eid bigint;
BEGIN
  -- 1. Existing link by workos_user_id (skip deleted/merged tombstones).
  SELECT wi.account INTO a
    FROM auth.workos_identities wi
    JOIN auth.accounts acc ON acc.id = wi.account
   WHERE wi.workos_user_id = wos_user_id
     AND acc.deleted_at IS NULL
     AND acc.merged_into IS NULL
   LIMIT 1;
  IF a IS NOT NULL THEN
    UPDATE auth.workos_identities
       SET last_seen_at = now(),
           provider = coalesce(nullif(provider_, ''), provider),
           last_organization_id = coalesce(nullif(org_id_, ''), last_organization_id)
     WHERE workos_user_id = wos_user_id;
    RETURN QUERY SELECT a, false, false, NULL::bigint;
    RETURN;
  END IF;

  -- 2. Auto-link by verified email ONLY when email_verified = true.
  IF email_verified AND email_lookups IS NOT NULL AND cardinality(email_lookups) > 0 THEN
    SELECT e.account, e.id INTO a, eid
      FROM auth.emails e
      JOIN auth.accounts acc ON acc.id = e.account
     WHERE e.lookup = ANY(email_lookups)
       AND acc.deleted_at IS NULL
       AND acc.merged_into IS NULL
     ORDER BY e.is_primary DESC, e.id ASC
     LIMIT 1;
    IF a IS NOT NULL THEN
      INSERT INTO auth.workos_identities (workos_user_id, account, provider, last_organization_id)
      VALUES (wos_user_id, a, nullif(provider_, ''), nullif(org_id_, ''))
      ON CONFLICT (workos_user_id) DO UPDATE
        SET account = EXCLUDED.account,
            provider = coalesce(EXCLUDED.provider, auth.workos_identities.provider),
            last_organization_id = coalesce(EXCLUDED.last_organization_id, auth.workos_identities.last_organization_id),
            last_seen_at = now();
      RETURN QUERY SELECT a, false, true, eid;
      RETURN;
    END IF;
  END IF;

  -- 3. Provision a brand-new acct_ (for new users or unverified emails — never merges on unverified email).
  a := auth.new_account_id();
  INSERT INTO auth.accounts (id, kind, team, verse_account)
  VALUES (a, 'person', coalesce(is_team, false), a);

  eid := NULL;
  IF email_verified AND nullif(primary_lookup, '') IS NOT NULL THEN
    INSERT INTO auth.emails (account, lookup, sealed, is_primary, verified_at)
    VALUES (a, primary_lookup, '', true, now())
    RETURNING id INTO eid;
  END IF;

  INSERT INTO auth.workos_identities (workos_user_id, account, provider, last_organization_id)
  VALUES (wos_user_id, a, nullif(provider_, ''), nullif(org_id_, ''))
  ON CONFLICT (workos_user_id) DO UPDATE
    SET account = EXCLUDED.account,
        provider = coalesce(EXCLUDED.provider, auth.workos_identities.provider),
        last_organization_id = coalesce(EXCLUDED.last_organization_id, auth.workos_identities.last_organization_id),
        last_seen_at = now();

  RETURN QUERY SELECT a, true, false, eid;
END $$;

-- Idempotent event ingestion for both webhooks and Events API polling.
-- Returns true when the event was newly inserted and applied, false on duplicate event id.
CREATE FUNCTION workos.upsert_event(ev_id text, ev_type text, raw jsonb) RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE
  inserted boolean := false;
  d jsonb := raw->'data';
  eid text := coalesce(d->>'id', '');
BEGIN
  INSERT INTO workos.events (id, event, _raw)
  VALUES (ev_id, ev_type, raw)
  ON CONFLICT (id) DO NOTHING;
  GET DIAGNOSTICS inserted = ROW_COUNT;
  IF NOT inserted THEN
    RETURN false;
  END IF;

  IF d IS NOT NULL AND eid <> '' THEN
    IF ev_type IN ('user.created', 'user.updated') THEN
      INSERT INTO workos.users (id, _raw, updated_at, deleted_at)
      VALUES (eid, d, now(), NULL)
      ON CONFLICT (id) DO UPDATE SET _raw = EXCLUDED._raw, updated_at = now(), deleted_at = NULL;
    ELSIF ev_type = 'user.deleted' THEN
      INSERT INTO workos.users (id, _raw, updated_at, deleted_at)
      VALUES (eid, d, now(), now())
      ON CONFLICT (id) DO UPDATE SET _raw = EXCLUDED._raw, updated_at = now(), deleted_at = coalesce(workos.users.deleted_at, now());
    ELSIF ev_type IN ('organization.created', 'organization.updated') THEN
      INSERT INTO workos.organizations (id, _raw, updated_at, deleted_at)
      VALUES (eid, d, now(), NULL)
      ON CONFLICT (id) DO UPDATE SET _raw = EXCLUDED._raw, updated_at = now(), deleted_at = NULL;
    ELSIF ev_type = 'organization.deleted' THEN
      INSERT INTO workos.organizations (id, _raw, updated_at, deleted_at)
      VALUES (eid, d, now(), now())
      ON CONFLICT (id) DO UPDATE SET _raw = EXCLUDED._raw, updated_at = now(), deleted_at = coalesce(workos.organizations.deleted_at, now());
    ELSIF ev_type IN ('organization_membership.created', 'organization_membership.updated') THEN
      INSERT INTO workos.organization_memberships (id, _raw, updated_at, deleted_at)
      VALUES (eid, d, now(), NULL)
      ON CONFLICT (id) DO UPDATE SET _raw = EXCLUDED._raw, updated_at = now(), deleted_at = NULL;
    ELSIF ev_type = 'organization_membership.deleted' THEN
      INSERT INTO workos.organization_memberships (id, _raw, updated_at, deleted_at)
      VALUES (eid, d, now(), now())
      ON CONFLICT (id) DO UPDATE SET _raw = EXCLUDED._raw, updated_at = now(), deleted_at = coalesce(workos.organization_memberships.deleted_at, now());
    ELSIF ev_type = 'session.created' THEN
      INSERT INTO workos.sessions (id, _raw, updated_at, deleted_at)
      VALUES (eid, d, now(), NULL)
      ON CONFLICT (id) DO UPDATE SET _raw = EXCLUDED._raw, updated_at = now(), deleted_at = NULL;
    ELSIF ev_type = 'session.revoked' THEN
      INSERT INTO workos.sessions (id, _raw, updated_at, deleted_at)
      VALUES (eid, d, now(), now())
      ON CONFLICT (id) DO UPDATE SET _raw = EXCLUDED._raw, updated_at = now(), deleted_at = coalesce(workos.sessions.deleted_at, now());
    END IF;
  END IF;

  UPDATE workos.events SET applied_at = now() WHERE id = ev_id;
  RETURN true;
END $$;

-- Captures WorkOS user IDs for accounts about to be purged by auth.purge_due() (30-day lifecycle).
CREATE FUNCTION workos.queue_due_purges() RETURNS int LANGUAGE plpgsql AS $$
DECLARE n int := 0;
BEGIN
  INSERT INTO workos.pending_deletes (workos_user_id, account)
  SELECT wi.workos_user_id, wi.account
    FROM auth.workos_identities wi
    JOIN auth.accounts a ON a.id = wi.account
   WHERE a.deleted_at IS NULL
     AND a.purge_at IS NOT NULL
     AND a.purge_at <= now()
  ON CONFLICT (workos_user_id) DO NOTHING;
  GET DIAGNOSTICS n = ROW_COUNT;
  RETURN n;
END $$;

-- Ensure existing runtime and reader roles can access the new workos schema immediately.
DO $$
DECLARE r text;
BEGIN
  FOREACH r IN ARRAY ARRAY['oa_api', 'oa_api_staging'] LOOP
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = r) THEN
      EXECUTE format('GRANT USAGE ON SCHEMA workos TO %I', r);
      EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA workos TO %I', r);
      EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON auth.workos_identities, auth.workos_pending TO %I', r);
    END IF;
  END LOOP;
  FOREACH r IN ARRAY ARRAY['oa_dump', 'oa_readonly'] LOOP
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = r) THEN
      EXECUTE format('GRANT USAGE ON SCHEMA workos TO %I', r);
      EXECUTE format('GRANT SELECT ON ALL TABLES IN SCHEMA workos TO %I', r);
      EXECUTE format('GRANT SELECT ON auth.workos_identities, auth.workos_pending TO %I', r);
    END IF;
  END LOOP;
END $$;

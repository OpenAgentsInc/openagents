-- Migration 1: the identity and workspace domains sign-in needs, and the
-- audit revisions they write. docs/data/schema.md is the design.
--
-- Each table keeps the store's own JSON record (`record`) beside the
-- columns its constraints and lookups need, which Postgres generates from
-- the record. Constraints between rows are deferred to commit, because a
-- store's writer changes several rows in one transaction in whatever
-- order its document lists them.

CREATE SCHEMA IF NOT EXISTS identity;
CREATE SCHEMA IF NOT EXISTS workspace;
CREATE SCHEMA IF NOT EXISTS audit;

-- One row per document store kept here: what of the document has no
-- table yet, and a revision counter readers cache against.
CREATE TABLE identity.stores (
    store text PRIMARY KEY,
    revision bigint NOT NULL,
    rest jsonb NOT NULL,
    schema text GENERATED ALWAYS AS (rest ->> 'v') STORED,
    sequence bigint GENERATED ALWAYS AS ((rest ->> 'sequence')::bigint) STORED,
    digest text GENERATED ALWAYS AS (rest ->> 'digest') STORED,
    updated_at timestamptz NOT NULL DEFAULT now()
);

-- identity -----------------------------------------------------------------

CREATE TABLE identity.accounts (
    id text PRIMARY KEY,
    record jsonb NOT NULL,
    label text GENERATED ALWAYS AS (record ->> 'label') STORED,
    created text GENERATED ALWAYS AS (record ->> 'created') STORED,
    CONSTRAINT accounts_record_id CHECK (record ->> 'id' = id)
);

-- One principal names one account. Kept from accounts.record.principals by
-- the trigger below.
CREATE TABLE identity.principals (
    principal text NOT NULL,
    account_id text NOT NULL,
    kind text GENERATED ALWAYS AS (split_part(principal, ':', 1)) STORED,
    CONSTRAINT principals_one_account UNIQUE (principal) DEFERRABLE INITIALLY DEFERRED,
    CONSTRAINT principals_account FOREIGN KEY (account_id)
        REFERENCES identity.accounts (id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX principals_by_account ON identity.principals (account_id);

CREATE FUNCTION identity.keep_principals() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    DELETE FROM identity.principals WHERE account_id = NEW.id;
    INSERT INTO identity.principals (principal, account_id)
    SELECT p, NEW.id
    FROM jsonb_array_elements_text(coalesce(NEW.record -> 'principals', '[]'::jsonb)) AS p;
    RETURN NULL;
END $$;

CREATE TRIGGER accounts_keep_principals
    AFTER INSERT OR UPDATE OF record ON identity.accounts
    FOR EACH ROW EXECUTE FUNCTION identity.keep_principals();

-- The profile a provider returned for a principal (GitHub today).
CREATE TABLE identity.linked_identities (
    provider text NOT NULL,
    provider_id text NOT NULL,
    record jsonb NOT NULL,
    account_id text GENERATED ALWAYS AS (record ->> 'account') STORED,
    login text GENERATED ALWAYS AS (record -> 'profile' ->> 'login') STORED,
    PRIMARY KEY (provider, provider_id),
    CONSTRAINT linked_identities_account FOREIGN KEY (account_id)
        REFERENCES identity.accounts (id) DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX linked_identities_by_account ON identity.linked_identities (account_id);

CREATE TABLE identity.account_sessions (
    id text PRIMARY KEY,
    record jsonb NOT NULL,
    account_id text GENERATED ALWAYS AS (record ->> 'user') STORED,
    kind text GENERATED ALWAYS AS (coalesce(record ->> 'kind', 'user')) STORED,
    state text GENERATED ALWAYS AS (record ->> 'state') STORED,
    created_at bigint GENERATED ALWAYS AS ((record ->> 'created_at')::bigint) STORED,
    expires_at bigint GENERATED ALWAYS AS ((record ->> 'expires_at')::bigint) STORED,
    CONSTRAINT account_sessions_record_id CHECK (record ->> 'id' = id)
);
CREATE INDEX account_sessions_by_account ON identity.account_sessions (account_id);

CREATE TABLE identity.device_sign_ins (
    id text PRIMARY KEY,
    record jsonb NOT NULL,
    user_code text GENERATED ALWAYS AS (record ->> 'user_code') STORED,
    state text GENERATED ALWAYS AS (record ->> 'state') STORED,
    account_id text GENERATED ALWAYS AS (record ->> 'account') STORED,
    CONSTRAINT device_sign_ins_user_code UNIQUE (user_code) DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE identity.recoveries (
    id text PRIMARY KEY,
    record jsonb NOT NULL,
    account_id text GENERATED ALWAYS AS (record ->> 'user') STORED,
    state text GENERATED ALWAYS AS (record ->> 'state') STORED
);

CREATE TABLE identity.credentials (
    account_id text PRIMARY KEY,
    record jsonb NOT NULL
);

CREATE TABLE identity.onboarding_budgets (
    id text PRIMARY KEY,
    record jsonb NOT NULL
);

-- oak_ bearer keys: the secret's digest only. A key's account is the one
-- holding the principal `key:<id>`.
CREATE TABLE identity.bearer_keys (
    id text PRIMARY KEY,
    record jsonb NOT NULL,
    tenant text GENERATED ALWAYS AS (record ->> 'tenant') STORED,
    digest text GENERATED ALWAYS AS (record ->> 'digest') STORED,
    status text GENERATED ALWAYS AS (record ->> 'status') STORED,
    name text GENERATED ALWAYS AS (record ->> 'name') STORED,
    CONSTRAINT bearer_keys_record_id CHECK (record ->> 'id' = id),
    CONSTRAINT bearer_keys_digest UNIQUE (digest) DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX bearer_keys_by_tenant ON identity.bearer_keys (tenant);

-- A workspace's own provider keys, sealed under the BYOK keyring.
CREATE TABLE identity.provider_keys (
    tenant text NOT NULL,
    provider text NOT NULL,
    record jsonb NOT NULL,
    added_at bigint GENERATED ALWAYS AS ((record ->> 'added_at')::bigint) STORED,
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant, provider)
);

-- An account's GitHub access: the sealed user token, App installations,
-- broker tickets and chosen repositories.
CREATE TABLE identity.github_access (
    account_id text PRIMARY KEY,
    account_digest text NOT NULL UNIQUE,
    record jsonb NOT NULL,
    github_id bigint GENERATED ALWAYS AS (
        coalesce((record -> 'grant' ->> 'github_id')::bigint, (record -> 'app' ->> 'github_id')::bigint)
    ) STORED,
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT github_access_record_account CHECK (record ->> 'account' = account_id)
);

-- workspace ----------------------------------------------------------------

CREATE TABLE workspace.workspaces (
    id text PRIMARY KEY,
    record jsonb NOT NULL,
    kind text GENERATED ALWAYS AS (record ->> 'kind') STORED,
    name text GENERATED ALWAYS AS (record ->> 'name') STORED,
    tenant text GENERATED ALWAYS AS (record ->> 'tenant') STORED,
    CONSTRAINT workspaces_record_id CHECK (record ->> 'id' = id),
    CONSTRAINT workspaces_kind CHECK (record ->> 'kind' IN ('personal', 'organization'))
);
CREATE INDEX workspaces_by_tenant ON workspace.workspaces (tenant);

CREATE TABLE workspace.memberships (
    workspace_id text NOT NULL,
    account_id text NOT NULL,
    record jsonb NOT NULL,
    role text GENERATED ALWAYS AS (record ->> 'role') STORED,
    status text GENERATED ALWAYS AS (record ->> 'status') STORED,
    epoch bigint GENERATED ALWAYS AS ((record ->> 'epoch')::bigint) STORED,
    PRIMARY KEY (workspace_id, account_id),
    CONSTRAINT memberships_record_account CHECK (record ->> 'account' = account_id),
    CONSTRAINT memberships_workspace FOREIGN KEY (workspace_id)
        REFERENCES workspace.workspaces (id) DEFERRABLE INITIALLY DEFERRED,
    CONSTRAINT memberships_account FOREIGN KEY (account_id)
        REFERENCES identity.accounts (id) DEFERRABLE INITIALLY DEFERRED,
    -- Exactly one active owner at a time: at most one here, at least one
    -- checked by the store.
    CONSTRAINT memberships_one_owner EXCLUDE USING btree (workspace_id WITH =)
        WHERE (role = 'owner' AND status = 'active') DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX memberships_by_account ON workspace.memberships (account_id);

CREATE TABLE workspace.invitations (
    id text PRIMARY KEY,
    record jsonb NOT NULL,
    workspace_id text GENERATED ALWAYS AS (record ->> 'workspace') STORED,
    digest text GENERATED ALWAYS AS (record ->> 'digest') STORED,
    status text GENERATED ALWAYS AS (record ->> 'status') STORED,
    expires_unix bigint GENERATED ALWAYS AS ((record ->> 'expires_unix')::bigint) STORED,
    CONSTRAINT invitations_record_id CHECK (record ->> 'id' = id),
    CONSTRAINT invitations_digest UNIQUE (digest) DEFERRABLE INITIALLY DEFERRED,
    CONSTRAINT invitations_workspace FOREIGN KEY (workspace_id)
        REFERENCES workspace.workspaces (id) DEFERRABLE INITIALLY DEFERRED
);

-- audit --------------------------------------------------------------------

-- Every sealed revision of a document store, by digest. Append-only.
CREATE TABLE audit.revisions (
    store text NOT NULL,
    digest text NOT NULL,
    sequence bigint,
    supersedes text,
    document jsonb NOT NULL,
    written_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (store, digest)
);

CREATE FUNCTION audit.append_only() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION '%.% is append-only', TG_TABLE_SCHEMA, TG_TABLE_NAME;
END $$;

CREATE TRIGGER revisions_append_only
    BEFORE UPDATE OR DELETE ON audit.revisions
    FOR EACH ROW EXECUTE FUNCTION audit.append_only();
CREATE TRIGGER revisions_no_truncate
    BEFORE TRUNCATE ON audit.revisions
    FOR EACH STATEMENT EXECUTE FUNCTION audit.append_only();

-- Migration 4 (#12320): links between our accounts and WorkOS.
--
-- WorkOS holds a User for each person and an Organization for each shared
-- workspace (docs/auth/2026-10-20-workos-source-of-truth.md). These two
-- tables join their IDs to ours. The account and workspace IDs never
-- change; every other store keeps keying on them. Nothing in the account
-- or workspace documents changes, so a binary from before this migration
-- still opens the stores and simply ignores these tables.

CREATE TABLE identity.workos_users (
    account_id text PRIMARY KEY,
    workos_user_id text NOT NULL,
    linked_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT workos_users_user UNIQUE (workos_user_id),
    CONSTRAINT workos_users_account_shape CHECK (account_id ~ '^acct_[0-9a-f]{16}$'),
    CONSTRAINT workos_users_user_shape CHECK (workos_user_id ~ '^user_[0-9A-Za-z]{1,64}$')
);

CREATE TABLE workspace.workos_orgs (
    workspace_id text PRIMARY KEY,
    workos_org_id text NOT NULL,
    linked_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT workos_orgs_org UNIQUE (workos_org_id),
    CONSTRAINT workos_orgs_org_shape CHECK (workos_org_id ~ '^org_[0-9A-Za-z]{1,64}$')
);

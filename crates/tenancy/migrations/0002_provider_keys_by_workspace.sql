-- Migration 2 (#11186): a provider key belongs to a workspace.
--
-- Migration 1 kept the gateway's own provider keys by registry tenant, and
-- every personal workspace made by sign-up shares one tenant (`signup`), so
-- one person's saved key replaced everyone's and paid for other accounts'
-- calls. Keys are now kept by `workspace_id` (docs/data/schema.md, rule 1),
-- sealed with the workspace as associated data.
--
-- The rows kept by tenant move to `identity.provider_keys_by_tenant`,
-- which nothing reads for a call or a listing. The gateway re-seals a row
-- into its workspace at start when the row's tenant holds exactly one
-- workspace, or when an operator has named the workspace that saved it
-- (`tenant-db assign-provider-key`, which sets `workspace_id`). A row on a
-- shared tenant that nobody names stays unused until it is removed.

ALTER TABLE identity.provider_keys RENAME TO provider_keys_by_tenant;
ALTER INDEX identity.provider_keys_pkey RENAME TO provider_keys_by_tenant_pkey;
ALTER TABLE identity.provider_keys_by_tenant ADD COLUMN workspace_id text;

CREATE TABLE identity.provider_keys (
    workspace_id text NOT NULL,
    provider text NOT NULL,
    record jsonb NOT NULL,
    added_at bigint GENERATED ALWAYS AS ((record ->> 'added_at')::bigint) STORED,
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, provider)
);

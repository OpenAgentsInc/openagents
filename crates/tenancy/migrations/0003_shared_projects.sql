-- Migration 3 (#11585): projects get their own rows and members.
--
-- Until now an account's projects lived inside its GitHub-access record
-- (`identity.github_access.record -> 'projects'`, at most 200), so a
-- project could only ever belong to one account. A project is now a row
-- of its own (`identity.projects`), and who may use it is
-- `identity.project_members`: exactly one owner, and any number of
-- editors and viewers. Single-use invitations keep only the secret's
-- digest (`invitations`, as `workspace.invitations` does).
--
-- `record` is the project exactly as the GitHub-access record kept it
-- (the older one-repository shape included; the reader converts it), so
-- every existing project reads back unchanged. `home_account` is the
-- account whose storage holds the project's files, notes, and Drive
-- sources: the account that made it. It does not change when ownership
-- moves.
--
-- Every project kept today becomes a row owned by its account, with no
-- other members, in the order the account kept them. The copy in
-- `identity.github_access.record -> 'projects'` stays where it is, read by
-- nothing from this version on, so a rollback to the previous version
-- still finds the projects it knew; a later migration drops it.

CREATE TABLE identity.projects (
    id text PRIMARY KEY,
    home_account text NOT NULL,
    record jsonb NOT NULL,
    invitations jsonb NOT NULL DEFAULT '[]'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT projects_record_id CHECK (record ->> 'id' = id),
    CONSTRAINT projects_id_shape CHECK (id ~ '^prj_[0-9a-f]{16}$'),
    CONSTRAINT projects_invitations_array CHECK (jsonb_typeof(invitations) = 'array')
);

CREATE TABLE identity.project_members (
    project_id text NOT NULL REFERENCES identity.projects (id) ON DELETE CASCADE,
    account_id text NOT NULL,
    role text NOT NULL,
    joined_unix bigint NOT NULL,
    -- The order an account's projects list in: the order it got them.
    position bigserial NOT NULL,
    PRIMARY KEY (project_id, account_id),
    CONSTRAINT project_members_role CHECK (role IN ('owner', 'editor', 'viewer'))
);

CREATE UNIQUE INDEX project_members_one_owner
    ON identity.project_members (project_id) WHERE role = 'owner';
CREATE INDEX project_members_by_account
    ON identity.project_members (account_id, position);

-- The copy. A project id is random (64 bits), so two accounts never hold
-- the same one; should it ever happen, the first account (by id) keeps it
-- and the dry run (`tenant-db projects-dry-run`) reports it.
INSERT INTO identity.projects (id, home_account, record)
SELECT DISTINCT ON (kept.value ->> 'id')
       kept.value ->> 'id', access.account_id, kept.value
  FROM identity.github_access AS access
 CROSS JOIN LATERAL jsonb_array_elements(
           CASE WHEN jsonb_typeof(access.record -> 'projects') = 'array'
                THEN access.record -> 'projects' ELSE '[]'::jsonb END
       ) WITH ORDINALITY AS kept (value, n)
 WHERE (kept.value ->> 'id') ~ '^prj_[0-9a-f]{16}$'
 ORDER BY kept.value ->> 'id', access.account_id;

INSERT INTO identity.project_members (project_id, account_id, role, joined_unix)
SELECT project.id, project.home_account, 'owner',
       coalesce((kept.value ->> 'created_unix')::bigint, 0)
  FROM identity.github_access AS access
 CROSS JOIN LATERAL jsonb_array_elements(
           CASE WHEN jsonb_typeof(access.record -> 'projects') = 'array'
                THEN access.record -> 'projects' ELSE '[]'::jsonb END
       ) WITH ORDINALITY AS kept (value, n)
  JOIN identity.projects AS project
    ON project.id = kept.value ->> 'id' AND project.home_account = access.account_id
 ORDER BY access.account_id, kept.n;

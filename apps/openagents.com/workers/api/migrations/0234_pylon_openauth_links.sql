ALTER TABLE agent_credentials
  ADD COLUMN openauth_user_id TEXT REFERENCES users(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS agent_credentials_openauth_user_idx
  ON agent_credentials(openauth_user_id, status, revoked_at);

CREATE TABLE IF NOT EXISTS openauth_agent_links (
  id TEXT PRIMARY KEY,
  openauth_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  agent_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  agent_credential_id TEXT REFERENCES agent_credentials(id) ON DELETE SET NULL,
  link_kind TEXT NOT NULL CHECK (
    link_kind IN ('claim_approval', 'credential_anchor', 'manual')
  ),
  status TEXT NOT NULL CHECK (status IN ('active', 'revoked')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  revoked_at TEXT,
  UNIQUE(openauth_user_id, agent_user_id, agent_credential_id)
);

CREATE INDEX IF NOT EXISTS openauth_agent_links_owner_status_idx
  ON openauth_agent_links(openauth_user_id, status, updated_at);

CREATE INDEX IF NOT EXISTS openauth_agent_links_agent_status_idx
  ON openauth_agent_links(agent_user_id, status, updated_at);

INSERT OR IGNORE INTO openauth_agent_links (
  id,
  openauth_user_id,
  agent_user_id,
  agent_credential_id,
  link_kind,
  status,
  created_at,
  updated_at,
  revoked_at
)
SELECT
  'openauth_agent_link_' || agent_owner_claims.id,
  agent_owner_claims.owner_user_id,
  agent_owner_claims.agent_user_id,
  agent_owner_claims.credential_id,
  'claim_approval',
  'active',
  COALESCE(agent_owner_claims.decided_at, agent_owner_claims.updated_at),
  COALESCE(agent_owner_claims.decided_at, agent_owner_claims.updated_at),
  NULL
FROM agent_owner_claims
WHERE agent_owner_claims.status = 'approved'
  AND agent_owner_claims.owner_user_id IS NOT NULL
  AND agent_owner_claims.agent_user_id IS NOT NULL;

UPDATE agent_credentials
   SET openauth_user_id = (
     SELECT agent_owner_claims.owner_user_id
       FROM agent_owner_claims
      WHERE agent_owner_claims.credential_id = agent_credentials.id
        AND agent_owner_claims.status = 'approved'
        AND agent_owner_claims.owner_user_id IS NOT NULL
      ORDER BY agent_owner_claims.decided_at DESC
      LIMIT 1
   )
 WHERE openauth_user_id IS NULL
   AND EXISTS (
     SELECT 1
       FROM agent_owner_claims
      WHERE agent_owner_claims.credential_id = agent_credentials.id
        AND agent_owner_claims.status = 'approved'
        AND agent_owner_claims.owner_user_id IS NOT NULL
   );

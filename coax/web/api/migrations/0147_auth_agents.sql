-- Agent accounts, their grants and keys (docs/auth/identity-and-methods.md#agents, #13211).
--
-- An agent is an auth.accounts row with kind 'agent' and a person as its owner (0142's check and
-- trigger). It proves who it is with its own Nostr key (an auth.nostr_keys row on the agent's
-- account, labelled 'agent') signing each request with NIP-98, or with an oa_key_ API key
-- (auth.agent_keys, its SHA-256 only). Never its owner's key, email or session.
--
-- Its grant (auth.agent_grants, one per agent) is what its owner approved at L2: the scopes as bits
-- (coax/web/api/agent-policy.coax), the spend caps in sats, the addresses it may call from and when
-- it ends (at most a year). `record` keeps the approved openagents.account.v1alpha1.GrantRequest's
-- exact bytes. Whether a grant is active now, and what it allows, is decided in agent-policy.coax
-- from these columns; the rows only hold facts. Revoking sets revoked_at and takes effect on the
-- agent's next request. Every change and every use is in auth.audit.
--
-- An agent asking its owner for a grant (auth.agent_requests): signed by the agent's new key, it
-- holds the GrantRequest and an 8-character code the owner opens at /settings/agents#request=.
-- The CLI's `openagents login` keeps 0098's auth.device_logins; auth.device_link (below) links its
-- key to the person's account in auth.nostr_keys as cli:<machine>.
--
-- auth.agent_spends is the ledger of what each grant spent, one row per payment id, so asking
-- twice for one payment counts once. It holds no memo and no destination.
--
-- Roles (db/grants.sql, #13194): the API's role gets rows through the default privileges; the
-- dump and read-only roles read them.

CREATE TABLE auth.agent_grants (
  id text PRIMARY KEY CHECK (id ~ '^grt_[a-z2-7]{26}$'),
  agent text NOT NULL UNIQUE REFERENCES auth.accounts (id),
  owner text NOT NULL REFERENCES auth.accounts (id),
  scopes int NOT NULL CHECK (scopes > 0),
  sats_per_day bigint NOT NULL DEFAULT 0 CHECK (sats_per_day >= 0),
  sats_per_payment bigint NOT NULL DEFAULT 0 CHECK (sats_per_payment >= 0 AND sats_per_payment <= sats_per_day),
  hosts text[] NOT NULL DEFAULT '{}' CHECK (cardinality(hosts) <= 8),
  record bytea NOT NULL CHECK (length(record) <= 2048),
  created_at timestamptz NOT NULL DEFAULT now(),
  approved_at timestamptz NOT NULL DEFAULT now(),
  expires_at timestamptz NOT NULL,
  revoked_at timestamptz,
  suspended_at timestamptz,              -- the owner paused every agent ("log out everywhere" with agents)
  last_used_at timestamptz,
  CHECK (expires_at > approved_at AND expires_at <= approved_at + interval '365 days'),
  CHECK (agent <> owner)
);
CREATE INDEX agent_grants_owner ON auth.agent_grants (owner);
-- The agent's account is an agent whose owner is the grant's owner.
CREATE FUNCTION auth.grant_matches_agent() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM auth.accounts WHERE id = NEW.agent AND kind = 'agent' AND owner = NEW.owner) THEN
    RAISE EXCEPTION 'auth: a grant''s agent is an agent of its owner' USING ERRCODE = 'check_violation';
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER grant_matches_agent BEFORE INSERT OR UPDATE OF agent, owner ON auth.agent_grants FOR EACH ROW EXECUTE FUNCTION auth.grant_matches_agent();

CREATE TABLE auth.agent_keys (
  id text PRIMARY KEY CHECK (id ~ '^[A-Z2-7]{8}$'),   -- the oa_key_ id people see in lists
  agent text NOT NULL REFERENCES auth.accounts (id),
  token_sha256 text NOT NULL UNIQUE CHECK (token_sha256 ~ '^[0-9a-f]{64}$'),
  created_at timestamptz NOT NULL DEFAULT now(),
  last_used_at timestamptz,
  revoked_at timestamptz
);
CREATE INDEX agent_keys_agent ON auth.agent_keys (agent);

CREATE TABLE auth.agent_spends (
  grant_id text NOT NULL REFERENCES auth.agent_grants (id) ON DELETE CASCADE,
  payment_id text NOT NULL CHECK (payment_id ~ '^[A-Za-z0-9_.:-]{1,64}$'),
  sats bigint NOT NULL CHECK (sats > 0),
  at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (grant_id, payment_id)
);
CREATE INDEX agent_spends_at ON auth.agent_spends (grant_id, at);

CREATE TABLE auth.agent_requests (
  code text PRIMARY KEY CHECK (code ~ '^[A-Z2-7]{8}$'),
  pubkey text NOT NULL CHECK (pubkey ~ '^[0-9a-f]{64}$'),   -- the agent's new key, which signed it
  request bytea NOT NULL CHECK (length(request) <= 2048),  -- the GrantRequest it asked for
  created_at timestamptz NOT NULL DEFAULT now(),
  expires_at timestamptz NOT NULL,
  owner text REFERENCES auth.accounts (id),       -- who answered it
  agent text REFERENCES auth.accounts (id),       -- the agent account an approval made
  done_at timestamptz,
  result text CHECK (result IN ('approved', 'denied'))
);
CREATE INDEX agent_requests_pubkey ON auth.agent_requests (pubkey);
CREATE INDEX agent_requests_expires ON auth.agent_requests (expires_at);

-- What an agent spent in the last 24 hours.
CREATE FUNCTION auth.spent_24h(grant_ text) RETURNS bigint LANGUAGE sql STABLE AS $$
  SELECT coalesce(sum(sats), 0)::bigint FROM auth.agent_spends WHERE grant_id = grant_ AND at > now() - interval '24 hours'
$$;

-- A new agent account for owner, playing as itself. Its id.
CREATE FUNCTION auth.new_agent(owner_ text, name_ text) RETURNS text LANGUAGE plpgsql AS $$
DECLARE id_ text := auth.new_account_id();
BEGIN
  INSERT INTO verse.accounts (id, created_ms) VALUES (id_, verse.now_ms()) ON CONFLICT DO NOTHING;
  INSERT INTO auth.accounts (id, kind, owner, display_name, team, verse_account)
  SELECT id_, 'agent', a.id, left(name_, 80), a.team, id_ FROM auth.accounts a WHERE a.id = owner_ AND a.kind = 'person' AND a.deleted_at IS NULL;
  IF NOT FOUND THEN RAISE EXCEPTION 'auth: no such owner' USING ERRCODE = 'check_violation'; END IF;
  RETURN id_;
END $$;

-- The facts agent-policy.coax decides an agent's credential on, by its account: the grant, its
-- clock and caps, and whether it is paused (by its owner, or a frozen or deleted account).
CREATE FUNCTION auth.agent_facts(acct text) RETURNS TABLE (grant_id text, owner text, scopes int, per_day bigint, per_payment bigint,
  hosts text[], expires_ms bigint, revoked_ms bigint, suspended boolean, now_ms bigint) LANGUAGE sql STABLE AS $$
  SELECT g.id, g.owner, g.scopes, g.sats_per_day, g.sats_per_payment, g.hosts,
         (extract(epoch FROM g.expires_at) * 1000)::bigint,
         coalesce((extract(epoch FROM g.revoked_at) * 1000)::bigint, 0),
         g.suspended_at IS NOT NULL OR a.frozen_at IS NOT NULL OR a.deleted_at IS NOT NULL
           OR o.frozen_at IS NOT NULL OR o.deleted_at IS NOT NULL,
         (extract(epoch FROM now()) * 1000)::bigint
    FROM auth.agent_grants g JOIN auth.accounts a ON a.id = g.agent JOIN auth.accounts o ON o.id = g.owner
   WHERE g.agent = acct
$$;

-- An owner's agents as rows for GET /api/agents (one: only that agent, for GET /api/agent/me), the
-- revoked ones left out (the audit log keeps them; at most 32 others, agent-policy.coax): the
-- grant's facts, the approved GrantRequest's bytes (hex; its name, key and attestation) and the
-- agent's keys as lines "id api created_ms last_used_ms revoked_ms" (api 1: an oa_key_, 0: a Nostr key).
CREATE FUNCTION auth.agents_of(owner_ text, one text) RETURNS TABLE (account text, name text, grant_id text, scopes int, per_day bigint,
  per_payment bigint, hosts text, created_ms bigint, expires_ms bigint, revoked_ms bigint, suspended boolean,
  spent bigint, last_used_ms bigint, now_ms bigint, record text, keys text) LANGUAGE sql STABLE AS $$
  SELECT a.id, coalesce(a.display_name, ''), g.id, g.scopes, g.sats_per_day, g.sats_per_payment, array_to_string(g.hosts, ' '),
         (extract(epoch FROM g.approved_at) * 1000)::bigint, (extract(epoch FROM g.expires_at) * 1000)::bigint,
         coalesce((extract(epoch FROM g.revoked_at) * 1000)::bigint, 0),
         g.suspended_at IS NOT NULL OR a.frozen_at IS NOT NULL OR o.frozen_at IS NOT NULL,
         auth.spent_24h(g.id), coalesce((extract(epoch FROM g.last_used_at) * 1000)::bigint, 0),
         (extract(epoch FROM now()) * 1000)::bigint, encode(g.record, 'hex'),
         coalesce((SELECT string_agg(k, E'\n' ORDER BY k) FROM (
             SELECT concat_ws(' ', ak.id, 1, (extract(epoch FROM ak.created_at) * 1000)::bigint,
                              coalesce((extract(epoch FROM ak.last_used_at) * 1000)::bigint, 0),
                              coalesce((extract(epoch FROM ak.revoked_at) * 1000)::bigint, 0)) k
               FROM auth.agent_keys ak WHERE ak.agent = a.id
             UNION ALL
             SELECT concat_ws(' ', nk.pubkey, 0, (extract(epoch FROM nk.created_at) * 1000)::bigint,
                              coalesce((extract(epoch FROM nk.last_used_at) * 1000)::bigint, 0), 0)
               FROM auth.nostr_keys nk WHERE nk.account = a.id) s), '')
    FROM auth.accounts a JOIN auth.agent_grants g ON g.agent = a.id JOIN auth.accounts o ON o.id = a.owner
   WHERE a.owner = owner_ AND a.kind = 'agent' AND a.deleted_at IS NULL AND g.revoked_at IS NULL AND (one IS NULL OR a.id = one)
   ORDER BY g.approved_at DESC
   LIMIT 32
$$;

-- Requests waiting more than a day, and spends older than a week, go (auth-sweep, nightly).
CREATE FUNCTION auth.agents_sweep() RETURNS int LANGUAGE plpgsql AS $$
DECLARE n int; m int;
BEGIN
  DELETE FROM auth.agent_requests WHERE expires_at < now() - interval '1 day';
  GET DIAGNOSTICS n = ROW_COUNT;
  DELETE FROM auth.agent_spends WHERE at < now() - interval '7 days';
  GET DIAGNOSTICS m = ROW_COUNT;
  RETURN n + m;
END $$;

-- Whether an account has a strong way in (a Nostr key or a passkey): then linking another key, or
-- giving an agent anything, needs a login with one of them (agent-policy.coax's ag_link).
CREATE FUNCTION auth.has_strong(acct text) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT EXISTS (SELECT 1 FROM auth.nostr_keys k WHERE k.account = acct AND coalesce(k.label, '') <> 'agent')
      OR EXISTS (SELECT 1 FROM auth.passkeys p WHERE p.account = acct)
$$;

-- `openagents login`: a logged-in person approves the CLI's request (0098's auth.device_logins, by
-- its code); the CLI's key joins acct in auth.nostr_keys as cli:<machine>. Once, before it expires.
-- A key that is already another account's is refused (merging accounts is not built).
-- (status, machine): 200, 404 no such code, 409 approved already or another account's, 410 expired.
CREATE FUNCTION auth.device_link(acct text, code text, OUT status int, OUT machine text) LANGUAGE plpgsql AS $$
DECLARE d auth.device_logins; held text;
BEGIN
  SELECT * INTO d FROM auth.device_logins x WHERE x.code_sha256 = projects.sha(coalesce(code, '')) FOR UPDATE;
  IF d.id IS NULL THEN status := 404; RETURN; END IF;
  machine := d.machine;
  IF d.account IS NOT NULL THEN status := 409; RETURN; END IF;
  IF d.expires_ms <= projects.now_ms() THEN status := 410; RETURN; END IF;
  SELECT k.account INTO held FROM auth.nostr_keys k WHERE k.pubkey = d.pubkey;
  IF held IS NOT NULL AND held <> acct THEN status := 409; RETURN; END IF;
  UPDATE auth.device_logins SET account = acct, approved_ms = projects.now_ms(), approved_by = acct WHERE id = d.id;
  IF held IS NULL THEN
    INSERT INTO auth.nostr_keys (account, pubkey, label) VALUES (acct, d.pubkey, left('cli:' || d.machine, 80));
  END IF;
  status := 200;
END $$;

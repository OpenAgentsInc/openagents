-- Account life: freeze, merge, data export and deletion (docs/auth/identity-and-methods.md#merging-accounts,
-- docs/auth/data-and-backups.md#data-export-and-account-deletion, #13214). What may happen is
-- decided in auth-life-policy.coax (#authority); these rows hold the facts, and the constraints and
-- functions below refuse what no code may do:
--
--   * a deletion is never purged sooner than 30 days after it was asked (purge_waits_30_days)
--   * a purged or merged account's id is a tombstone: the row stays, so the id is never reused
--   * an account never merges into itself, into an agent, or into a tombstone
--   * the audit log keeps every row: a merge leaves both accounts' histories under their own ids,
--     and a purge only clears the account id from its rows (auth.audit_forget, as audit_age does)
--
-- auth.life_columns names every column that holds an account, game account, player, key or realm
-- of a person, and what a merge, a purge and an export do with it. auth.life_unlisted() lists the
-- columns that point at an account (foreign keys to auth.accounts, verse.accounts and
-- verse.players) the registry doesn't cover; test.sh fails while it lists any, so a new table
-- can't be forgotten by deletion or merging.

-- ---- the account's life, on its row
ALTER TABLE auth.accounts
  ADD COLUMN frozen_why text CHECK (frozen_why IN ('freeze', 'delete')),   -- why frozen_at is set
  ADD COLUMN delete_asked_at timestamptz,
  ADD COLUMN purge_at timestamptz;
-- A row already frozen by 0143's freeze has no why yet.
UPDATE auth.accounts SET frozen_why = 'freeze' WHERE frozen_at IS NOT NULL AND frozen_why IS NULL;
ALTER TABLE auth.accounts
  ADD CONSTRAINT purge_waits_30_days CHECK (purge_at IS NULL OR (delete_asked_at IS NOT NULL AND purge_at >= delete_asked_at + interval '30 days')),
  ADD CONSTRAINT frozen_has_why CHECK ((frozen_at IS NULL) = (frozen_why IS NULL)),
  ADD CONSTRAINT deleting_is_frozen CHECK (delete_asked_at IS NULL OR frozen_at IS NOT NULL);
CREATE INDEX accounts_purge ON auth.accounts (purge_at) WHERE purge_at IS NOT NULL AND deleted_at IS NULL;

-- Merges, for the export and the audit: who merged into whom, and what moved.
CREATE TABLE auth.merges (
  id bigserial PRIMARY KEY,
  survivor text NOT NULL REFERENCES auth.accounts (id),
  merged text NOT NULL UNIQUE REFERENCES auth.accounts (id),
  at timestamptz NOT NULL DEFAULT now(),
  moved jsonb NOT NULL DEFAULT '{}',     -- {"auth.emails.account": 1, ...}: rows moved, per column
  kept jsonb NOT NULL DEFAULT '{}',      -- rows that stayed with the tombstone (a save slot both had)
  money boolean NOT NULL,                -- the person confirmed the other account's credit moving too
  CHECK (survivor <> merged)
);
CREATE INDEX merges_survivor ON auth.merges (survivor);

-- A merge waiting for its second proof and its confirmation: the survivor's session asked,
-- the other account proved itself (a key's signature or an email code), the page confirms.
ALTER TABLE auth.pending DROP CONSTRAINT pending_purpose_check;
ALTER TABLE auth.pending ADD CONSTRAINT pending_purpose_check CHECK (purpose IN ('login', 'link', 'stepup', 'finish', 'merge'));
CREATE TABLE auth.merge_asks (
  id text PRIMARY KEY CHECK (id ~ '^[0-9a-f]{32}$'),
  survivor text NOT NULL REFERENCES auth.accounts (id),
  session text NOT NULL,                 -- the survivor's session that asked; only it may confirm
  other text REFERENCES auth.accounts (id),   -- once proved
  proved_via text CHECK (proved_via IN ('email', 'nostr')),
  proved_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT now(),
  expires_at timestamptz NOT NULL,       -- 10 minutes: both proofs are fresh when it runs
  done_at timestamptz,
  CHECK (other IS NULL OR other <> survivor)
);
CREATE INDEX merge_asks_survivor ON auth.merge_asks (survivor);

-- ---- the registry
CREATE TABLE auth.life_columns (
  tbl text NOT NULL,                     -- schema.table
  col text NOT NULL,
  holds text NOT NULL CHECK (holds IN ('account', 'verse', 'player', 'pubkey', 'world')),
  on_merge text NOT NULL CHECK (on_merge IN ('move', 'end', 'keep', 'follows')),
  on_purge text NOT NULL CHECK (on_purge IN ('delete', 'anonymize', 'keep')),
  export boolean NOT NULL,
  omit text[] NOT NULL DEFAULT '{}',     -- columns never exported: digests of secrets, ciphertext
  why text NOT NULL,
  PRIMARY KEY (tbl, col)
);
-- holds:  account  auth.accounts ids: the account (and, for a purge, each of its agents in turn)
--         verse    its game account (auth.accounts.verse_account)
--         player   the game account's players (verse.players.account)
--         pubkey   the account's Nostr keys
--         world    the realms its game account owns (verse.worlds.owner_account)
-- on_merge:  move     the column's value becomes the survivor's (a row that would collide with one
--                     the survivor has stays with the tombstone, and is counted as kept)
--            end      the rows go (sessions, pending logins): the other account's logins end
--            keep     the rows stay with the tombstone (its history)
--            follows  nothing to do: the row follows a key, player or realm that moves
-- on_purge:  delete    the rows go, and every row that points at them (foreign keys, recursively)
--            anonymize the value becomes 'deleted': a record the law or the audit keeps
--            keep      the rows stay (they hold nothing about the person once the rest is gone)
INSERT INTO auth.life_columns (tbl, col, holds, on_merge, on_purge, export, omit, why) VALUES
  -- our accounts
  ('auth.emails', 'account', 'account', 'move', 'delete', true, '{lookup,sealed}', 'ways in: the export carries the address itself'),
  ('auth.nostr_keys', 'account', 'account', 'move', 'delete', true, '{}', 'ways in'),
  ('auth.passkeys', 'account', 'account', 'move', 'delete', true, '{public_key,user_handle,prf_salt}', 'ways in'),
  ('auth.devices', 'account', 'account', 'move', 'delete', true, '{public_key}', 'ways in'),
  ('auth.recovery', 'account', 'account', 'move', 'delete', true, '{code_scrypt,salt}', 'ways in'),
  ('auth.recovery_requests', 'account', 'account', 'keep', 'delete', true, '{cancel_sha256}', 'recovery steps'),
  ('auth.connections', 'account', 'account', 'move', 'delete', true, '{token_sealed}', 'GitHub and Google connections'),
  ('auth.sessions', 'account', 'account', 'end', 'delete', true, '{token_sha256,prev_sha256}', 'logins'),
  ('auth.pending', 'account', 'account', 'end', 'delete', false, '{}', 'logins waiting for a code'),
  ('auth.freeze_links', 'account', 'account', 'end', 'delete', false, '{}', '"This wasn''t me" links'),
  ('auth.notices', 'account', 'account', 'keep', 'delete', true, '{}', 'security notices sent'),
  ('auth.accounts', 'owner', 'account', 'move', 'keep', false, '{}', 'its agents move; a purge purges each agent'),
  ('auth.agent_grants', 'owner', 'account', 'move', 'delete', false, '{record}', 'agents'' grants (exported as Agent messages)'),
  ('auth.agent_requests', 'owner', 'account', 'move', 'delete', false, '{request}', 'agents'' requests answered'),
  ('auth.agent_keys', 'agent', 'account', 'keep', 'delete', false, '{token_sha256}', 'an agent''s API keys (purged with the agent)'),
  ('auth.agent_grants', 'agent', 'account', 'keep', 'delete', false, '{record}', 'an agent''s grant (purged with the agent)'),
  ('auth.agent_requests', 'agent', 'account', 'keep', 'delete', false, '{request}', 'an agent''s request (purged with the agent)'),
  ('auth.device_logins', 'account', 'account', 'move', 'delete', true, '{code_sha256}', '`openagents login` approvals'),
  ('auth.merge_asks', 'survivor', 'account', 'end', 'delete', false, '{}', 'merges waiting'),
  ('auth.merge_asks', 'other', 'account', 'end', 'delete', false, '{}', 'merges waiting'),
  ('auth.merges', 'survivor', 'account', 'keep', 'keep', true, '{}', 'merges: the audit''s record of both accounts'),
  ('auth.merges', 'merged', 'account', 'keep', 'keep', true, '{}', 'merges: the audit''s record of both accounts'),
  -- the game: accounts, players, realms, saves
  ('verse.players', 'account', 'verse', 'move', 'delete', true, '{key_sha256}', 'players and their characters (characters follow, 0135)'),
  ('verse.worlds', 'owner_account', 'verse', 'move', 'delete', true, '{}', 'realms'),
  ('verse.progress', 'account', 'verse', 'move', 'delete', true, '{cur,prev}', 'saves (sizes only in the export)'),
  ('verse.realm_rights', 'owner_account', 'verse', 'move', 'delete', true, '{}', 'realms'' rights'),
  ('verse.seats', 'account', 'verse', 'move', 'delete', true, '{}', 'seats in realms'),
  ('verse.items', 'owner', 'verse', 'move', 'delete', true, '{}', 'items'),
  ('verse.inventories', 'who', 'verse', 'move', 'delete', false, '{}', 'inventory versions'),
  ('verse.room_vitals', 'who', 'verse', 'move', 'delete', true, '{}', 'health and look'),
  ('verse.room_owners', 'who', 'verse', 'keep', 'delete', false, '{}', 'character fences (0135 moves them)'),
  ('verse.character_checkpoints_v2', 'who', 'verse', 'follows', 'delete', true, '{}', 'characters (0135 moves them with players)'),
  ('verse.character_profiles_v2', 'who', 'verse', 'follows', 'delete', false, '{}', 'characters'),
  ('verse.character_checkpoints_v1', 'who', 'verse', 'keep', 'delete', true, '{}', 'old characters'),
  ('access.creators', 'who', 'verse', 'move', 'delete', true, '{}', 'may make realms'),
  ('access.requests', 'who', 'verse', 'move', 'delete', true, '{}', 'asked to make realms'),
  ('access.redemptions', 'who', 'verse', 'move', 'delete', true, '{}', 'invites used'),
  ('verse.room_chat', 'player', 'player', 'follows', 'delete', true, '{}', 'what you said in rooms'),
  ('verse.log', 'by_player', 'player', 'follows', 'anonymize', false, '{}', 'realm history: who published becomes deleted'),
  ('verse.published', 'world', 'world', 'follows', 'delete', true, '{}', 'your realms'' published pages'),
  ('verse.world_state', 'world', 'world', 'follows', 'delete', true, '{}', 'your realms'' settings'),
  ('verse.log', 'world', 'world', 'follows', 'delete', false, '{}', 'your realms'' history'),
  -- keys: the rows follow a key wherever it goes
  ('inference.calls', 'pubkey', 'pubkey', 'follows', 'delete', true, '{}', 'model calls'),
  ('inference.account_day', 'pubkey', 'pubkey', 'follows', 'delete', true, '{}', 'model use a day'),
  ('inference.credit', 'pubkey', 'pubkey', 'follows', 'delete', true, '{}', 'service credit (spend it first: it can''t be withdrawn)'),
  ('inference.topups', 'pubkey', 'pubkey', 'follows', 'anonymize', true, '{}', 'payments to us: kept for accounting, no longer linked to you'),
  ('inference.used_payments', 'pubkey', 'pubkey', 'follows', 'anonymize', false, '{}', 'payments to us: kept so a payment is never counted twice'),
  ('mesh.calls', 'pubkey', 'pubkey', 'follows', 'delete', true, '{}', 'mesh calls'),
  ('makers.realms', 'pubkey', 'pubkey', 'follows', 'delete', true, '{}', 'realms you make as a maker'),
  ('makers.listings', 'maker', 'pubkey', 'follows', 'delete', true, '{}', 'upgrades you list'),
  ('makers.tips', 'maker', 'pubkey', 'follows', 'anonymize', true, '{}', 'tips paid to you: kept for accounting, no longer linked to you'),
  ('makers.payouts', 'maker', 'pubkey', 'follows', 'anonymize', true, '{}', 'payouts to you: kept for accounting, no longer linked to you'),
  ('chat.conversations', 'owner', 'pubkey', 'follows', 'delete', true, '{}', 'chats'),
  ('chat.turns', 'owner', 'pubkey', 'follows', 'delete', true, '{}', 'chats'),
  ('chat.credentials', 'owner', 'pubkey', 'follows', 'delete', false, '{}', 'chat model keys'),
  ('drops.drops', 'owner', 'pubkey', 'follows', 'delete', true, '{}', 'files you dropped'),
  ('access.requests', 'pubkey', 'pubkey', 'follows', 'delete', false, '{}', 'asked to make realms'),
  ('signin.sessions', 'pubkey', 'pubkey', 'follows', 'delete', false, '{token_sha256}', 'old phone sign-in sessions'),
  ('auth.device_logins', 'pubkey', 'pubkey', 'follows', 'delete', false, '{code_sha256}', '`openagents login` requests');
-- Every registered table and column exists (a typo fails the migration, not a purge).
DO $$
DECLARE r record;
BEGIN
  FOR r IN SELECT tbl, col FROM auth.life_columns LOOP
    IF to_regclass(r.tbl) IS NULL THEN DELETE FROM auth.life_columns WHERE tbl = r.tbl AND col = r.col; CONTINUE; END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_attribute WHERE attrelid = to_regclass(r.tbl) AND attname = r.col AND NOT attisdropped) THEN
      RAISE EXCEPTION 'auth.life_columns: %.% has no column %', r.tbl, r.col, r.col;
    END IF;
  END LOOP;
END $$;

-- Columns that point at an account, a game account or a player by a foreign key, and that the
-- registry doesn't name (auth.accounts' own merged_into is the tombstone itself). Must be empty.
CREATE FUNCTION auth.life_unlisted() RETURNS TABLE (tbl text, col text, points_at text) LANGUAGE sql STABLE AS $$
  SELECT c.conrelid::regclass::text, a.attname::text, c.confrelid::regclass::text
    FROM pg_constraint c
    JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = c.conkey[1]
   WHERE c.contype = 'f' AND c.confrelid IN ('auth.accounts'::regclass, 'verse.accounts'::regclass)
     AND NOT (c.conrelid = 'auth.accounts'::regclass AND a.attname = 'merged_into')
     AND NOT EXISTS (SELECT 1 FROM auth.life_columns l WHERE to_regclass(l.tbl) = c.conrelid AND l.col = a.attname)
$$;

-- ---- the values a registry row's holds stand for
CREATE FUNCTION auth.life_values(acct text, holds_ text) RETURNS text[] LANGUAGE sql STABLE AS $$
  SELECT CASE holds_
    WHEN 'account' THEN ARRAY[acct]
    WHEN 'verse' THEN (SELECT array_agg(a.verse_account) FROM auth.accounts a WHERE a.id = acct)
    WHEN 'player' THEN (SELECT array_agg(p.id) FROM verse.players p JOIN auth.accounts a ON a.verse_account = p.account WHERE a.id = acct)
    WHEN 'pubkey' THEN (SELECT array_agg(k.pubkey) FROM auth.nostr_keys k WHERE k.account = acct)
    WHEN 'world' THEN (SELECT array_agg(w.id) FROM verse.worlds w JOIN auth.accounts a ON a.verse_account = w.owner_account WHERE a.id = acct)
  END
$$;

-- ---- freeze
-- Freeze ("This wasn't me", or the person's own FREEZE): every session ends at once and the account
-- takes sessions only for unfreezing, deleting, cancelling and logging out (auth-life-policy.coax).
-- Its agents are paused by auth.c (agents_pause_all) in the same request. Replaces 0143's.
CREATE OR REPLACE FUNCTION auth.freeze(acct text) RETURNS int LANGUAGE plpgsql AS $$
DECLARE n int;
BEGIN
  UPDATE auth.accounts SET frozen_at = coalesce(frozen_at, now()), frozen_why = 'freeze'
   WHERE id = acct AND deleted_at IS NULL AND merged_into IS NULL;
  UPDATE auth.recovery_requests SET cancelled_at = now(), cancelled_via = 'freeze'   -- as 0148's: a freeze stops a recovery
   WHERE account = acct AND cancelled_at IS NULL AND done_at IS NULL;
  n := auth.sessions_end_all(acct);
  RETURN n;
END $$;
-- Unfreeze: only a freeze (a deletion is cancelled, not unfrozen). true when it was frozen.
CREATE FUNCTION auth.unfreeze(acct text) RETURNS boolean LANGUAGE sql AS $$
  WITH u AS (UPDATE auth.accounts SET frozen_at = NULL, frozen_why = NULL
              WHERE id = acct AND frozen_why = 'freeze' AND delete_asked_at IS NULL AND deleted_at IS NULL RETURNING 1)
  SELECT EXISTS (SELECT 1 FROM u)
$$;
-- A new session for a frozen account is allowed now (0143 refused it): it can only unfreeze,
-- cancel a deletion, look and log out, which the gate enforces. A tombstone takes none.
CREATE OR REPLACE FUNCTION auth.session_new(sha text, acct text, kind_ text, via bigint, level_ text, client_ text, name_ text, country_ text)
RETURNS text LANGUAGE plpgsql AS $$
DECLARE sid text := 'ses_' || left(projects.sha(projects.token()), 16);
BEGIN
  INSERT INTO auth.sessions (token_sha256, id, account, via_kind, via_id, client, level, l1_at, l2_at, epoch, name, country)
  SELECT sha, sid, a.id, kind_, via, client_, level_, now(), CASE WHEN level_ = 'L2' THEN now() END, a.session_epoch,
         nullif(left(name_, 80), ''), nullif(left(country_, 2), '')
    FROM auth.accounts a WHERE a.id = acct AND a.deleted_at IS NULL AND a.merged_into IS NULL;
  IF NOT FOUND THEN RETURN NULL; END IF;
  RETURN sid;
END $$;

-- ---- the facts auth-life-policy.coax decides on, for the page and the routes
CREATE FUNCTION auth.life_of(acct text) RETURNS TABLE (
  kind text, frozen_ms bigint, frozen_why text, delete_asked_ms bigint, purge_ms bigint, now_ms bigint,
  strong int, emails int, keys int, agents int, agents_paused int, credit_msat bigint, worlds int, saves int,
  merged_from int, deleted boolean, merged_into text) LANGUAGE sql STABLE AS $$
  SELECT a.kind,
         coalesce((extract(epoch FROM a.frozen_at) * 1000)::bigint, 0), coalesce(a.frozen_why, ''),
         coalesce((extract(epoch FROM a.delete_asked_at) * 1000)::bigint, 0),
         coalesce((extract(epoch FROM a.purge_at) * 1000)::bigint, 0),
         (extract(epoch FROM now()) * 1000)::bigint,
         (SELECT count(*) FROM auth.nostr_keys k WHERE k.account = a.id)::int
           + (SELECT count(*) FROM auth.passkeys p WHERE p.account = a.id)::int
           + (SELECT count(*) FROM auth.devices d WHERE d.account = a.id AND d.revoked_at IS NULL)::int
           + (SELECT count(*) FROM auth.recovery r WHERE r.account = a.id AND r.spent_at IS NULL)::int,
         (SELECT count(*) FROM auth.emails e WHERE e.account = a.id)::int,
         (SELECT count(*) FROM auth.nostr_keys k WHERE k.account = a.id)::int,
         (SELECT count(*) FROM auth.accounts g WHERE g.owner = a.id AND g.deleted_at IS NULL)::int,
         (SELECT count(*) FROM auth.agent_grants g WHERE g.owner = a.id AND g.revoked_at IS NULL AND g.suspended_at IS NOT NULL)::int,
         coalesce((SELECT sum(c.msat) FROM inference.credit c JOIN auth.nostr_keys k ON k.pubkey = c.pubkey WHERE k.account = a.id AND c.msat > 0), 0)::bigint,
         (SELECT count(*) FROM verse.worlds w WHERE w.owner_account = a.verse_account)::int,
         (SELECT count(*) FROM verse.progress g WHERE g.account = a.verse_account)::int,
         (SELECT count(*) FROM auth.merges m WHERE m.survivor = a.id)::int,
         a.deleted_at IS NOT NULL, coalesce(a.merged_into, '')
    FROM auth.accounts a WHERE a.id = acct
$$;

-- ---- deletion
-- Asked: frozen at once, every session ends, purged at purge_at (30 days on; the constraint
-- refuses sooner). A deletion asked again keeps its first time.
CREATE FUNCTION auth.delete_ask(acct text) RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE at_ timestamptz;
BEGIN
  UPDATE auth.accounts SET delete_asked_at = coalesce(delete_asked_at, now()),
                           purge_at = coalesce(purge_at, now() + interval '30 days'),
                           frozen_at = coalesce(frozen_at, now()), frozen_why = coalesce(frozen_why, 'delete')
   WHERE id = acct AND kind = 'person' AND deleted_at IS NULL AND merged_into IS NULL
   RETURNING purge_at INTO at_;
  IF at_ IS NULL THEN RETURN NULL; END IF;
  PERFORM auth.sessions_end_all(acct);
  RETURN (extract(epoch FROM at_) * 1000)::bigint;
END $$;
-- Cancelled from any way in: the deletion's freeze lifts (a freeze the person made stays).
CREATE FUNCTION auth.delete_cancel(acct text) RETURNS boolean LANGUAGE sql AS $$
  WITH u AS (UPDATE auth.accounts SET delete_asked_at = NULL, purge_at = NULL,
                                      frozen_at = CASE WHEN frozen_why = 'delete' THEN NULL ELSE frozen_at END,
                                      frozen_why = CASE WHEN frozen_why = 'delete' THEN NULL ELSE frozen_why END
              WHERE id = acct AND delete_asked_at IS NOT NULL AND deleted_at IS NULL RETURNING 1)
  SELECT EXISTS (SELECT 1 FROM u)
$$;

-- Deletes the rows where col holds one of vals, and first every row that points at them by a
-- foreign key (recursively). How many rows went.
CREATE FUNCTION auth.life_delete(tbl regclass, col text, vals text[], depth int) RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE
  fk record;
  keys text[];
  n bigint := 0;
  m bigint;
BEGIN
  IF vals IS NULL OR cardinality(vals) = 0 THEN RETURN 0; END IF;
  IF depth > 8 THEN RAISE EXCEPTION 'auth.life_delete: too deep at %', tbl; END IF;
  FOR fk IN
    SELECT c.conrelid::regclass AS dep, da.attname::text AS depcol, ra.attname::text AS refcol
      FROM pg_constraint c
      JOIN pg_attribute da ON da.attrelid = c.conrelid AND da.attnum = c.conkey[1]
      JOIN pg_attribute ra ON ra.attrelid = c.confrelid AND ra.attnum = c.confkey[1]
     WHERE c.contype = 'f' AND c.confrelid = tbl AND cardinality(c.conkey) = 1 AND c.confdeltype <> 'c'
       AND c.conrelid <> tbl
  LOOP
    EXECUTE format('SELECT array_agg(%I::text) FROM %s WHERE %I::text = ANY($1)', fk.refcol, tbl, col) INTO keys USING vals;
    n := n + auth.life_delete(fk.dep, fk.depcol, keys, depth + 1);
  END LOOP;
  EXECUTE format('DELETE FROM %s WHERE %I::text = ANY($1)', tbl, col) USING vals;
  GET DIAGNOSTICS m = ROW_COUNT;
  RETURN n + m;
END $$;

-- The audit keeps its rows; a purged account's rows lose its id (as audit_age does after two
-- years). SECURITY DEFINER: the owner is the only role the audit's trigger lets change a row.
CREATE FUNCTION auth.audit_forget(acct text) RETURNS int LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, auth AS $$
DECLARE n int;
BEGIN
  IF coalesce(acct, '') !~ '^acct_[a-z2-7]{26}$' THEN RETURN 0; END IF;
  IF NOT EXISTS (SELECT 1 FROM auth.accounts WHERE id = acct AND deleted_at IS NOT NULL) THEN RETURN 0; END IF;
  UPDATE auth.audit SET account = NULL WHERE account = acct;
  GET DIAGNOSTICS n = ROW_COUNT;
  RETURN n;
END $$;

-- Purges one account now: every registered row goes or is anonymized, its agents are purged with
-- it, the audit's rows lose its id, and the account row stays as a tombstone (kind, created_at,
-- deleted_at) so the id is never reused. Lightning Address names it holds stay held (the
-- name-holds table of docs/auth/lightning-addresses.md registers here when it lands). Counts.
CREATE FUNCTION auth.purge(acct text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  r record;
  vals text[];
  m bigint;
  counts jsonb := '{}';
  agent text;
  vacct text;
BEGIN
  SELECT verse_account INTO vacct FROM auth.accounts WHERE id = acct AND deleted_at IS NULL FOR UPDATE;
  IF vacct IS NULL THEN RETURN NULL; END IF;
  FOR agent IN SELECT id FROM auth.accounts WHERE owner = acct AND deleted_at IS NULL LOOP
    counts := counts || jsonb_build_object('agent:' || agent, auth.purge(agent));
  END LOOP;
  PERFORM auth.sessions_end_all(acct);
  -- A tombstone from here on: no session, and keep_a_way_in (0142) lets its last way in go.
  UPDATE auth.accounts SET deleted_at = now(), display_name = NULL, team = false WHERE id = acct;
  -- Values first (the keys, players and realms), then the rows: deleting the keys' rows last.
  FOR r IN SELECT l.* FROM auth.life_columns l
            WHERE l.on_purge <> 'keep' AND l.tbl <> 'auth.accounts' AND to_regclass(l.tbl) IS NOT NULL
            ORDER BY CASE l.holds WHEN 'world' THEN 0 WHEN 'player' THEN 1 WHEN 'pubkey' THEN 2 WHEN 'verse' THEN 3 ELSE 4 END,
                     CASE WHEN l.tbl = 'auth.nostr_keys' THEN 1 ELSE 0 END, l.tbl, l.col LOOP
    vals := auth.life_values(acct, r.holds);
    IF vals IS NULL OR cardinality(vals) = 0 THEN CONTINUE; END IF;
    IF r.on_purge = 'delete' THEN
      m := auth.life_delete(r.tbl::regclass, r.col, vals, 0);
    ELSE
      EXECUTE format('UPDATE %s SET %I = ''deleted'' WHERE %I::text = ANY($1)', r.tbl, r.col, r.col) USING vals;
      GET DIAGNOSTICS m = ROW_COUNT;
    END IF;
    IF m > 0 THEN counts := counts || jsonb_build_object(r.tbl || '.' || r.col, m); END IF;
  END LOOP;
  UPDATE verse.accounts SET email = NULL, npub = NULL, name = NULL WHERE id = vacct;
  counts := counts || jsonb_build_object('audit_forgotten', auth.audit_forget(acct));
  RETURN counts;
END $$;

-- Every deletion whose 30 days are up (the daily auth-sweep job). The ids purged.
CREATE FUNCTION auth.purge_due() RETURNS text[] LANGUAGE plpgsql AS $$
DECLARE acct text; done text[] := '{}';
BEGIN
  FOR acct IN SELECT id FROM auth.accounts WHERE purge_at IS NOT NULL AND purge_at <= now() AND deleted_at IS NULL AND kind = 'person'
               ORDER BY purge_at LIMIT 100 LOOP
    PERFORM auth.purge(acct);
    done := done || acct;
  END LOOP;
  RETURN done;
END $$;

-- ---- merge
-- Moves column col of tbl from one value to another, except rows that would collide with a row the
-- target already has under one of the table's unique keys (those stay, and are counted as kept).
CREATE FUNCTION auth.life_move(tbl regclass, col text, from_ text, to_ text, OUT moved bigint, OUT kept bigint) LANGUAGE plpgsql AS $$
DECLARE
  k record;
  clash text := '';
  others text;
BEGIN
  FOR k IN
    SELECT i.indkey::int2[] AS cols FROM pg_index i
     WHERE i.indrelid = tbl AND i.indisunique
       AND (SELECT attnum FROM pg_attribute WHERE attrelid = tbl AND attname = col) = ANY(i.indkey::int2[])
  LOOP
    SELECT string_agg(format('x.%1$I IS NOT DISTINCT FROM t.%1$I', a.attname), ' AND ') INTO others
      FROM pg_attribute a WHERE a.attrelid = tbl AND a.attnum = ANY(k.cols) AND a.attname <> col;
    clash := clash || format(' OR EXISTS (SELECT 1 FROM %s x WHERE x.%I::text = $2%s)', tbl, col,
                             CASE WHEN others IS NULL THEN '' ELSE ' AND ' || others END);
  END LOOP;
  EXECUTE format('UPDATE %s t SET %I = $2 WHERE t.%I::text = $1 AND NOT (false%s)', tbl, col, col, clash) USING from_, to_;
  GET DIAGNOSTICS moved = ROW_COUNT;
  EXECUTE format('SELECT count(*) FROM %s WHERE %I::text = $1', tbl, col) INTO kept USING from_;
END $$;

-- Merges other into survivor: every registered column marked move moves, the other's logins end,
-- its history stays under its own id, and it becomes a tombstone pointing at the survivor.
-- The caller (auth-life.c) has already had auth-life-policy.coax decide that both are proved,
-- and that the other's money moves only if the person confirmed it (money). What moved and
-- what stayed, or NULL when either account can't merge.
CREATE FUNCTION auth.merge(survivor_ text, other_ text, money_ boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  s auth.accounts;
  o auth.accounts;
  r record;
  from_v text;
  to_v text;
  mv bigint; kp bigint;
  moved jsonb := '{}';
  kept jsonb := '{}';
BEGIN
  IF survivor_ = other_ THEN RETURN NULL; END IF;
  -- Both rows locked, in id order, so two merges can't cross.
  PERFORM 1 FROM auth.accounts WHERE id IN (survivor_, other_) ORDER BY id FOR UPDATE;
  SELECT * INTO s FROM auth.accounts WHERE id = survivor_;
  SELECT * INTO o FROM auth.accounts WHERE id = other_;
  IF s.id IS NULL OR o.id IS NULL OR s.kind <> 'person' OR o.kind <> 'person'
     OR s.deleted_at IS NOT NULL OR o.deleted_at IS NOT NULL OR s.merged_into IS NOT NULL OR o.merged_into IS NOT NULL
     OR s.delete_asked_at IS NOT NULL OR o.delete_asked_at IS NOT NULL THEN
    RETURN NULL;
  END IF;
  PERFORM auth.sessions_end_all(other_);
  FOR r IN SELECT l.* FROM auth.life_columns l WHERE l.on_merge IN ('move', 'end') AND to_regclass(l.tbl) IS NOT NULL ORDER BY l.holds, l.tbl, l.col LOOP
    IF r.holds = 'account' THEN from_v := other_; to_v := survivor_;
    ELSIF r.holds = 'verse' THEN from_v := o.verse_account; to_v := s.verse_account;
    ELSE CONTINUE; END IF;
    IF r.on_merge = 'end' THEN
      EXECUTE format('DELETE FROM %s WHERE %I::text = $1', r.tbl, r.col) USING from_v;
      CONTINUE;
    END IF;
    SELECT x.moved, x.kept INTO mv, kp FROM auth.life_move(r.tbl::regclass, r.col, from_v, to_v) x;
    IF mv > 0 THEN moved := moved || jsonb_build_object(r.tbl || '.' || r.col, mv); END IF;
    IF kp > 0 THEN kept := kept || jsonb_build_object(r.tbl || '.' || r.col, kp); END IF;
  END LOOP;
  -- A realm's admins name game accounts too.
  UPDATE verse.realm_rights SET admins = array_replace(admins, o.verse_account, s.verse_account) WHERE o.verse_account = ANY(admins);
  -- Freezes join: a frozen side keeps the survivor frozen. Recovery waiting on the other ends.
  UPDATE auth.accounts SET frozen_at = coalesce(s.frozen_at, o.frozen_at), frozen_why = coalesce(s.frozen_why, o.frozen_why),
                           team = s.team OR o.team
   WHERE id = survivor_;
  UPDATE auth.recovery_requests SET cancelled_at = now() WHERE account = other_ AND cancelled_at IS NULL AND done_at IS NULL;
  UPDATE auth.accounts SET merged_into = survivor_, deleted_at = now(), frozen_at = NULL, frozen_why = NULL WHERE id = other_;
  INSERT INTO auth.merges (survivor, merged, moved, kept, money) VALUES (survivor_, other_, moved, kept, money_);
  RETURN jsonb_build_object('moved', moved, 'kept', kept);
END $$;

-- The other account's facts for the merge's confirmation: what would move.
CREATE FUNCTION auth.merge_preview(other_ text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object('emails', l.emails, 'keys', l.keys, 'agents', l.agents, 'worlds', l.worlds, 'saves', l.saves,
                            'credit_msat', l.credit_msat, 'strong', l.strong, 'kind', l.kind,
                            'frozen', l.frozen_ms > 0, 'deleting', l.delete_asked_ms > 0, 'gone', l.deleted)
    FROM auth.life_of(other_) l
$$;

-- ---- merge asks
CREATE FUNCTION auth.merge_ask(id_ text, survivor_ text, session_ text) RETURNS void LANGUAGE sql AS $$
  DELETE FROM auth.merge_asks WHERE survivor = survivor_ AND done_at IS NULL;
  INSERT INTO auth.merge_asks (id, survivor, session, expires_at) VALUES (id_, survivor_, session_, now() + interval '10 minutes');
$$;
-- The other account proved (by email or key), for the survivor's one open ask: 'ok', 'none' (no
-- open ask, or it ended), 'self' (the proof was the survivor's own).
CREATE FUNCTION auth.merge_proved(survivor_ text, other_ text, via text) RETURNS text LANGUAGE plpgsql AS $$
DECLARE a auth.merge_asks;
BEGIN
  SELECT * INTO a FROM auth.merge_asks WHERE survivor = survivor_ AND done_at IS NULL AND expires_at > now()
   ORDER BY expires_at DESC LIMIT 1 FOR UPDATE;
  IF a.id IS NULL THEN RETURN 'none'; END IF;
  IF other_ = survivor_ THEN RETURN 'self'; END IF;
  UPDATE auth.merge_asks SET other = other_, proved_via = via, proved_at = now() WHERE id = a.id;
  RETURN 'ok';
END $$;
-- The survivor's open ask, to confirm from the session that asked: its other account and how old
-- that account's proof is (ms), or no row.
CREATE FUNCTION auth.merge_ready(survivor_ text, session_ text) RETURNS TABLE (id text, other text, proved_via text, proof_age_ms bigint)
LANGUAGE sql STABLE AS $$
  SELECT a.id, a.other, a.proved_via, (extract(epoch FROM now() - a.proved_at) * 1000)::bigint
    FROM auth.merge_asks a
   WHERE a.survivor = survivor_ AND a.session = session_ AND a.done_at IS NULL AND a.expires_at > now() AND a.other IS NOT NULL
   ORDER BY a.expires_at DESC LIMIT 1
$$;
CREATE FUNCTION auth.merge_done(id_ text) RETURNS void LANGUAGE sql AS $$ UPDATE auth.merge_asks SET done_at = now() WHERE id = id_ $$;

-- ---- export
-- Every registered row the account (and the accounts merged into it) holds, as JSON lines of
-- {"type":"row","table":...,"column":...,"row":{...}}, without the columns the registry omits.
-- auth-life.c adds the header, the emails themselves, the agents as Agent messages and the audit.
CREATE FUNCTION auth.export_rows(acct text) RETURNS SETOF text LANGUAGE plpgsql STABLE AS $$
DECLARE
  r record;
  a text;
  vals text[];
  line text;
BEGIN
  FOR a IN SELECT acct UNION ALL SELECT m.merged FROM auth.merges m WHERE m.survivor = acct LOOP
    FOR r IN SELECT l.* FROM auth.life_columns l WHERE l.export AND to_regclass(l.tbl) IS NOT NULL ORDER BY l.tbl, l.col LOOP
      vals := auth.life_values(a, r.holds);
      IF vals IS NULL OR cardinality(vals) = 0 THEN CONTINUE; END IF;
      FOR line IN EXECUTE format(
          'SELECT jsonb_build_object(''type'', ''row'', ''account'', $3, ''table'', $2, ''column'', $4, ''row'', to_jsonb(t) - $5)::text '
          'FROM %s t WHERE %I::text = ANY($1) LIMIT 50000', r.tbl, r.col)
        USING vals, r.tbl, a, r.col, r.omit LOOP
        RETURN NEXT line;
      END LOOP;
    END LOOP;
  END LOOP;
END $$;
-- The audit rows of the account and of every account merged into it.
CREATE FUNCTION auth.export_audit(acct text) RETURNS SETOF text LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object('type', 'audit', 'account', x.account, 'at', x.at, 'action', x.action, 'result', x.result, 'detail', x.detail)::text
    FROM auth.audit x
   WHERE x.account = acct OR x.account IN (SELECT m.merged FROM auth.merges m WHERE m.survivor = acct)
   ORDER BY x.id
$$;

-- Sweeps: merge asks done or ended over a day ago.
CREATE FUNCTION auth.life_sweep() RETURNS int LANGUAGE sql AS $$
  WITH d AS (DELETE FROM auth.merge_asks WHERE expires_at < now() - interval '1 day' RETURNING 1) SELECT count(*)::int FROM d
$$;

-- Roles (#13194): the API reads and writes rows; the dump and read-only roles read.
DO $$
DECLARE r text;
BEGIN
  FOREACH r IN ARRAY ARRAY['oa_api', 'oa_api_staging'] LOOP
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = r) THEN
      EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON auth.merges, auth.merge_asks TO %I', r);
      EXECUTE format('GRANT SELECT ON auth.life_columns TO %I', r);
      EXECUTE format('GRANT USAGE, SELECT ON SEQUENCE auth.merges_id_seq TO %I', r);
    END IF;
  END LOOP;
  FOREACH r IN ARRAY ARRAY['oa_dump', 'oa_readonly'] LOOP
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = r) THEN
      EXECUTE format('GRANT SELECT ON auth.merges, auth.merge_asks, auth.life_columns TO %I', r);
    END IF;
  END LOOP;
END $$;

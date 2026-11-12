-- name@openagents.com Lightning Addresses (docs/auth/lightning-addresses.md, #13208). The money
-- never lands with us: an address forwards to the person's own Lightning Address (forward mode)
-- or makes invoices for their own Spark wallet (spark mode); with neither it answers "This
-- address isn't receiving yet". lnaddr.c serves the LNURL routes; lnaddr-policy.coax and
-- coax/lib/lnaddr/lnurl.coax decide. These tables keep the facts.
--
-- pay.addresses        one per person account: the name, its skeleton (look-alikes are one name),
--                      where the money goes, the state, the revision (bumped on every change)
-- pay.address_changes  a destination change waits here 24 hours (72 for email-only accounts),
--                      cancellable until it applies; one waiting change per name
-- pay.name_holds       a name ever paid, held by its account for good: rows are never deleted or
--                      moved, and no address may take a held name or skeleton from another account
-- pay.invoices         every invoice we handed out: its payment hash, amount, the revision it was
--                      for (an invoice keeps its recipient whatever changes later), the note
--                      (deleted 30 days after it was read), and for forward mode the upstream's
--                      LUD-21 verify URL, never shown to the payer
-- pay.notices          each destination change's notices, for every channel the account has
-- pay.secrets          the key the limits hash network addresses under (never an address itself)
-- Every claim, change, cancel, on and off also goes into auth.audit.
CREATE SCHEMA IF NOT EXISTS pay;

CREATE TABLE pay.addresses (
  name text PRIMARY KEY CHECK (name ~ '^[a-z0-9]([a-z0-9._-]{1,30})[a-z0-9]$' AND name !~ '[._-]{2}'),
  skeleton text NOT NULL UNIQUE CHECK (skeleton ~ '^[a-z0-9]{1,32}$'),
  account text NOT NULL UNIQUE REFERENCES auth.accounts (id),
  mode text NOT NULL DEFAULT 'none' CHECK (mode IN ('none', 'spark', 'forward')),
  spark_pubkey text CHECK (spark_pubkey ~ '^0[23][0-9a-f]{64}$'),
  forward_to text CHECK (length(forward_to) <= 160),
  state text NOT NULL DEFAULT 'active' CHECK (state IN ('active', 'off', 'suspended', 'retired')),
  revision bigint NOT NULL DEFAULT 1 CHECK (revision >= 1),
  max_msat bigint NOT NULL DEFAULT 1000000000 CHECK (max_msat BETWEEN 1000 AND 1000000000),
  comments_on boolean NOT NULL DEFAULT true,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  renamed_from text,
  CHECK ((mode = 'spark') = (spark_pubkey IS NOT NULL)),
  CHECK ((mode = 'forward') = (forward_to IS NOT NULL))
);

CREATE TABLE pay.name_holds (
  name text PRIMARY KEY,
  skeleton text NOT NULL,
  account text NOT NULL,
  since timestamptz NOT NULL DEFAULT now(),
  reason text NOT NULL CHECK (reason IN ('paid', 'renamed', 'deleted', 'suspended'))
);
CREATE INDEX name_holds_skeleton ON pay.name_holds (skeleton);

-- Holds are forever: no role but the owner (migrations) may change or delete one.
CREATE FUNCTION pay.holds_frozen() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF current_user <> (SELECT pg_get_userbyid(c.relowner) FROM pg_class c WHERE c.oid = TG_RELID) THEN
    RAISE EXCEPTION 'pay.name_holds rows are held for good (docs/auth/lightning-addresses.md#names)' USING ERRCODE = 'insufficient_privilege';
  END IF;
  RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END $$;
CREATE TRIGGER holds_frozen BEFORE UPDATE OR DELETE ON pay.name_holds FOR EACH ROW EXECUTE FUNCTION pay.holds_frozen();

-- No address takes a name, or a look-alike of one, that another account holds; and an address's
-- name never moves to another account once held. A second wall behind the API's own check.
CREATE FUNCTION pay.respect_holds() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF EXISTS (SELECT 1 FROM pay.name_holds h WHERE (h.name = NEW.name OR h.skeleton = NEW.skeleton) AND h.account <> NEW.account) THEN
    RAISE EXCEPTION 'pay: % is held by another account', NEW.name USING ERRCODE = 'unique_violation';
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER respect_holds BEFORE INSERT OR UPDATE OF name, skeleton, account ON pay.addresses
  FOR EACH ROW EXECUTE FUNCTION pay.respect_holds();

CREATE TABLE pay.address_changes (
  id bigserial PRIMARY KEY,
  name text NOT NULL REFERENCES pay.addresses (name),
  account text NOT NULL,
  new_mode text NOT NULL CHECK (new_mode IN ('spark', 'forward')),
  new_target text NOT NULL CHECK (length(new_target) <= 160),
  proof text,                              -- spark: the new wallet's signature, hex
  requested_session text,
  requested_at timestamptz NOT NULL DEFAULT now(),
  ready_at timestamptz NOT NULL,
  cancelled_at timestamptz,
  applied_at timestamptz,
  CHECK (cancelled_at IS NULL OR applied_at IS NULL)
);
CREATE UNIQUE INDEX address_changes_one_waiting ON pay.address_changes (name) WHERE cancelled_at IS NULL AND applied_at IS NULL;

CREATE TABLE pay.invoices (
  payment_hash text PRIMARY KEY CHECK (payment_hash ~ '^[0-9a-f]{64}$'),
  name text NOT NULL,
  revision bigint NOT NULL,
  mode text NOT NULL CHECK (mode IN ('spark', 'forward')),
  amount_msat bigint NOT NULL CHECK (amount_msat > 0),
  pr text NOT NULL,
  verify_url text,
  zap_request text CHECK (length(zap_request) <= 8192),
  comment text CHECK (length(comment) <= 1200),
  comment_read_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT now(),
  settled_at timestamptz,
  preimage text CHECK (preimage ~ '^[0-9a-f]{64}$')
);
CREATE INDEX invoices_name ON pay.invoices (name, created_at);

CREATE TABLE pay.notices (
  id bigserial PRIMARY KEY,
  account text NOT NULL,
  name text NOT NULL,
  kind text NOT NULL CHECK (kind IN ('change.requested', 'change.applied', 'change.cancelled', 'claimed', 'off', 'on')),
  detail jsonb NOT NULL DEFAULT '{}' CHECK (length(detail::text) <= 1000),
  created_at timestamptz NOT NULL DEFAULT now(),
  emailed_at timestamptz
);
CREATE INDEX notices_account ON pay.notices (account, created_at);

CREATE TABLE pay.secrets (
  name text PRIMARY KEY,
  value text NOT NULL CHECK (value ~ '^[0-9a-f]{64}$')
);

-- Notes go 30 days after their holder read them (the cleanup job calls this).
CREATE FUNCTION pay.sweep() RETURNS int LANGUAGE sql AS $$
  WITH d AS (UPDATE pay.invoices SET comment = NULL WHERE comment IS NOT NULL AND comment_read_at < now() - interval '30 days' RETURNING 1)
  SELECT count(*)::int FROM d
$$;

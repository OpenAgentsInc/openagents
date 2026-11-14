-- Resolve legacy key-addressed records through account owner handles too.
-- These are storage aliases only; auth.strong_ways and key login are unchanged.
CREATE OR REPLACE FUNCTION auth.life_values(acct text, holds_ text) RETURNS text[] LANGUAGE sql STABLE AS $$
  SELECT CASE holds_
    WHEN 'account' THEN ARRAY[acct]
    WHEN 'verse' THEN (SELECT array_agg(a.verse_account) FROM auth.accounts a WHERE a.id = acct)
    WHEN 'player' THEN (SELECT array_agg(p.id) FROM verse.players p JOIN auth.accounts a ON a.verse_account = p.account WHERE a.id = acct)
    WHEN 'pubkey' THEN (SELECT array_agg(x.owner) FROM (SELECT k.pubkey AS owner FROM auth.nostr_keys k WHERE k.account=acct UNION SELECT o.owner FROM drops.account_owners_v1 o WHERE o.account=acct) x)
    WHEN 'world' THEN (SELECT array_agg(w.id) FROM verse.worlds w JOIN auth.accounts a ON a.verse_account = w.owner_account WHERE a.id = acct)
  END
$$;

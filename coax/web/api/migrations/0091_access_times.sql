-- /admin/access shows when (docs/auth/2026-10-31-realm-invites-and-auth.md): each invite's expiry,
-- each request's time and each creator's, in Chicago time as people read it (projects.chi), as a
-- last field after the ones 0090 wrote. Inert: it replaces only 0090's admin_lines, which only
-- an admin reads.
CREATE OR REPLACE FUNCTION access.admin_lines() RETURNS text LANGUAGE sql STABLE AS $$
  SELECT string_agg(l, E'\n' ORDER BY o, k) || E'\n' FROM (
    SELECT 0 o, 0::bigint k, format('F|%s|%s', n, CASE WHEN access.flag(n) THEN 1 ELSE 0 END) l FROM unnest(ARRAY['realm_invites', 'auth_v2']) n
    UNION ALL
    SELECT 1, -i.id, format('I|%s|%s|%s|%s|%s|%s|%s', i.id, i.uses, i.used, i.expires_ms,
      CASE WHEN i.revoked_ms IS NOT NULL THEN 'revoked' WHEN i.used >= i.uses THEN 'used' WHEN i.expires_ms <= projects.now_ms() THEN 'expired' ELSE 'live' END,
      access.line(i.note), projects.chi(i.expires_ms)) FROM access.invites i
    UNION ALL
    SELECT 2, CASE WHEN r.state = 'waiting' THEN r.id ELSE 1000000000000 - r.id END,
      format('R|%s|%s|%s|%s|%s|%s|%s', r.id, signin.bech32('npub', r.pubkey), access.line(r.note), access.line(r.contact), r.state, r.at_ms, projects.chi(r.at_ms)) FROM access.requests r
    UNION ALL
    SELECT 3, -c.at_ms, format('C|%s|%s|%s|%s', access.line(c.who), c.how, c.at_ms, projects.chi(c.at_ms)) FROM access.creators c
  ) x
$$;

-- Watchtower (/realms/watchtower), a cyber defense range, joins the hand-built realms that edit
-- (docs/verse/realm-edit.md): its owner account and admins as apps/openagents-web/realms.json says,
-- and the two rows its code switches (watchtower.coax: triad_on and the "traffic" row);
-- test_realm_rights.py checks the two agree.
INSERT INTO verse.realm_rights (slug, owner_account, admins, tools, things, rows) VALUES ('watchtower', 'n-9f4b4c7e5944ddb3404651d3ba1a9985dc4261ccbca4e81d769a54e43614e27a', ARRAY['n-1e60e1b4820b482c593da4d7745f479fd1a5018ba374f50978b99bad9963c423']::text[], ARRAY[]::text[], ARRAY[]::text[],
  '[{"id": "triad", "label": "The triad''s advice and autopilot", "says": "the three advisors'' votes on each host, FOLLOW TRIAD, and the triad defending on its own"}, {"id": "traffic", "label": "Ordinary traffic in the log", "says": "everyday log lines the intruder hides among; off, the log shows only the intruder and you"}]'::jsonb)
ON CONFLICT (slug) DO NOTHING;

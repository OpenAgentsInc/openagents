-- The Rust relay's events as JSON lines, for the Coax relay's --import
-- (chapter 103, steps 4 to 6; #13361 step 2). Run with psql against a
-- throwaway database restored from a backup of the Rust relay's Postgres,
-- never against production's:
--   psql -X -q -At -v ON_ERROR_STOP=1 -v out=DIR -f nostr/relay/export.sql
-- It writes, in DIR:
--   counts.txt          what an event export can't carry, for the record
--   export.jsonl        every live row of nostr_event in ingest_seq order, one
--                       NIP-01 object a line, except events of private or hidden
--                       NIP-29 groups (decision 3)
--   refused-groups.jsonl the events of private or hidden groups, which stay off
--                       the Coax relay
-- A SELECT, not chapter 103's COPY: COPY's text format doubles every backslash,
-- and a JSON line's \" and \\ must reach the importer as they are.
\set FETCH_COUNT 2000
\pset footer off
BEGIN TRANSACTION READ ONLY ISOLATION LEVEL REPEATABLE READ;
\o :out/counts.txt
SELECT 'events', count(*), coalesce(max(ingest_seq), 0), coalesce(max(created_at), 0) FROM nostr_event;
SELECT 'groups', count(*), count(*) FILTER (WHERE private), count(*) FILTER (WHERE hidden) FROM relay_group;
SELECT 'group_members', count(*) FROM relay_group_member;
SELECT 'private_or_hidden_group_events', count(*) FROM nostr_event e
  WHERE EXISTS (SELECT 1 FROM relay_group g WHERE (g.private OR g.hidden)
                AND e.tags @> jsonb_build_array(jsonb_build_array('h', g.id)));
SELECT 'tombstones', count(*) FROM deletion_tombstone;
SELECT 'orphan_tombstones', tombstone_type, count(*) FROM deletion_tombstone t
  WHERE NOT EXISTS (SELECT 1 FROM nostr_event e WHERE e.id = t.deletion_event_id)
  GROUP BY tombstone_type ORDER BY tombstone_type;
SELECT 'policy', (SELECT count(*) FROM relay_blocked_pubkey), (SELECT count(*) FROM relay_blocked_kind),
       (SELECT count(*) FROM relay_allowed_pubkey), (SELECT count(*) FROM relay_allowed_kind),
       (SELECT count(*) FROM relay_member_pubkey);
SELECT 'kind62', count(*) FROM nostr_event WHERE kind = 62;
SELECT 'kind', kind, count(*) FROM nostr_event
  WHERE kind IN (78, 1059, 3187, 3188, 21059, 24200, 30078, 30174, 30175, 30178, 30179, 30181, 30186, 30300, 30350, 30622, 39620, 39650, 44200)
     OR kind BETWEEN 39604 AND 39613
  GROUP BY kind ORDER BY kind;
\o :out/export.jsonl
SELECT json_build_object('id', id, 'pubkey', pubkey, 'created_at', created_at, 'kind', kind,
                         'tags', tags, 'content', content, 'sig', sig)
  FROM nostr_event e
  WHERE NOT EXISTS (SELECT 1 FROM relay_group g WHERE (g.private OR g.hidden)
                    AND e.tags @> jsonb_build_array(jsonb_build_array('h', g.id)))
  ORDER BY ingest_seq;
\o :out/refused-groups.jsonl
SELECT json_build_object('id', id, 'pubkey', pubkey, 'created_at', created_at, 'kind', kind,
                         'tags', tags, 'content', content, 'sig', sig)
  FROM nostr_event e
  WHERE EXISTS (SELECT 1 FROM relay_group g WHERE (g.private OR g.hidden)
                AND e.tags @> jsonb_build_array(jsonb_build_array('h', g.id)))
  ORDER BY ingest_seq;
\o
COMMIT;

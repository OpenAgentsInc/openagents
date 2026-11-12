-- The rows the Rust relay took after ingest_seq :after, as JSON lines in
-- ingest_seq order: the cutover's catch-up, and a rollback's (#13361; chapter
-- 103, "Cutover"). relay_live_cutover.py publish sends them to the Coax relay.
-- Run with psql against a throwaway database restored from a backup of the
-- Rust relay's Postgres, never against production's:
--   psql -X -q -At -v ON_ERROR_STOP=1 -v out=DIR -v after=SEQ -f nostr/relay/export-delta.sql
-- SEQ is the last export's newest ingest_seq (its counts.txt "events" line).
-- It writes, in DIR:
--   delta-counts.txt  the table's size and newest ingest_seq (the next SEQ), the
--                     delta's size by kind with its largest content, and the
--                     counts an event export can't carry
--   delta.jsonl       the rows, except events of private or hidden NIP-29 groups
\set FETCH_COUNT 2000
\pset footer off
BEGIN TRANSACTION READ ONLY ISOLATION LEVEL REPEATABLE READ;
\o :out/delta-counts.txt
SELECT 'events', count(*), coalesce(max(ingest_seq), 0), coalesce(max(created_at), 0) FROM nostr_event;
SELECT 'delta', count(*), coalesce(min(ingest_seq), 0), coalesce(max(ingest_seq), 0) FROM nostr_event WHERE ingest_seq > :after;
SELECT 'delta_kind', kind, count(*), max(octet_length(content)) FROM nostr_event WHERE ingest_seq > :after GROUP BY kind ORDER BY kind;
SELECT 'groups', count(*), count(*) FILTER (WHERE private), count(*) FILTER (WHERE hidden) FROM relay_group;
SELECT 'tombstones', count(*) FROM deletion_tombstone;
SELECT 'policy', (SELECT count(*) FROM relay_blocked_pubkey), (SELECT count(*) FROM relay_blocked_kind),
       (SELECT count(*) FROM relay_allowed_pubkey), (SELECT count(*) FROM relay_allowed_kind),
       (SELECT count(*) FROM relay_member_pubkey);
\o :out/delta.jsonl
SELECT json_build_object('id', id, 'pubkey', pubkey, 'created_at', created_at, 'kind', kind,
                         'tags', tags, 'content', content, 'sig', sig)
  FROM nostr_event e
  WHERE ingest_seq > :after
    AND NOT EXISTS (SELECT 1 FROM relay_group g WHERE (g.private OR g.hidden)
                    AND e.tags @> jsonb_build_array(jsonb_build_array('h', g.id)))
  ORDER BY ingest_seq;
\o
COMMIT;

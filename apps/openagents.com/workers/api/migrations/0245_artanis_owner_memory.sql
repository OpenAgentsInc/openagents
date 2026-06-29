-- Artanis owner-interaction memory (issue #6363, epic #6359).
--
-- Persistent, OWNER-SCOPED memory for the "Talk to Artanis" operator channel:
-- prior owner<->Artanis conversation turns plus durable notes (stated
-- decisions, preferences). This is what lets continuity hold across sessions so
-- the operator agent remembers the owner instead of starting cold each time.
--
-- Strictly private. Every row is keyed by owner_id and reads MUST be owner
-- scoped (the store never returns one owner's rows to another). This table is
-- never projected publicly and must not feed the public Khala identity or any
-- public counter/projection.
--
-- kind: 'turn' rows carry a role ('owner' | 'artanis') + the message text of a
-- single conversation turn. kind: 'note' rows carry a durable note category
-- ('decision' | 'preference' | 'fact') + the note text. Both share the same
-- bounded text column so the store can read a single ordered owner timeline.

CREATE TABLE IF NOT EXISTS artanis_owner_memory (
  memory_ref TEXT PRIMARY KEY,
  owner_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('turn', 'note')),
  role TEXT CHECK (role IN ('owner', 'artanis')),
  note_category TEXT CHECK (note_category IN ('decision', 'preference', 'fact')),
  body TEXT NOT NULL,
  created_at TEXT NOT NULL
);

-- Primary read path: most-recent-first timeline for one owner.
CREATE INDEX IF NOT EXISTS idx_artanis_owner_memory_owner_created
  ON artanis_owner_memory (owner_id, created_at DESC);

-- Secondary read path: durable notes only, for one owner.
CREATE INDEX IF NOT EXISTS idx_artanis_owner_memory_owner_kind_created
  ON artanis_owner_memory (owner_id, kind, created_at DESC);

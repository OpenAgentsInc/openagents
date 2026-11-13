-- A world's content tables, edited (GitHub #13419, the browser editor's E10;
-- docs/protocol/world-edit-v1.md "A cell"). A cell edit is an edit of the world's one log, and the
-- tables it changes are saved beside the lane's document: dev's as they are edited, live's as they
-- were published, and the ones before the last pin change for one release. '' means the tables
-- the build shipped with: nobody has edited them.
ALTER TABLE verse.world_docs
  ADD COLUMN dev_tables text NOT NULL DEFAULT '',
  ADD COLUMN live_tables text NOT NULL DEFAULT '',
  ADD COLUMN prev_tables text NOT NULL DEFAULT '';

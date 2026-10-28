-- The demo's data, carried over (pt-import-demo-data; import_demo.py and
-- import_demo.sql). The import reads the bucket aethyr-demo-data read-only
-- and writes here, keyed on the demo's own ids, so it runs again at any time
-- (just before the switch-over too) and adds only what is new.
--
-- source: the demo file a row came from, "aethyr-demo-data/<path>", or null
-- for a row openagents.com wrote itself. Events carry the same in import_key
-- (0002_events.sql), with the file's line.
ALTER TABLE verse.players ADD COLUMN source text;
ALTER TABLE verse.worlds ADD COLUMN source text;
ALTER TABLE verse.published ADD COLUMN source text;
ALTER TABLE verse.log ADD COLUMN source text, ADD COLUMN import_key text UNIQUE;
ALTER TABLE analytics.playtesters ADD COLUMN source text;
ALTER TABLE analytics.survey_answers ADD COLUMN source text;
ALTER TABLE analytics.surveys ADD COLUMN source text;

-- Every other demo file as it was, for the tasks that give it a table later:
-- planets and awards (pt-planets), room chat (pt-presence-chat), offered
-- changes in the edit logs (pt-change-offers), the old step counter, the
-- Grid's suggestions. space: 'kiki' (the demo's namespace "") or 'dev'.
CREATE TABLE verse.demo_files (
  path text PRIMARY KEY,         -- aethyr-demo-data/<path>
  space text NOT NULL,
  doc jsonb NOT NULL,
  md5 text NOT NULL,             -- of the file's bytes: a changed file is read again
  imported_ms bigint NOT NULL
);

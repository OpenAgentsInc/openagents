-- Pluralis roadmap P1.2/P1.3 (openagents#4850, openagents#4851, master
-- tracking issue #4855, rail #4673): the run authority carries the
-- seal/snapshot publication cadence as a per-run contract value (joiners
-- bootstrap from the last durable seal only, so cadence bounds joiner
-- staleness), and a run-level seal-in-flight marker raised while a window
-- seal operation is being persisted so the dispatcher queues joiner
-- bootstrap grants and join-lifecycle transitions instead of handing out
-- half-updated state.

ALTER TABLE training_runs
  ADD COLUMN seal_publication_cadence_windows INTEGER NOT NULL DEFAULT 1;

ALTER TABLE training_runs
  ADD COLUMN seal_in_flight_at TEXT;

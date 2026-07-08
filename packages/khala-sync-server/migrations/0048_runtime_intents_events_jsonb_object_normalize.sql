-- Normalize DOUBLE-ENCODED runtime jsonb columns to real jsonb OBJECTS.
--
-- Before the writer fix, `runtime.startTurn` / `runtime.recordEvent` bound a
-- PRE-STRINGIFIED JSON string (`canonicalJson(...)`) into the jsonb columns
-- `khala_sync_runtime_control_intents.intent_json` and
-- `khala_sync_runtime_events.event_json`. Both drivers (Bun's `SQL`,
-- postgres.js over Hyperdrive) then JSON-encoded that string AGAIN, so the
-- stored jsonb was a STRING SCALAR (`"{\"bodyRef\":...}"`,
-- `jsonb_typeof = 'string'`) rather than an object — making
-- `intent_json->>'bodyRef'` NULL and breaking hosted-chat turn resolution.
--
-- The writer now binds the object (single-encode). This one-time backfill
-- unwraps the historical string scalars: `col #>> '{}'` extracts the inner
-- JSON text of the string scalar, and `::jsonb` re-parses it into the object
-- it always should have been. Scoped ONLY to these two runtime tables (whose
-- writers are fixed); the other jsonb columns (changelog.post_image_json,
-- mutations.result_json) still store string scalars by design and their
-- readers expect that form, so they are intentionally left untouched.
--
-- Idempotent: only rows still encoded as a string scalar are rewritten, so
-- re-running is a no-op. No prompts or private bodies are stored in these
-- columns (ref-only redaction); this only changes the jsonb SHAPE, not content.

UPDATE khala_sync_runtime_control_intents
   SET intent_json = (intent_json #>> '{}')::jsonb
 WHERE jsonb_typeof(intent_json) = 'string';

UPDATE khala_sync_runtime_events
   SET event_json = (event_json #>> '{}')::jsonb
 WHERE jsonb_typeof(event_json) = 'string';

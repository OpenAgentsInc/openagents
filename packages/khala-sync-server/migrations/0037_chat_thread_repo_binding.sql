-- MM-B2 (#8472) follow-up: server-side storage for the mobile repo<->thread
-- binding. The mobile client (packages/khala-sync's ChatThreadEntity.repoBinding,
-- landed in f61f82a94f) has applied this optimistically on-device since #8472
-- closed; this migration plus chat.bindThreadRepo in chat-mutators.ts is what
-- makes it durable server-side so it survives across devices/sessions and
-- reaches the org-cloud executor.
--
-- Flat nullable columns, matching this table's existing convention (no JSONB
-- elsewhere in khala_sync_chat_threads). All three are NULL together (no
-- repo bound) or all three set together (enforced by the mutator, not a DB
-- constraint, to keep the migration additive/reversible).

ALTER TABLE khala_sync_chat_threads
  ADD COLUMN IF NOT EXISTS repo_binding_owner text,
  ADD COLUMN IF NOT EXISTS repo_binding_name text,
  ADD COLUMN IF NOT EXISTS repo_binding_default_branch text;

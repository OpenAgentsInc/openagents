-- MOBILE-PARITY-03D1 (#8955): owner-private chat lifecycle tombstones.
-- Message bodies remain in scope.thread.*; personal scope receives only the
-- existing bounded chat_thread metadata post-image.

ALTER TABLE khala_sync_chat_threads
  DROP CONSTRAINT IF EXISTS khala_sync_chat_threads_status_check;

ALTER TABLE khala_sync_chat_threads
  ADD CONSTRAINT khala_sync_chat_threads_status_check
  CHECK (status IN ('active', 'archived', 'deleted'));

-- CUT-16: owner-private, byte-bearing image attachments on authoritative chat
-- messages. The mutator schema and digest verifier enforce the tighter
-- count/media/size/integrity contract before this JSON reaches storage.
ALTER TABLE khala_sync_chat_messages
  ADD COLUMN IF NOT EXISTS attachments_json jsonb NOT NULL DEFAULT '[]'::jsonb;

ALTER TABLE khala_sync_chat_messages
  DROP CONSTRAINT IF EXISTS khala_sync_chat_messages_attachments_array;
ALTER TABLE khala_sync_chat_messages
  ADD CONSTRAINT khala_sync_chat_messages_attachments_array
  CHECK (jsonb_typeof(attachments_json) = 'array'
    AND jsonb_array_length(attachments_json) <= 4);

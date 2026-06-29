UPDATE team_chat_messages
SET autopilot_thread_id =
  lower(hex(randomblob(4))) || '-' ||
  lower(hex(randomblob(2))) || '-' ||
  '4' || substr(lower(hex(randomblob(2))), 2) || '-' ||
  substr('89ab', (random() & 3) + 1, 1) ||
  substr(lower(hex(randomblob(2))), 2) || '-' ||
  lower(hex(randomblob(6)))
WHERE kind = 'autopilot_intent'
  AND autopilot_thread_id IS NULL;

CREATE INDEX team_chat_messages_autopilot_thread_idx
  ON team_chat_messages(autopilot_thread_id, created_at)
  WHERE autopilot_thread_id IS NOT NULL;
